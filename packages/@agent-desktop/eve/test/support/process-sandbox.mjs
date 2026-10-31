// A sandbox backed by real processes on this machine, shaped like eve's sandbox
// session so the same tool code runs against it. `kill` resolves once the kill
// is dispatched, not once the process is gone — the same contract eve's
// provider gives — so `wait` stays the only evidence of a real exit.
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";

export function createProcessSandbox() {
  const children = [];
  return {
    children,
    async spawn({ command }) {
      const child = spawn("bash", ["-lc", command], { stdio: ["ignore", "pipe", "pipe"] });
      children.push(child);
      let exited;
      const wait = () => {
        exited ??= new Promise((resolve) => {
          if (child.exitCode !== null || child.signalCode !== null) {
            resolve({ exitCode: child.exitCode ?? 1 });
            return;
          }
          child.once("close", (code) => resolve({ exitCode: code ?? 1 }));
        });
        return exited;
      };
      return {
        stdout: child.stdout,
        stderr: child.stderr,
        wait,
        async kill() {
          child.kill("SIGKILL");
        },
      };
    },
    async readBinaryFile({ path }) {
      try {
        return readFileSync(path);
      } catch {
        return null;
      }
    },
  };
}
