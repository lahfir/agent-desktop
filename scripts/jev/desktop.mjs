/**
 * jev-desktop executor: the only part that speaks to agent-desktop. Every
 * observation and every mutation goes through this file, so the policy above it
 * never learns what a ref is.
 */
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";

import { collect, offerable, outsideWindow, overlayRole } from "./screen.mjs";
import { ARGV } from "./policy.mjs";

const REPO = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

export const bin =
  process.env.AGENT_DESKTOP_BIN ??
  (existsSync(join(REPO, "target/release/agent-desktop")) ? join(REPO, "target/release/agent-desktop") : "agent-desktop");

export const cli = (...argv) => {
  try {
    return JSON.parse(execFileSync(bin, argv, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 }));
  } catch (error) {
    try {
      return JSON.parse(error.stdout ?? "");
    } catch {
      return { ok: false, error: { code: "SPAWN_FAILED", message: String(error.message ?? error) } };
    }
  }
};

/**
 * A cursor makes the run watchable: it travels to each element before the
 * operation lands. It is drawn where the element is, so the application has to
 * be in front of whatever else is on screen or the cursor arrives behind it.
 * Turning it on also starts the session that carries the trace.
 */
export const startCursor = (label) => {
  const session = cli("session", "start", "--name", "jev-desktop", "--cursor", "--multi-agent");
  if (!session.ok) return null;
  process.env.AGENT_DESKTOP_SESSION = session.data.session_id;
  process.env.AGENT_DESKTOP_AGENT_ID ??= "jev-desktop";
  cli("cursor-overlay", "enable", "--label", label.slice(0, 60));
  return session.data.session_id;
};

export const stopCursor = () => cli("cursor-overlay", "disable");

/**
 * A window is read shallowly until the policy asks to go deeper, so a document
 * holding thousands of elements costs the same first look as a panel holding
 * thirty. A region cut off by that shallow read still reports how much it holds,
 * which is what makes it worth drilling into. A sheet or menu owns the screen
 * while it is up, so it is read instead of the window behind it.
 */
export const observe = (app, root, windowId = null) => {
  const scope = windowId ? ["--app", app, "--window-id", windowId] : ["--app", app];
  const base = ["snapshot", ...scope, "-i", "--compact", "--include-bounds"];
  const unreadable = (what, error) => Object.assign(new Error(`${what} could not be read: ${error?.code}`),
    { code: error?.code ?? null });
  let snap = root ? cli(...base, "--root", root) : cli(...base, "--skeleton");
  if (!snap.ok && root) throw unreadable("that region", snap.error);
  if (snap.ok && root && windowId && outsideWindow(snap.data, windowId)) {
    throw Object.assign(new Error(`that region is not in window ${windowId}`), { code: "ROOT_OUTSIDE_WINDOW" });
  }
  if (!snap.ok && snap.error?.code !== "WINDOW_NOT_FOUND") snap = cli(...base, "--max-depth", "4");
  if (!snap.ok) throw unreadable("the screen", snap.error);
  const surface = root ? null : overlayRole(snap.data.tree);
  if (surface) {
    const scoped = cli("snapshot", ...scope, "--surface", surface, "-i", "--compact", "--include-bounds");
    if (scoped.ok && windowId && outsideWindow(scoped.data, windowId)) {
      throw Object.assign(new Error(`the ${surface} is not in window ${windowId}`), { code: "SURFACE_OUTSIDE_WINDOW" });
    }
    if (scoped.ok) snap = scoped;
  }
  const nodes = offerable(collect(snap.data.tree));
  return {
    nodes,
    screen: { app, window: snap.data.window?.title ?? null, window_id: snap.data.window?.id ?? windowId,
      surface: surface ?? "window", root },
  };
};

/**
 * The clipboard is borrowed, never assumed. It is read only when a paste is
 * about to need it, so a run that never pastes leaves it untouched. What was
 * there is put back if it was text; if it held an image, a file, or nothing that
 * could be read, it is emptied instead, because leaving the run's own string
 * behind is worse than leaving it empty.
 */
export const clipboardGuard = () => {
  let held = null;
  return {
    borrow() {
      if (held) return;
      const read = cli("clipboard-get");
      held = typeof read.data?.text === "string" ? { text: read.data.text } : { unreadable: true };
    },
    restore() {
      if (!held) return;
      if (held.text === undefined) cli("clipboard-clear");
      else cli("clipboard-set", held.text);
      held = null;
    },
  };
};

const verifyWindowMs = (raw) => {
  const value = Number(raw ?? 2000);
  if (!Number.isFinite(value) || value < 0 || value > 60000) {
    throw new Error(`JEV_VERIFY_MS must be a number of milliseconds from 0 to 60000, not ${JSON.stringify(raw)}`);
  }
  return value;
};
const VERIFY_MS = verifyWindowMs(process.env.JEV_VERIFY_MS);
const VERIFY_INTERVAL_MS = 100;

const awaitFieldText = async (ref, text) => {
  const deadline = Date.now() + VERIFY_MS;
  for (;;) {
    const observed = cli("get", ref, "--property", "value");
    if (observed.ok && observed.data?.value === text) return { held: true, observed };
    if (Date.now() >= deadline) return { held: false, observed };
    await sleep(VERIFY_INTERVAL_MS);
  }
};

const verificationFailure = (observed) => ({
  code: "TEXT_VERIFICATION_FAILED",
  message: "after the paste the field does not hold the requested text",
  disposition: { delivery: "delivered_unverified", retry: "unsafe" },
  ...(observed.ok ? {} : { details: { read_error: observed.error ?? null } }),
});

/**
 * Text goes in through whichever route the application accepts. A direct value
 * write is one verified call, and the applications that refuse it report that
 * refusal, so the paste path runs only when it is needed. A paste arrives whole
 * where one key press per character loses characters and capitals.
 */
export const enterText = async ({ app, windowId = null }, node, text, clipboard) => {
  const written = cli("set-value", node.ref_id, text);
  if (written.ok) return { route: "set-value", result: written };
  const failure = written.error;
  if (failure && failure.code !== "SPAWN_FAILED" && !failure.disposition) {
    return { route: "set-value", result: { ok: false, error: {
      code: "BINARY_TOO_OLD",
      message: "this agent-desktop binary reports no error disposition; the paste fallback needs a binary whose failed set-value says whether a retry is safe",
      details: { cause: failure },
    } } };
  }
  if (failure?.disposition?.retry !== "safe") return { route: "set-value", result: written };
  const focused = cli("focus", node.ref_id);
  if (!focused.ok) return { route: "paste", result: focused };
  clipboard.borrow();
  const copied = cli("clipboard-set", text);
  if (!copied.ok) return { route: "paste", result: copied };
  const pasted = cli("press", "cmd+v", "--app", app, ...(windowId ? ["--window-id", windowId] : []));
  if (!pasted.ok) return { route: "paste", result: pasted };
  const { held, observed } = await awaitFieldText(node.ref_id, text);
  if (!held) return { route: "paste", result: { ok: false, error: verificationFailure(observed) } };
  return { route: "paste", result: { ...pasted, data: {
    ...pasted.data, disposition: { delivery: "delivered_verified", retry: "unsafe" },
  } } };
};

export const execute = async (target, operation, node, text, clipboard) => {
  if (operation === "WAIT") {
    await sleep(250);
    return { ok: true, delivery: "waited" };
  }
  if (operation === "DRILL") return { ok: true, delivery: "looked", root: node.ref_id };
  if (operation === "WIDEN") return { ok: true, delivery: "looked", root: null };
  if (operation === "TYPE_TEXT") {
    const { route, result } = await enterText(target, node, text, clipboard);
    return { ...deliveryEvidence(result), route };
  }
  const result = cli(...ARGV[operation](node.ref_id));
  return deliveryEvidence(result);
};

export const deliveryEvidence = (result) => {
  const details = result.data?.details ?? result.error?.details ?? null;
  const after = result.error?.details?.after_action;
  return {
    ok: result.ok,
    delivery: result.data?.disposition?.delivery ?? result.error?.disposition?.delivery ?? null,
    error: result.error ?? null,
    steps: result.data?.steps ?? after?.steps ?? [],
    post_state: result.data?.post_state ?? details?.post_state ?? after?.post_state ?? null,
    details,
  };
};
