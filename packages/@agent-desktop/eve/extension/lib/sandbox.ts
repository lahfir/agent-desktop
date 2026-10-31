/**
 * Sandbox helpers for running agent-desktop inside an eve sandbox: install the
 * CLI and a minimal X11 desktop, start the display, session bus and window
 * manager, and build the environment every agent-desktop command needs.
 *
 * This file is also published as `@agent-desktop/eve/sandbox`, so it imports
 * nothing from the rest of the extension.
 */

/** The agent-desktop CLI version this package's tools are written against. */
export const AGENT_DESKTOP_VERSION = "1.0.2";

export const DEFAULT_AGENT_DESKTOP_INSTALL_SPEC = `agent-desktop@${AGENT_DESKTOP_VERSION}`;

/** Where the managed desktop keeps its logs, session-bus address and runtime directory. */
export const DESKTOP_ROOT = "/tmp/agent-desktop-eve";

export const DEFAULT_DISPLAY = ":99";

/**
 * apt packages for a headless X11 desktop that agent-desktop can read: Xvfb,
 * a D-Bus session, the AT-SPI accessibility bus and GTK bridges, and openbox
 * as the window manager (window control and focus need one; openbox is the
 * X11 window manager agent-desktop's own Linux tests run against). xterm gives
 * the agent at least one accessible app to open.
 */
export const DESKTOP_APT_PACKAGES = [
  "at-spi2-core",
  "dbus-x11",
  "libatk-adaptor",
  "libgail-common",
  "x11-utils",
  "openbox",
  "x11-xserver-utils",
  "xdotool",
  "xterm",
  "xvfb",
] as const;

const BOOTSTRAP_REVISION = "1";

export interface SandboxCommandResult {
  readonly exitCode?: number;
  readonly stderr?: string;
  readonly stdout?: string;
}

/** Structural subset of eve's sandbox session; every `ctx.getSandbox()` result satisfies it. */
export interface DesktopSandbox {
  run(options: {
    readonly command: string;
    readonly env?: Record<string, string>;
    readonly signal?: AbortSignal;
  }): PromiseLike<SandboxCommandResult>;
}

export interface InstallAgentDesktopOptions {
  readonly signal?: AbortSignal;
  /** npm spec for the CLI, such as a tarball or mirror. Defaults to the version this package targets. */
  readonly installSpec?: string;
  /** Install the Xvfb desktop packages. Turn off when the image already has a desktop session. */
  readonly installDesktop?: boolean;
  /** Extra apt packages, such as the apps the agent should operate (`gnome-calculator`, `mousepad`). */
  readonly extraPackages?: readonly string[];
}

export interface StartDesktopOptions {
  readonly signal?: AbortSignal;
  readonly display?: string;
  readonly width?: number;
  readonly height?: number;
}

/** Include this in the sandbox revalidation key so cached templates rebuild when the install changes. */
export function agentDesktopRevalidationKey(options: InstallAgentDesktopOptions = {}): string {
  return [
    "agent-desktop",
    `bootstrap-${BOOTSTRAP_REVISION}`,
    options.installSpec ?? DEFAULT_AGENT_DESKTOP_INSTALL_SPEC,
    options.installDesktop === false ? "no-desktop" : "desktop",
    ...(options.extraPackages ?? []),
  ].join(":");
}

/** Install the agent-desktop CLI and, by default, the desktop packages. Needs apt and root or passwordless sudo for the desktop. */
export async function installAgentDesktop(
  sandbox: DesktopSandbox,
  options: InstallAgentDesktopOptions = {},
): Promise<void> {
  const packages = [
    ...(options.installDesktop === false ? [] : DESKTOP_APT_PACKAGES),
    ...(options.extraPackages ?? []),
  ];
  if (packages.length > 0) {
    await runOrThrow(sandbox, aptInstallCommand(packages), "Installing desktop packages", options.signal);
  }
  const spec = quoteShellArg(options.installSpec ?? DEFAULT_AGENT_DESKTOP_INSTALL_SPEC);
  await runOrThrow(
    sandbox,
    // A user-writable npm prefix needs no sudo; fall back to sudo for a root-owned one.
    `npm install -g ${spec} || sudo -n npm install -g ${spec}`,
    "Installing agent-desktop",
    options.signal,
  );
}

/**
 * Start the virtual display, a D-Bus session and the window manager. Safe to
 * repeat: each piece starts only when it is not already running. A resumed
 * sandbox keeps files but not processes, so call this again (or let the tools
 * do it) after a resume.
 */
export async function startDesktop(sandbox: DesktopSandbox, options: StartDesktopOptions = {}): Promise<void> {
  await runOrThrow(sandbox, startDesktopScript(options), "Starting the desktop", options.signal);
}

/** A command that exits 0 when the display, session bus and window manager are all live. */
export function desktopReadyCommand(display: string = DEFAULT_DISPLAY): string {
  const d = quoteShellArg(display);
  return [
    `xdpyinfo -display ${d} >/dev/null 2>&1`,
    `DBUS_SESSION_BUS_ADDRESS="$(cat ${DESKTOP_ROOT}/dbus-session-address 2>/dev/null)" dbus-send --session --dest=org.freedesktop.DBus --type=method_call --print-reply / org.freedesktop.DBus.GetId >/dev/null 2>&1`,
    `DISPLAY=${d} xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -Eq 'window id # 0x[1-9a-f]'`,
  ].join(" && ");
}

/**
 * Shell prefix that puts a command inside the managed desktop session. Apps
 * started under it register on the same accessibility bus agent-desktop reads.
 */
export function desktopEnvPrefix(display: string = DEFAULT_DISPLAY): string {
  return [
    `export DISPLAY=${quoteShellArg(display)}`,
    `export XDG_RUNTIME_DIR=${DESKTOP_ROOT}/runtime`,
    `export DBUS_SESSION_BUS_ADDRESS="$(cat ${DESKTOP_ROOT}/dbus-session-address 2>/dev/null)"`,
    "export GTK_MODULES=gail:atk-bridge NO_AT_BRIDGE=0 ACCESSIBILITY_ENABLED=1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1",
  ].join("; ");
}

export function startDesktopScript(options: StartDesktopOptions = {}): string {
  const display = quoteShellArg(options.display ?? DEFAULT_DISPLAY);
  const size = `${options.width ?? 1280}x${options.height ?? 800}x24`;
  const root = DESKTOP_ROOT;
  const wmReady = `DISPLAY=${display} xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -Eq 'window id # 0x[1-9a-f]'`;
  return [
    "set -eu",
    "for tool in Xvfb xdpyinfo dbus-launch openbox; do",
    "  command -v \"$tool\" >/dev/null || { echo \"$tool is missing; run installAgentDesktop first\" >&2; exit 1; }",
    "done",
    `mkdir -p ${root}/runtime && chmod 700 ${root} ${root}/runtime`,
    `export XDG_RUNTIME_DIR=${root}/runtime`,
    // 1. The virtual display.
    `if ! xdpyinfo -display ${display} >/dev/null 2>&1; then`,
    `  setsid -f Xvfb ${display} -screen 0 ${size} -nolisten tcp +extension RANDR > ${root}/xvfb.log 2>&1`,
    "  for _ in $(seq 1 50); do",
    `    xdpyinfo -display ${display} >/dev/null 2>&1 && break`,
    "    sleep 0.1",
    "  done",
    `  xdpyinfo -display ${display} >/dev/null 2>&1 || { cat ${root}/xvfb.log >&2; echo 'Xvfb did not start' >&2; exit 1; }`,
    "fi",
    // 2. One session bus, reused across commands. AT-SPI's launcher is activated on it on first use.
    `if DBUS_SESSION_BUS_ADDRESS="$(cat ${root}/dbus-session-address 2>/dev/null)" dbus-send --session --dest=org.freedesktop.DBus --type=method_call --print-reply / org.freedesktop.DBus.GetId >/dev/null 2>&1; then`,
    `  export DBUS_SESSION_BUS_ADDRESS="$(cat ${root}/dbus-session-address)"`,
    "else",
    `  eval "$(DISPLAY=${display} dbus-launch --sh-syntax)"`,
    `  printf '%s' "$DBUS_SESSION_BUS_ADDRESS" > ${root}/dbus-session-address`,
    `  chmod 600 ${root}/dbus-session-address`,
    "fi",
    // 3. A window manager, so windows can be focused, moved and listed.
    `if ! ${wmReady}; then`,
    `  setsid -f env DISPLAY=${display} DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" openbox > ${root}/openbox.log 2>&1`,
    "  for _ in $(seq 1 100); do",
    `    ${wmReady} && break`,
    "    sleep 0.1",
    "  done",
    `  ${wmReady} || { cat ${root}/openbox.log >&2; echo 'openbox did not claim the display' >&2; exit 1; }`,
    `  DISPLAY=${display} xsetroot -solid '#1f1f1f' 2>/dev/null || true`,
    "fi",
  ].join("\n");
}

function aptInstallCommand(packages: readonly string[]): string {
  const list = packages.map(quoteShellArg).join(" ");
  const install = `apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ${list} && apt-get clean`;
  return [
    "command -v apt-get >/dev/null || { echo 'agent-desktop desktop install needs an apt-based image (Ubuntu 24.04 or later)' >&2; exit 1; }",
    `if [ "$(id -u)" = 0 ]; then sh -c ${quoteShellArg(install)}; else sudo -n sh -c ${quoteShellArg(install)}; fi`,
  ].join("\n");
}

async function runOrThrow(
  sandbox: DesktopSandbox,
  command: string,
  label: string,
  signal?: AbortSignal,
): Promise<SandboxCommandResult> {
  const result = await sandbox.run({ command, signal });
  if ((result.exitCode ?? 0) !== 0) {
    const detail = [result.stderr, result.stdout]
      .map((stream) => (stream ?? "").trim())
      .filter(Boolean)
      .join("\n")
      .slice(-4000);
    throw new Error(`${label} failed (exit ${result.exitCode}): ${detail || "no output"}`);
  }
  return result;
}

export function quoteShellArg(value: string): string {
  return `'${value.replaceAll("'", `'"'"'`)}'`;
}
