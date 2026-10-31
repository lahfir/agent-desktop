// End-to-end check against a real desktop in this machine, the same code path a
// tool takes inside an eve sandbox. Opt in with AGENT_DESKTOP_LIVE=1 on an
// Ubuntu 24.04+ host with passwordless sudo; it installs packages.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";

globalThis[Symbol.for("eve.ext-config-scope")] = "@agent-desktop/eve.live";

const live = process.env.AGENT_DESKTOP_LIVE === "1";

test("drives gnome-calculator through the desktop tools", { skip: !live, timeout: 600_000 }, async () => {
  const { default: extension } = await import("../dist/index.mjs");
  const tools = await import("../dist/tools/index.mjs");
  extension({ extraPackages: ["gnome-calculator"] });

  const sandbox = {
    async run({ command }) {
      const result = spawnSync("bash", ["-c", command], { encoding: "utf8" });
      return { exitCode: result.status ?? 1, stdout: result.stdout, stderr: result.stderr };
    },
    async readBinaryFile({ path }) {
      return readFileSync(path);
    },
  };
  const ctx = { getSandbox: async () => sandbox };

  const launched = await tools.launch.execute({ app: "gnome-calculator", activate: true }, ctx);
  assert.equal(launched.app, "gnome-calculator");

  const press = async (name) => {
    const found = await tools.find.execute({ app: "gnome-calculator", role: "button", name, exact: true, first: true }, ctx);
    await tools.click.execute({ ref: found.match.ref_id }, ctx);
    await tools.wait.execute({ ms: 300 }, ctx);
  };
  for (const key of ["7", "×", "6", "="]) await press(key);

  const tree = JSON.stringify(await tools.snapshot.execute({ app: "gnome-calculator", interactiveOnly: false, compact: true }, ctx));
  assert.match(tree, /42/);

  const shot = await tools.screenshot.execute({ app: "gnome-calculator" }, ctx);
  assert.ok(shot.imageDataUrl?.startsWith("data:image/png;base64,"));

  await tools.close_app.execute({ app: "gnome-calculator" }, ctx);
});
