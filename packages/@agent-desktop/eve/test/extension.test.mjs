import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import test from "node:test";

// The eve runtime sets this ambient scope while it loads an extension's modules so
// `defineExtension` can bind config; emulate it before importing the built dist.
globalThis[Symbol.for("eve.ext-config-scope")] = "@agent-desktop/eve.test";

const { default: extension } = await import("../dist/index.mjs");
const tools = await import("../dist/tools/index.mjs");
const { AGENT_DESKTOP_VERSION } = await import("../dist/sandbox/sandbox.js");

const OK = (data, command = "x") => JSON.stringify({ version: "2.5", ok: true, command, data });

/** A sandbox where agent-desktop is installed, the desktop is up, and every command succeeds. */
function fakeSandbox({ respond, version = AGENT_DESKTOP_VERSION, desktopUp = true } = {}) {
  const commands = [];
  return {
    commands,
    async run({ command }) {
      commands.push(command);
      if (command.endsWith(" --version 2>/dev/null")) {
        return version === undefined
          ? { exitCode: 127, stdout: "", stderr: "" }
          : { exitCode: 0, stdout: `agent-desktop ${version}\n`, stderr: "" };
      }
      if (command.startsWith("xdpyinfo")) return { exitCode: desktopUp ? 0 : 1, stdout: "", stderr: "" };
      const result = respond?.(command);
      return { exitCode: 0, stdout: OK({}), stderr: "", ...result };
    },
  };
}

const ctx = (sandbox) => ({ getSandbox: async () => sandbox });

/** The argv after the binary in the last agent-desktop command. */
function lastArgv(sandbox) {
  const command = sandbox.commands.at(-1);
  const invocation = command.slice(command.lastIndexOf("; ") + 2);
  return [...invocation.matchAll(/'((?:[^']|'"'"')*)'/g)].map((m) => m[1].replaceAll(`'"'"'`, "'"));
}

function resetConfig(overrides = {}) {
  extension(overrides);
}

test("ships the expected tools and defers the advanced ones", () => {
  const files = readdirSync(new URL("../extension/tools", import.meta.url)).map((f) => f.replace(/\.ts$/, ""));
  assert.deepEqual(Object.keys(tools).sort(), files.sort());
  for (const name of ["snapshot", "find", "click", "type", "press", "launch", "screenshot", "wait", "list_apps"]) {
    assert.ok(name in tools, `missing ${name}`);
    assert.notEqual(tools[name].deferred, true, `${name} should be listed up front`);
  }
  for (const name of ["drag", "hover", "mouse_click", "clipboard_get", "resize_window", "status"]) {
    assert.equal(tools[name].deferred, true, `${name} should be deferred`);
  }
});

test("instructions name only tools the extension ships", () => {
  const text = readFileSync(new URL("../extension/instructions.md", import.meta.url), "utf8");
  for (const [, name] of text.matchAll(/`([a-z_]+)`/g)) {
    if (["eve__search", "root", "skeleton", "return", "mouse_"].includes(name) || name.endsWith("_")) continue;
    assert.ok(name in tools, `instructions mention missing tool ${name}`);
  }
});

test("click runs one agent-desktop command inside the desktop session and returns data", async () => {
  resetConfig();
  const sandbox = fakeSandbox({ respond: () => ({ stdout: OK({ action: "click" }) }) });
  const result = await tools.click.execute({ ref: "@s1:e2" }, ctx(sandbox));
  assert.deepEqual(result, { action: "click" });
  const command = sandbox.commands.at(-1);
  assert.match(command, /^export DISPLAY=':99'; /);
  assert.match(command, /DBUS_SESSION_BUS_ADDRESS="\$\(cat \/tmp\/agent-desktop-eve\/dbus-session-address/);
  assert.deepEqual(lastArgv(sandbox), ["agent-desktop", "click", "@s1:e2"]);
});

test("snapshot maps camelCase input to CLI flags", async () => {
  resetConfig();
  const sandbox = fakeSandbox();
  await tools.snapshot.execute(
    { app: "gnome-calculator", interactiveOnly: true, compact: true, skeleton: true, maxDepth: 4 },
    ctx(sandbox),
  );
  assert.deepEqual(lastArgv(sandbox), [
    "agent-desktop",
    "snapshot",
    "--app=gnome-calculator",
    "--skeleton",
    "--interactive-only",
    "--compact",
    "--max-depth=4",
  ]);
});

test("text with quotes and leading dashes reaches the CLI intact", async () => {
  resetConfig();
  const sandbox = fakeSandbox();
  await tools.type.execute({ ref: "@s1:e4", text: "it's -5 $HOME" }, ctx(sandbox));
  assert.deepEqual(lastArgv(sandbox), ["agent-desktop", "type", "@s1:e4", "it's -5 $HOME"]);
});

test("negative coordinates use = so clap does not read them as flags", async () => {
  resetConfig();
  const sandbox = fakeSandbox();
  await tools.move_window.execute({ app: "xterm", x: -100, y: 20 }, ctx(sandbox));
  assert.deepEqual(lastArgv(sandbox), ["agent-desktop", "move-window", "--app=xterm", "--x=-100", "--y=20"]);
});

test("headed config adds the global --headed flag", async () => {
  resetConfig({ headed: true });
  const sandbox = fakeSandbox();
  await tools.double_click.execute({ ref: "@s1:e2" }, ctx(sandbox));
  assert.deepEqual(lastArgv(sandbox), ["agent-desktop", "--headed", "double-click", "@s1:e2"]);
  resetConfig();
});

test("structured errors keep the code, suggestion and retry disposition", async () => {
  resetConfig();
  const error = {
    code: "STALE_REF",
    message: "@s1:e7 not found",
    suggestion: "Re-run a snapshot",
    disposition: { delivery: "not_delivered", retry: "safe" },
  };
  const sandbox = fakeSandbox({
    respond: () => ({ exitCode: 1, stdout: JSON.stringify({ version: "2.5", ok: false, command: "click", error }) }),
  });
  await assert.rejects(() => tools.click.execute({ ref: "@s1:e7" }, ctx(sandbox)), (err) => {
    assert.match(err.message, /STALE_REF: @s1:e7 not found/);
    assert.match(err.message, /Suggestion: Re-run a snapshot/);
    assert.match(err.message, /retry: safe\.$/);
    return true;
  });
});

test("an unsafe failure warns against repeating the action", async () => {
  resetConfig();
  const error = { code: "TIMEOUT", message: "slow", disposition: { delivery: "unknown", retry: "unknown" } };
  const sandbox = fakeSandbox({ respond: () => ({ exitCode: 1, stdout: JSON.stringify({ ok: false, error }) }) });
  await assert.rejects(() => tools.click.execute({ ref: "@s1:e7" }, ctx(sandbox)), /Do not repeat this action blindly/);
});

test("starts the desktop when it is not running", async () => {
  resetConfig();
  const sandbox = fakeSandbox({ desktopUp: false });
  await tools.list_apps.execute({}, ctx(sandbox));
  assert.ok(sandbox.commands.some((command) => command.includes("setsid -f Xvfb ':99' -screen 0 1280x800x24")));
});

test("manageDesktop false leaves the desktop alone", async () => {
  resetConfig({ manageDesktop: false, display: ":0" });
  const sandbox = fakeSandbox({ desktopUp: false });
  await tools.list_apps.execute({}, ctx(sandbox));
  assert.ok(!sandbox.commands.some((command) => command.startsWith("xdpyinfo") || command.includes("Xvfb")));
  assert.match(sandbox.commands.at(-1), /^export DISPLAY=':0'; /);
  resetConfig();
});

test("auto-installs the CLI and desktop packages when the binary is missing", async () => {
  resetConfig({ extraPackages: ["gnome-calculator"] });
  const versions = [undefined, AGENT_DESKTOP_VERSION];
  const sandbox = fakeSandbox();
  const run = sandbox.run;
  sandbox.run = async (options) => {
    if (options.command.endsWith(" --version 2>/dev/null")) {
      sandbox.commands.push(options.command);
      const version = versions.shift();
      return version === undefined
        ? { exitCode: 127, stdout: "", stderr: "" }
        : { exitCode: 0, stdout: `agent-desktop ${version}\n`, stderr: "" };
    }
    return run(options);
  };
  await tools.list_apps.execute({}, ctx(sandbox));
  assert.ok(sandbox.commands.some((command) => command.includes("'xvfb'") && command.includes("'gnome-calculator'")));
  assert.ok(sandbox.commands.some((command) => command.includes(`npm install -g 'agent-desktop@${AGENT_DESKTOP_VERSION}'`)));
  resetConfig();
});

test("does not reinstall a custom installSpec on a version mismatch", async () => {
  resetConfig({ installSpec: "agent-desktop@0.9.0" });
  const sandbox = fakeSandbox({ version: "0.9.0" });
  await assert.rejects(() => tools.list_apps.execute({}, ctx(sandbox)), /agent-desktop 0\.9\.0 in the sandbox does not match/);
  assert.ok(!sandbox.commands.some((command) => command.includes("npm install -g")));
  resetConfig();
});

test("screenshot writes to a private path and hands the model pixels, not the path", async () => {
  resetConfig();
  const png = Buffer.from([0x89, 0x50, 0x4e, 0x47]);
  const sandbox = fakeSandbox({
    respond: (command) => {
      const path = /'(\/tmp\/agent-desktop-eve\/screenshots\/[^']+\.png)'/.exec(command)?.[1];
      return { stdout: OK({ format: "png", path, width: 370, height: 616 }) };
    },
  });
  sandbox.readBinaryFile = async () => png;
  const output = await tools.screenshot.execute({ app: "gnome-calculator" }, ctx(sandbox));
  assert.match(output.path, /^\/tmp\/agent-desktop-eve\/screenshots\/.+\.png$/);
  assert.equal(output.imageDataUrl, `data:image/png;base64,${png.toString("base64")}`);
  const model = await tools.screenshot.toModelOutput(output);
  assert.equal(model.type, "content");
  assert.ok(!JSON.stringify(model).includes("/tmp/agent-desktop-eve"));
});
