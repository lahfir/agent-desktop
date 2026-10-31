// The process runner against real processes: the stop a cancelled call reports
// has to be a stop that happened, not one that was requested.
import assert from "node:assert/strict";
import test from "node:test";

import { createProcessSandbox } from "./support/process-sandbox.mjs";

const { runCommand } = await import("../dist/sandbox/process.js");
const tick = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const gone = (child) => child.exitCode !== null || child.signalCode !== null;

test("collects stdout, stderr and the exit code of a finished command", async () => {
  const sandbox = createProcessSandbox();
  const result = await runCommand(sandbox, "echo out; echo err >&2; exit 7");
  assert.equal(result.exitCode, 7);
  assert.equal(result.stdout?.trim(), "out");
  assert.equal(result.stderr?.trim(), "err");
});

test("a cancelled command is dead before the call rejects", async () => {
  const sandbox = createProcessSandbox();
  const controller = new AbortController();
  const call = runCommand(sandbox, "sleep 30", controller.signal);
  await tick(50);
  controller.abort(new Error("cancelled mid-flight"));
  await assert.rejects(call, /cancelled mid-flight/);
  assert.ok(gone(sandbox.children.at(-1)), "the call reported failure while its command was still alive");
});

test("a command that finishes before the abort still reports the cancellation", async () => {
  const sandbox = createProcessSandbox();
  const controller = new AbortController();
  const call = runCommand(sandbox, "exit 0", controller.signal);
  controller.abort(new Error("cancelled late"));
  await assert.rejects(call, /cancelled late/);
  assert.ok(gone(sandbox.children.at(-1)));
});

test("a signal that fires before the command starts never runs it", async () => {
  const sandbox = createProcessSandbox();
  const controller = new AbortController();
  controller.abort(new Error("cancelled first"));
  await assert.rejects(() => runCommand(sandbox, "touch /tmp/agent-desktop-eve-should-not-exist", controller.signal), /cancelled first/);
  assert.equal(sandbox.children.length, 0);
});
