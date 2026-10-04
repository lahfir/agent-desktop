#!/usr/bin/env node
/**
 * jev-desktop: one goal in, a desktop driven to it.
 *
 *   node scripts/jev/run.mjs --app Finder "open the Applications folder"
 *
 * The caller states a goal once. Each turn reads the live accessibility tree,
 * asks for one operation and a target for that operation, carries it out, and
 * reads again. The tree never reaches the caller.
 */
import { fileURLToPath } from "node:url";

import { describe } from "./screen.mjs";
import { clipboardGuard, execute, observe, startCursor, stopCursor } from "./desktop.mjs";
import {
  TERMINALS,
  typesafeApi,
  actionSpace,
  buildRequest,
  criterion,
  fingerprint,
  needsRiskCheck,
  riskRequest,
  route,
  shouldStop,
  textSupply,
  validateChoice,
} from "./policy.mjs";


const post = async (url, key, body) => {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const res = await fetch(url, {
      method: "POST",
      headers: { Authorization: `Bearer ${key}`, "Content-Type": "application/json" },
      body: JSON.stringify(body),
    }).catch(() => null);
    if (!res) throw new Error("the model could not be reached; nothing ran");
    if ([429, 503, 529].includes(res.status) && attempt < 2) {
      await new Promise((r) => setTimeout(r, 500 * 2 ** attempt));
      continue;
    }
    if (!res.ok) {
      const detail = await res.text().catch(() => "");
      throw new Error(`the model returned HTTP ${res.status}; nothing ran. ${detail.slice(0, 400)}`);
    }
    return res.json();
  }
  throw new Error("the model stayed unavailable");
};

export const decide = (goal, screen, space, history, values) =>
  post(typesafeApi(), process.env.TYPESAFE_API_KEY, buildRequest(goal, screen, space, history, { values }));

export const rateRisk = (goal, screen, operation, node, values) =>
  post(typesafeApi(), process.env.TYPESAFE_API_KEY, riskRequest(goal, screen, operation, node, { values }));

const WINDOW_STOPS = {
  WINDOW_NOT_FOUND: "window_closed",
  SURFACE_OUTSIDE_WINDOW: "surface_outside_window",
  ROOT_OUTSIDE_WINDOW: "root_outside_window",
};

const windowStop = (error, windowId) => (windowId ? WINDOW_STOPS[error.code] ?? null : null);

export const run = async function* (
  goal,
  app,
  { root = null, windowId = null, text = null, cursor = false, values = true } = {},
) {
  if (!process.env.TYPESAFE_API_KEY) throw new Error("TYPESAFE_API_KEY unset");
  const supply = textSupply(text);
  const session = cursor ? startCursor(goal) : null;
  const clipboard = clipboardGuard();
  const state = { steps: 0, calls: 0, history: [], operation: null, root };
  try {
    let last = null;
    for (;;) {
      let seen;
      try {
        seen = observe(app, state.root, windowId);
      } catch (error) {
        const stop = windowStop(error, windowId);
        if (!stop) throw error;
        yield { stop, screen: last, history: state.history };
        return;
      }
      const { nodes, screen } = seen;
      last = screen;
      const before = fingerprint(nodes);
      const space = actionSpace(nodes, { drillable: !state.root, typable: supply.available(), values });
      if (!space.elements.length) {
        yield { stop: "nothing on this screen can be acted on", screen };
        return;
      }
      const answers = (await decide(goal, screen, space, state.history, values)).answers;
      state.calls += 1;
      const options = [
        ...Object.keys(space.targets),
        ...(state.root ? ["WIDEN"] : []),
        ...Object.keys(TERMINALS),
      ];
      state.operation = validateChoice(answers.operation, options).choice;
      const stop = shouldStop(state);
      if (stop) {
        yield {
          stop,
          screen,
          why: answers.operation.probabilities,
          confidence: answers.operation.confidence,
          history: state.history,
        };
        return;
      }
      let node = null;
      let confidence = answers.operation.confidence;
      if (space.targets[state.operation]) {
        const head = validateChoice(
          answers[`${state.operation.toLowerCase()}_target`],
          Object.keys(space.targets[state.operation]),
        );
        node = space.targets[state.operation][head.choice];
        confidence = head.confidence;
        let destructive = null;
        if (needsRiskCheck(confidence)) {
          const rated = await rateRisk(goal, screen, state.operation, node, values);
          state.calls += 1;
          const answer = rated.answers?.destructive?.noul;
          destructive = typeof answer === "number" ? answer : 1;
        }
        const settled = route({
          target: head.choice,
          targetConfidence: confidence,
          present: null,
          destructive,
        });
        if (settled.decision !== "act") {
          yield {
            stop: `${settled.decision}: ${settled.why}`,
            screen,
            candidate: { operation: state.operation, target: criterion(node, head.choice), confidence },
            history: state.history,
          };
          return;
        }
      }
      let value = null;
      if (state.operation === "TYPE_TEXT") {
        value = await supply.take(describe(node, true), state.history);
        if (typeof value !== "string" || !value.trim()) {
          yield { stop: `no value was supplied for ${criterion(node, "?")}`, screen, history: state.history };
          return;
        }
      }
      const outcome = await execute(app, state.operation, node, value, clipboard, windowId);
      state.steps += 1;
      const turn = {
        step: state.steps,
        operation: state.operation,
        target: node ? `${node.role}${node.name ? ` "${node.name}"` : ""}` : null,
        ref: node?.ref_id ?? null,
        text: value,
        confidence,
        ok: outcome.ok,
        delivery: outcome.delivery,
        error: outcome.error ?? null,
        route: outcome.route ?? null,
        truncated: space.truncated,
        changed: null,
      };
      state.history.push(turn);
      if (!outcome.ok) {
        yield { turn, screen };
        yield { stop: "action_failed", screen, error: outcome.error, history: state.history };
        return;
      }
      if ("root" in outcome) state.root = outcome.root;
      let after;
      try {
        after = "root" in outcome ? null : observe(app, state.root, windowId);
      } catch (error) {
        const stop = windowStop(error, windowId);
        if (!stop) throw error;
        turn.changed = true;
        yield { turn, screen };
        yield { stop, screen, history: state.history };
        return;
      }
      turn.changed = "root" in outcome || fingerprint(after.nodes) !== before;
      yield { turn, screen };
      const settled = shouldStop(state);
      if (settled) {
        yield { stop: settled, screen, history: state.history };
        return;
      }
    }
  } finally {
    clipboard.restore();
    if (session) stopCursor();
  }
};

const main = async (argv) => {
  const flag = (name) => {
    const at = argv.indexOf(`--${name}`);
    if (at === -1) return null;
    const value = argv[at + 1];
    if (value === undefined || value.startsWith("--")) {
      console.error(`--${name} needs a value`);
      process.exit(2);
    }
    return value;
  };
  const app = flag("app");
  const root = flag("root");
  const windowId = flag("window-id");
  const text = argv.flatMap((a, i) => (argv[i - 1] === "--text" ? [a] : []));
  const cursor = argv.includes("--cursor");
  const values = !argv.includes("--no-values");
  const goal = argv
    .filter((a, i) => !a.startsWith("--") && !(argv[i - 1]?.startsWith("--") && argv[i - 1] !== "--cursor" && argv[i - 1] !== "--no-values"))
    .join(" ");
  if (!app || !goal) {
    console.error(
      'usage: run.mjs --app <name> [--window-id <id>] [--cursor] [--no-values] [--root @ref] [--text "value"]... "<goal>"',
    );
    process.exit(2);
  }
  for await (const event of run(goal, app, { root, windowId, text, cursor, values })) {
    console.log(JSON.stringify(event));
    if (event.stop && event.stop !== "done") process.exitCode = 1;
  }
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  await main(process.argv.slice(2)).catch((error) => {
    console.log(JSON.stringify({ ok: false, error: String(error.message ?? error) }));
    process.exitCode = 1;
  });
}
