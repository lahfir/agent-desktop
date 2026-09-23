import assert from "node:assert/strict";
import { ask, buildRequest, collect, describe, label, offerable, overlayRole, readAnswers, reconcile, route, toArgv } from "./act.mjs";

const S = "s8f3k2p9";
const screen = collect({
  role: "window",
  name: "Untitled",
  children: [
    { ref_id: `@${S}:e1`, role: "textfield", name: "Save As:", value: "Untitled.rtf", available_actions: ["SetValue"] },
    { ref_id: `@${S}:e2`, role: "combobox", name: "File Format:", value: "Rich Text", states: ["readonly"], available_actions: ["Click"] },
    { ref_id: `@${S}:e3`, role: "button", name: "Save", available_actions: ["Click"] },
    { ref_id: `@${S}:e4`, role: "button", name: "Gone", states: ["disabled"], available_actions: ["Click"] },
    { ref_id: `@${S}:e5`, role: "treeitem", name: "Documents", available_actions: ["Click"],
      children: [{ ref_id: `@${S}:e6`, role: "cell", name: "Documents", available_actions: ["Click"] }] },
    { ref_id: `@${S}:e7`, role: "textfield", available_actions: ["TypeText"] },
  ],
});

assert.equal(screen.length, 7);
assert.deepEqual(screen[0].path, ['window "Untitled"']);

const offered = offerable(screen).map((n) => n.ref_id);
assert.ok(!offered.includes(`@${S}:e4`), "a disabled element is never offered");
assert.ok(!offered.includes(`@${S}:e6`), "a cell repeats its treeitem parent");
assert.deepEqual(offered, [`@${S}:e1`, `@${S}:e2`, `@${S}:e3`, `@${S}:e5`, `@${S}:e7`]);

const d = describe(screen[0], false);
assert.equal(d.what, 'textfield "Save As:"');
assert.equal(d.holds, "Untitled.rtf");
assert.equal(d.where, 'window "Untitled"');
assert.equal(describe(screen[0], false).supports, undefined);
assert.ok(describe(screen[0], true).supports.includes("SetValue"), "the rich pass adds supported actions");
assert.equal(label(screen[2]), 'button "Save"');

assert.equal(overlayRole({ role: "window", children: [{ role: "group" }] }), null);
assert.equal(overlayRole({ role: "window", children: [{ role: "group", children: [{ role: "sheet" }] }] }), "sheet");

const req = buildRequest("save the file", { app: "TextEdit", window: "Untitled" }, offerable(screen));
assert.deepEqual(Object.keys(req.questions), ["target", "command", "present", "destructive", "needs_text"]);
assert.equal(Object.keys(req.questions.target.criteria).at(-1), "none");
assert.equal(req.questions.present.type, "noul");
assert.equal(req.questions.target.criteria[`@${S}:e3`].what, 'button "Save"');

const body = (over = {}) => ({
  answers: {
    target: { type: "choice", choice: `@${S}:e3`, confidence: 0.95, probabilities: { [`@${S}:e3`]: 0.95, none: 0.05 } },
    command: { type: "choice", choice: "click", confidence: 0.9 },
    present: { type: "noul", noul: 0.9 },
    destructive: { type: "noul", noul: 0.1 },
    needs_text: { type: "noul", noul: 0.05 },
    ...over,
  },
});
assert.equal(readAnswers(body()).target, `@${S}:e3`);
assert.equal(readAnswers(body()).destructive, 0.1);
assert.ok(readAnswers({}).error);

assert.deepEqual(reconcile("set-value", screen[1]), { verb: "click", corrected: true });
assert.deepEqual(reconcile("click", screen[2]), { verb: "click", corrected: false });
assert.deepEqual(reconcile("set-value", screen[6]), { verb: "type", corrected: true });

assert.deepEqual(reconcile("focus", screen[6], true), { verb: "type", corrected: true });
assert.deepEqual(reconcile("focus", screen[0], true), { verb: "set-value", corrected: true });
assert.deepEqual(reconcile("focus", screen[2], false).verb, "click");

assert.equal(route(readAnswers(body())).decision, "act");
assert.equal(route(readAnswers(body({ target: { type: "choice", choice: "none", confidence: 0.9, probabilities: {} } }))).decision, "abstain");
assert.equal(route(readAnswers(body({ present: { type: "noul", noul: 0.1 } }))).decision, "abstain");
assert.equal(route(readAnswers(body({ target: { type: "choice", choice: `@${S}:e3`, confidence: 0.4, probabilities: {} } }))).decision, "abstain");
assert.equal(route(readAnswers(body({ target: { type: "choice", choice: `@${S}:e3`, confidence: 0.62, probabilities: {} } }))).decision, "confirm");

assert.equal(route(readAnswers(body({ target: { type: "choice", choice: `@${S}:e3`, confidence: 0.8, probabilities: {} } }))).decision, "act");
assert.equal(
  route(readAnswers(body({
    target: { type: "choice", choice: `@${S}:e3`, confidence: 0.8, probabilities: {} },
    destructive: { type: "noul", noul: 0.7 },
  }))).decision,
  "confirm",
);

assert.deepEqual(toArgv("click", `@${S}:e3`, null), ["click", `@${S}:e3`]);
assert.deepEqual(toArgv("set-value", `@${S}:e1`, "poem.txt"), ["set-value", `@${S}:e1`, "poem.txt"]);
assert.deepEqual(toArgv("scroll", `@${S}:e1`, null), ["scroll", `@${S}:e1`, "--direction", "down"]);
assert.deepEqual(toArgv("hover", `@${S}:e3`, null), ["--headed", "hover", `@${S}:e3`]);

{
  const saved = { ...process.env };
  const realFetch = globalThis.fetch;
  const endpoint = "http://127.0.0.1:9/decisions";
  process.env.TYPESAFE_BASE_URL = endpoint;
  process.env.TYPESAFE_API_KEY = "test-key";
  const sent = [];
  globalThis.fetch = async (url, init) => {
    sent.push({ url, auth: init.headers.Authorization, body: JSON.parse(init.body) });
    return {
      ok: true,
      status: 200,
      json: async () => ({
        answers: {
          target: { type: "choice", choice: "1", confidence: 0.9 },
          command: { type: "choice", choice: "CLICK", confidence: 0.9 },
        },
      }),
    };
  };
  try {
    const answer = await ask(req);
    assert.deepEqual(sent.map((call) => call.url), [endpoint], "act asks the configured endpoint");
    assert.equal(sent[0].auth, "Bearer test-key", "the key travels with the question");
    assert.deepEqual(sent[0].body, JSON.parse(JSON.stringify(req)), "act posts the request it built");
    assert.equal(answer.command, "CLICK", "the endpoint's answer is read back");
  } finally {
    globalThis.fetch = realFetch;
    for (const k of Object.keys(process.env)) if (!(k in saved)) delete process.env[k];
    Object.assign(process.env, saved);
  }
}

console.log("ok");
