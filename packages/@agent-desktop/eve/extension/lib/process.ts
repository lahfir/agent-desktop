/**
 * Run one command in the eve sandbox with a stop that is witnessed.
 *
 * When a call's budget runs out, or the caller cancels it, the running process
 * is killed and its exit is observed *before* the call reports failure. A tool
 * that rejected earlier than that could leave a click, a keystroke or an apt
 * run landing after the model was told the action failed, and a retry would
 * overlap it — the double delivery agent-desktop's retry rules exist to
 * prevent.
 */

/** Budget for one desktop tool call, covering sandbox acquisition, install, desktop start and the command. */
export const CALL_DEADLINE_MS = 300_000;

/** How long a kill and its exit are awaited before the stop is reported as unconfirmed. */
const TERMINATION_TIMEOUT_MS = 2_000;

/**
 * How long an aborted call waits for its work to settle. Long enough to cover a
 * witnessed kill plus its exit, so the reason a call reports is the reason the
 * work itself ended with.
 */
const ABORT_SETTLE_GRACE_MS = TERMINATION_TIMEOUT_MS * 2 + 1_000;

/** A process eve started, as much of `SandboxProcess` as this package uses. */
export interface SandboxProcess {
  readonly stdout: ReadableStream<Uint8Array>;
  readonly stderr: ReadableStream<Uint8Array>;
  wait(): PromiseLike<{ readonly exitCode: number }>;
  kill(): PromiseLike<void>;
}

/**
 * The sandbox capability these tools need. The process handle is required: a
 * command that cannot be killed and watched is not one that can be reported as
 * stopped. Every eve sandbox session provides `spawn`.
 */
export interface CommandSandbox {
  spawn(options: { readonly command: string }): PromiseLike<SandboxProcess>;
}

export interface SandboxCommandResult {
  readonly exitCode?: number;
  readonly stderr?: string;
  readonly stdout?: string;
}

type TerminationOutcome = "stopped" | "unknown";

/**
 * Resolves with `work`, or rejects with the call's abort reason once the signal
 * fires *and* `work` has settled — so a caller never reports failure while the
 * thing it cancelled is still running. Work that ignores the signal is given
 * {@link ABORT_SETTLE_GRACE_MS} before the call gives up on it; a work
 * rejection after the abort is preserved, because it says what happened.
 */
export async function untilSettled<T>(work: PromiseLike<T>, signal: AbortSignal | undefined): Promise<T> {
  if (signal === undefined) return await work;
  const settled = Promise.resolve(work);
  void settled.catch(() => undefined);
  let rejectAborted: (error: unknown) => void = () => {};
  const aborted = new Promise<never>((_resolve, reject) => {
    rejectAborted = reject;
  });
  const onAbort = (): void => {
    void gaveUpAfter(settled, ABORT_SETTLE_GRACE_MS).then(
      () => rejectAborted(abortReason(signal)),
      (error: unknown) => rejectAborted(error instanceof Error ? error : abortReason(signal)),
    );
  };
  signal.addEventListener("abort", onAbort, { once: true });
  if (signal.aborted) onAbort();
  try {
    return await Promise.race([settled, aborted]);
  } finally {
    signal.removeEventListener("abort", onAbort);
  }
}

/**
 * Run `command` and collect its streams and exit code. Rejects with the
 * caller's abort reason once the process is gone, or with a budget error when
 * the call's deadline fired first.
 */
export async function runCommand(
  sandbox: CommandSandbox,
  command: string,
  signal?: AbortSignal,
): Promise<SandboxCommandResult> {
  signal?.throwIfAborted();
  const process = await sandbox.spawn({ command });
  let terminating: Promise<TerminationOutcome> | undefined;
  const stop = (): Promise<TerminationOutcome> => (terminating ??= stopProcess(process));
  let rejectAborted: (error: Error) => void = () => {};
  const aborted = new Promise<never>((_resolve, reject) => {
    rejectAborted = reject;
  });
  const onAbort = (): void => {
    void stop().then((outcome) => rejectAborted(stoppedError(signal, outcome)));
  };
  if (signal !== undefined) {
    signal.addEventListener("abort", onAbort, { once: true });
    if (signal.aborted) onAbort();
  }
  const collected = collectOutput(process);
  void collected.catch(() => undefined);
  try {
    const result = await Promise.race([collected, aborted]);
    // The process may exit from the kill before the abort report is built; the
    // call was still cancelled, so the reason it reports is the abort.
    if (signal?.aborted === true) throw stoppedError(signal, await stop());
    return result;
  } finally {
    if (signal !== undefined) signal.removeEventListener("abort", onAbort);
  }
}

/** Kill the process, then wait for its exit. `unknown` means the stop could not be witnessed. */
async function stopProcess(process: SandboxProcess): Promise<TerminationOutcome> {
  try {
    await withTimeout(Promise.resolve().then(() => process.kill()), TERMINATION_TIMEOUT_MS);
  } catch {
    return "unknown";
  }
  try {
    await withTimeout(process.wait(), TERMINATION_TIMEOUT_MS);
  } catch {
    return "unknown";
  }
  return "stopped";
}

async function collectOutput(process: SandboxProcess): Promise<SandboxCommandResult> {
  const [stdout, stderr, exited] = await Promise.all([
    streamText(process.stdout),
    streamText(process.stderr),
    process.wait(),
  ]);
  return { exitCode: exited.exitCode, stderr, stdout };
}

async function streamText(stream: ReadableStream<Uint8Array>): Promise<string> {
  return await new Response(stream).text();
}

function withTimeout<T>(work: PromiseLike<T>, ms: number): Promise<T> {
  return Promise.race([
    work,
    new Promise<never>((_resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`timed out after ${ms}ms`)), ms);
      (timer as { unref?: () => void }).unref?.();
    }),
  ]);
}

const GAVE_UP = Symbol("gave-up");

/** `work`, or {@link GAVE_UP} when it has not settled within `ms`. */
function gaveUpAfter<T>(work: Promise<T>, ms: number): Promise<T | typeof GAVE_UP> {
  return new Promise<T | typeof GAVE_UP>((resolve, reject) => {
    const timer = setTimeout(() => resolve(GAVE_UP), ms);
    (timer as { unref?: () => void }).unref?.();
    work.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error: unknown) => {
        clearTimeout(timer);
        reject(error instanceof Error ? error : new Error(String(error)));
      },
    );
  });
}

/**
 * The error to report for an aborted call: the caller's own reason, or the
 * budget message when this call's deadline fired.
 */
export function abortReason(signal: AbortSignal | undefined): Error {
  const reason: unknown = signal?.reason;
  if (reason instanceof DOMException && reason.name === "TimeoutError") {
    return new Error(`The desktop tool call ran out of its ${CALL_DEADLINE_MS / 1000}s budget.`);
  }
  if (reason instanceof Error) return reason;
  return new Error("The tool call was cancelled.");
}

function stoppedError(signal: AbortSignal | undefined, termination: TerminationOutcome): Error {
  const reason = abortReason(signal);
  if (termination === "stopped" && !(signal?.reason instanceof DOMException)) return reason;
  const outcome =
    termination === "stopped"
      ? " The running command was stopped, so nothing it started will land."
      : " The running command could not be confirmed stopped, so treat what it was doing as possibly delivered.";
  return new Error(`${reason.message}${outcome}`);
}
