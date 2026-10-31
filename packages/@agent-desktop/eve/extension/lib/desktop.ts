import {
  AGENT_DESKTOP_VERSION,
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
  readonly command?: string;
  readonly data?: unknown;
  readonly error?: {
    readonly code: string;
    readonly message: string;
    readonly suggestion?: string;
    readonly disposition?: { readonly delivery?: string; readonly retry?: string };
    readonly details?: unknown;
  };
}

/**
 * Upper bound on acquiring the sandbox and on one command. A cold install of
 * the desktop packages can take a while; an unbounded await would leave the
 * turn stuck on a running tool.
 */
const SANDBOX_DEADLINE_MS = 300_000;

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
  const sandbox = await requireSandbox(ctx);
  const signal = ctx.abortSignal;
  const config = extension.config;
  if (config.autoInstall) await ensureInstalled(sandbox, signal);
  if (config.manageDesktop) await ensureDesktop(sandbox, signal);

  const command = [
    desktopEnvPrefix(config.display),
    [config.binary, ...(config.headed ? ["--headed"] : []), ...args].map(quoteShellArg).join(" "),
  ].join("; ");
  const raw = await withDeadline(sandbox.run({ command, signal }), "The desktop command");
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

async function requireSandbox(ctx: DesktopToolContext): Promise<DesktopSandbox> {
  const sandbox = await withDeadline(ctx.getSandbox(), "The sandbox session");
  if (sandbox === null || sandbox === undefined) {
    throw new Error("The desktop tools require an eve sandbox. Configure agent/sandbox.ts in the consuming agent.");
  }
  return sandbox;
}

async function ensureInstalled(sandbox: DesktopSandbox, signal?: AbortSignal): Promise<void> {
  const config = extension.config;
  const installed = await probeVersion(sandbox, signal);
  if (installed !== undefined && matchesPackageMinor(installed)) return;
  // Reinstalling a custom spec or binary would repeat on every call, so report the mismatch instead.
  const userOwned = config.installSpec !== undefined || config.binary !== "agent-desktop";
  if (installed !== undefined && userOwned) throw versionMismatchError(installed);
  await withDeadline(
    installAgentDesktop(sandbox, {
      signal,
      installSpec: config.installSpec,
      installDesktop: config.installDesktop,
      extraPackages: config.extraPackages,
    }),
    "Installing agent-desktop",
  );
  const reinstalled = await probeVersion(sandbox, signal);
  if (reinstalled === undefined || !matchesPackageMinor(reinstalled)) {
    throw versionMismatchError(reinstalled ?? "missing");
  }
}

async function ensureDesktop(sandbox: DesktopSandbox, signal?: AbortSignal): Promise<void> {
  const config = extension.config;
  // One cheap check when the desktop is live; a resumed sandbox loses its processes and starts them again here.
  const ready = await withDeadline(
    sandbox.run({ command: desktopReadyCommand(config.display), signal }),
    "The desktop readiness check",
  );
  if ((ready.exitCode ?? 0) === 0) return;
  await withDeadline(
    startDesktop(sandbox, {
      signal,
      display: config.display,
      width: config.screen.width,
      height: config.screen.height,
    }),
    "Starting the desktop",
  );
}

/** The installed CLI version, or undefined when the binary is missing. */
async function probeVersion(sandbox: DesktopSandbox, signal?: AbortSignal): Promise<string | undefined> {
  const probe = await withDeadline(
    sandbox.run({ command: `${quoteShellArg(extension.config.binary)} --version 2>/dev/null`, signal }),
    "The agent-desktop install probe",
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

async function withDeadline<T>(work: PromiseLike<T>, label: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      work,
      new Promise<never>((_, reject) => {
        timer = setTimeout(
          () => reject(new Error(`${label} did not respond within ${SANDBOX_DEADLINE_MS / 1000}s. Try again shortly.`)),
          SANDBOX_DEADLINE_MS,
        );
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}
