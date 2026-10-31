import {
  AGENT_DESKTOP_VERSION,
  DEFAULT_AGENT_DESKTOP_INSTALL_SPEC,
  desktopEnvPrefix,
  desktopReadyCommand,
  installAgentDesktop,
  quoteShellArg,
  startDesktop,
  type DesktopSandbox,
} from "./sandbox";

import extension from "../extension";

/** Structural subset of eve's tool context; every eve `ctx` satisfies it. */
export interface DesktopToolContext {
  readonly abortSignal?: AbortSignal;
  getSandbox(): PromiseLike<DesktopSandbox | null | undefined>;
}

/** agent-desktop's JSON envelope: one per command on stdout. */
interface Envelope {
  readonly ok: boolean;
  readonly data?: unknown;
  readonly error?: {
    readonly code: string;
    readonly message: string;
    readonly suggestion?: string;
    readonly disposition?: { readonly delivery?: string; readonly retry?: string };
  };
}

/**
 * Budget for one tool call, covering sandbox acquisition, a cold install,
 * desktop start and the command itself. Expiry aborts the running sandbox
 * command, so a timed-out action cannot land after the tool reported failure.
 */
const CALL_DEADLINE_MS = 300_000;

/**
 * Install specs that recently failed in this process. Retrying on every call
 * would repeat a slow apt and npm run, so a failure is reported again until it
 * is a few minutes old.
 */
const failedInstalls = new Map<string, { readonly error: Error; readonly at: number }>();
const INSTALL_RETRY_AFTER_MS = 300_000;

/**
 * Run one agent-desktop command in the eve sandbox and return the `data` of
 * its envelope. Installs the CLI and starts the desktop first when configured.
 * A structured error becomes a thrown Error that keeps the code, suggestion
 * and retry disposition, so the model can decide whether a retry is safe.
 */
export async function runAgentDesktop<TData = unknown>(
  ctx: DesktopToolContext,
  args: readonly string[],
): Promise<TData> {
  const deadline = AbortSignal.timeout(CALL_DEADLINE_MS);
  const signal = ctx.abortSignal ? AbortSignal.any([ctx.abortSignal, deadline]) : deadline;
  const config = extension.config;

  const sandbox = await bounded(ctx.getSandbox(), signal);
  if (sandbox === null || sandbox === undefined) {
    throw new Error("The desktop tools require an eve sandbox. Configure agent/sandbox.ts in the consuming agent.");
  }
  if (config.autoInstall) await ensureInstalled(sandbox, signal);
  if (config.manageDesktop) await ensureDesktop(sandbox, signal);

  const command = [
    desktopEnvPrefix(config.display, config.manageDesktop),
    [config.binary, ...(config.headed ? ["--headed"] : []), ...args].map(quoteShellArg).join(" "),
  ].join("; ");
  const raw = await bounded(sandbox.run({ command, signal }), signal);
  return parseEnvelope<TData>(raw.stdout ?? "", raw.stderr ?? "", args[0] ?? "command");
}

function parseEnvelope<TData>(stdout: string, stderr: string, name: string): TData {
  let envelope: Envelope;
  try {
    envelope = JSON.parse(stdout.trim()) as Envelope;
  } catch {
    throw new Error(`agent-desktop ${name} printed no JSON envelope.\n${(stderr.trim() || stdout.trim()).slice(-2000)}`);
  }
  if (envelope.ok) return (envelope.data ?? null) as TData;
  const error = envelope.error;
  if (error === undefined) throw new Error(`agent-desktop ${name} failed without an error.`);
  const retry = error.disposition?.retry ?? "unknown";
  const delivery = error.disposition?.delivery ?? "unknown";
  throw new Error(
    [
      `agent-desktop ${name} failed: ${error.code}: ${error.message}`,
      error.suggestion ? `Suggestion: ${error.suggestion}` : undefined,
      `Delivery: ${delivery}; retry: ${retry}.${retry === "safe" ? "" : " Do not repeat this action blindly: read the live state first."}`,
    ]
      .filter(Boolean)
      .join("\n"),
  );
}

async function ensureInstalled(sandbox: DesktopSandbox, signal: AbortSignal): Promise<void> {
  const config = extension.config;
  const installed = await probeVersion(sandbox, signal);
  if (installed !== undefined && matchesPackageMinor(installed)) return;

  // A custom binary is the app's to install; never replace it with the default package.
  if (config.binary !== "agent-desktop") {
    throw installed === undefined
      ? new Error(`agent-desktop binary ${config.binary} was not found in the sandbox.`)
      : versionMismatchError(installed);
  }
  // Reinstalling a custom spec would install the same mismatched version again.
  if (installed !== undefined && config.installSpec !== undefined) throw versionMismatchError(installed);

  const spec = config.installSpec ?? DEFAULT_AGENT_DESKTOP_INSTALL_SPEC;
  const failed = failedInstalls.get(spec);
  if (failed !== undefined && Date.now() - failed.at < INSTALL_RETRY_AFTER_MS) throw failed.error;
  try {
    await bounded(
      installAgentDesktop(sandbox, {
        signal,
        installSpec: config.installSpec,
        installDesktop: config.installDesktop,
        extraPackages: config.extraPackages,
      }),
      signal,
    );
    const reinstalled = await probeVersion(sandbox, signal);
    if (reinstalled === undefined || !matchesPackageMinor(reinstalled)) throw versionMismatchError(reinstalled ?? "missing");
    failedInstalls.delete(spec);
  } catch (error) {
    if (!signal.aborted) failedInstalls.set(spec, { error: error as Error, at: Date.now() });
    throw error;
  }
}

async function ensureDesktop(sandbox: DesktopSandbox, signal: AbortSignal): Promise<void> {
  const display = extension.config.display;
  // One cheap check when the desktop is live; a resumed sandbox loses its processes and starts them again here.
  const ready = await bounded(sandbox.run({ command: desktopReadyCommand(display), signal }), signal);
  if ((ready.exitCode ?? 0) === 0) return;
  await bounded(startDesktop(sandbox, { signal, display }), signal);
}

/** The installed CLI version, or undefined when the binary is missing. */
async function probeVersion(sandbox: DesktopSandbox, signal: AbortSignal): Promise<string | undefined> {
  const probe = await bounded(
    sandbox.run({ command: `${quoteShellArg(extension.config.binary)} --version 2>/dev/null`, signal }),
    signal,
  );
  if ((probe.exitCode ?? 0) !== 0) return undefined;
  return (probe.stdout ?? "").trim().split(/\s+/).at(-1) ?? "";
}

/** These tools describe one agent-desktop minor; another minor may take different flags. */
function matchesPackageMinor(version: string): boolean {
  const minor = (v: string) => v.split(".").slice(0, 2).join(".");
  return minor(version) === minor(AGENT_DESKTOP_VERSION);
}

function versionMismatchError(installed: string): Error {
  return new Error(
    `agent-desktop ${installed} in the sandbox does not match @agent-desktop/eve (agent-desktop ${AGENT_DESKTOP_VERSION}). Install agent-desktop@${AGENT_DESKTOP_VERSION} or set installSpec to it.`,
  );
}

/** Rejects as soon as the call's signal aborts, even if the work ignores it. */
async function bounded<T>(work: PromiseLike<T>, signal: AbortSignal): Promise<T> {
  signal.throwIfAborted();
  let onAbort: (() => void) | undefined;
  try {
    return await Promise.race([
      work,
      new Promise<never>((_, reject) => {
        onAbort = () =>
          reject(
            signal.reason instanceof DOMException && signal.reason.name === "TimeoutError"
              ? new Error(`The desktop tool call did not finish within ${CALL_DEADLINE_MS / 1000}s and was stopped.`)
              : signal.reason,
          );
        signal.addEventListener("abort", onAbort, { once: true });
      }),
    ]);
  } finally {
    if (onAbort) signal.removeEventListener("abort", onAbort);
  }
}
