---
name: jev-desktop
description: Drive a desktop application from one plain-language goal, or resolve one intent at a time, without ever reading the accessibility tree. Use when automating a desktop app and you want to keep a 150-element JSON tree out of your context. Hand `run.mjs` a whole goal and it observes, decides and acts until the goal is met; hand `act.mjs` a single step when you want to keep the plan yourself. Covers which element, which operation, when to look deeper, and when to stop.
---

# jev-desktop

Two entry points over the same screen reading. `run.mjs` takes a goal and drives
until it is met. `act.mjs` takes one step and hands the plan back to you. Neither
puts the tree in your context.

## The loop

```sh
node scripts/jev/run.mjs --app Finder "open the Applications folder"
node scripts/jev/run.mjs --app TextEdit --cursor --text "A line to write." "write the supplied sentence into the document"
```

Each turn reads the screen, asks for one operation and a target for that
operation in the same request, carries it out, and reads again. It prints one
JSON line per turn and one when it stops.

```json
{"turn":{"step":1,"operation":"CLICK","target":"treeitem \"Applications\"",
         "confidence":1,"ok":true,"delivery":"delivered_verified","changed":true}}
{"stop":"done","confidence":0.96}
```

The operations are `CLICK`, `TYPE_TEXT`, `CHECK`, `UNCHECK`, `EXPAND`,
`COLLAPSE`, `SCROLL`, `DRILL`, `WIDEN`, `WAIT`, `DONE` and `BLOCKED`. Only the
ones something on screen can receive are offered.

**The target is asked once per operation.** `click_target` is chosen from the
elements that advertise `Click`, `type_text_target` from those that advertise
`SetValue`, and so on. Only the head matching the chosen operation is read, so a
target can never be incompatible with the verb that acts on it. That is the
whole reason the loop does not need to correct itself afterwards.

**Looking is an operation.** A window is read with `--skeleton` first, so a
document holding four thousand elements costs the same first look as a panel
holding thirty. A region that was cut off reports how much it holds, and `DRILL`
pins it as the root for later turns. `WIDEN` gives the whole window back.

**`CHECK` and `UNCHECK` instead of a toggle.** Both are idempotent, so a box
already in the wanted state stays there and the policy never reasons about the
current one.

**You supply the text.** Nothing here writes a value, so a run never puts a
string on screen that you did not choose. Pass `--text` once per value and they
are consumed in order, or pass a function as the `text` option and it is asked
for each field with that field's description. When there is no value left to
give, `TYPE_TEXT` is not offered at all, so the run stops rather than inventing
one.

**Text goes in by whichever route the application accepts.** A direct value
write is one verified call; applications that refuse it say so, and only then
does the value go through the clipboard and a paste. The clipboard is put back
when the run ends. One key press per character is never used: it drops
characters and loses capitals.

**What leaves the machine.** Every turn posts a description of the screen to the
decision endpoint (TypeSafe's `api.typesafe.ai` by default, or whatever
`TYPESAFE_BASE_URL` points at): each element's role, its accessible name or description, up
to sixty characters of the value it holds, its state, the window title, and the
recent actions. That is enough to send the contents of a private document or a
filled form. Secure text fields are already withheld by agent-desktop and never
reach the request. Pass `--no-values` to withhold what every other field holds
as well; targeting gets harder, because a value is often the only thing that
tells two unnamed rows apart. Decide this before pointing a run at something
confidential.

**A step that is hard to undo is not taken quietly.** The same request asks how
hard the step would be to reverse. An ordinary step needs 0.70 confidence in its
target, one rated destructive needs 0.90, and below 0.55 nothing runs. When the
bar is not met the run stops and names the candidate it would have acted on, so
you decide instead of it.

**`--cursor` makes the run watchable.** It starts a session, shows a cursor that
travels to each element before the operation lands, and turns it off at the end.
The cursor is drawn where the element is, so bring the application in front of
your terminal or it arrives behind it.

It stops on `DONE`, on `BLOCKED`, on a confidence too low for the risk, after 40
actions, after 80 model calls, or after three turns that changed nothing.

A screen with more actionable elements than a choice can carry says so, in the
request and in the turn it reports, and the policy is told to look inside a
region rather than call the goal impossible.

`--root @ref` starts inside a region when you already know which one.

## One step at a time

You keep the goal, the plan and the memory. You send one sentence. You get back
one small object. The tree never enters your context.

```sh
node scripts/jev/act.mjs --app TextEdit --execute \
  --text "Morning over the dock." "type this text into the main writing area"
```

```json
{
  "ok": true,
  "app": "TextEdit", "window": "Untitled 3", "surface": "sheet",
  "element": { "ref": "@s1:e139", "role": "textfield", "name": "Save As:",
               "where": "sheet \"save\" > group" },
  "command": "set-value",
  "argv": ["set-value", "@s1:e139", "poem.txt"],
  "decision": "act",
  "confidence": { "target": 0.99, "command": 0.88 },
  "gates": { "present": 0.94, "destructive": 0.31, "needs_text": 0.87 },
  "runner_up": [{ "ref": "@s1:e137", "what": "textfield", "p": 0.02 }],
  "notes": [],
  "executed": { "ok": true, "delivery": "delivered_verified" }
}
```

## Calling it

| Flag | Meaning |
| --- | --- |
| `--app <name>` | Required. |
| `--execute` | Run the command when `decision` is `act`. Without it, nothing runs. |
| `--text "…"` | Text the intent needs. Jev returns choices, never strings. |
| `--root @ref` | Resolve inside one container instead of the whole window. |
| `--bin <path>` | agent-desktop binary. Defaults to the release build, then `PATH`. |

**Phrase the intent as an action, not as an element.** `"type this text into the
main writing area"` resolves to `type`. `"the main writing area"` names no
operation and resolved to `focus` in testing. Describe the target the way a
person would — `"the field holding the name the file will be saved under"` —
not the way the tree names it.

## Reading `decision`

| `decision` | What it means | What you do |
| --- | --- | --- |
| `act` | One element clearly matches and the confidence clears the bar for this action's risk. | Nothing. With `--execute` it already ran. |
| `confirm` | The match is plausible but under the bar. | Ask the user, or re-phrase the intent and call again. |
| `abstain` | Jev answered `none`, the element is probably not on this screen, or two elements fit equally. | The screen is not where you think. Open the surface you need, then call again. |
| `needs_text` | The command takes text and none was supplied. | Call again with `--text`. |

`why` always carries the reason in one sentence. `runner_up` shows what else it
considered, which is usually enough to tell a wrong screen from a vague intent.

## What the script decides, not Jev

- **Risk sets the bar.** The answer says what; confidence says whether to act.
  An ordinary action needs 0.70. One that Jev rates `destructive` at 0.50 or
  more needs 0.90 — writing a file, deleting, sending, confirming a warning.
  Below 0.55 nothing acts at all.
- **The command is reconciled against the element.** Both questions are
  answered in parallel and neither sees the other, so code checks the chosen
  verb against the element's advertised actions. A readonly combobox gets
  `click`, never `set-value`. A `--text` payload with a non-text verb corrects
  the verb, because the caller supplying text is evidence Jev does not have.
- **An open surface wins.** When a sheet, alert, menu or popover is up, the
  script reads that surface instead of the window. In the window tree those
  elements carry `offscreen` and would all be dropped.
- **Disabled and hidden elements are never offered.** Everything else is,
  including unnamed rows — a row is told apart by the value it holds, and a
  Choice does better with the full list than with a shortlist.

## How the request is shaped

One call carries five questions. They are answered in parallel, so the
speculative ones cost tokens and no latency.

| Question | Type | Asks |
| --- | --- | --- |
| `target` | choice | Which element the intent refers to, plus `none`. |
| `command` | choice | Which of the 16 interaction verbs it asks for. |
| `present` | noul | Is the thing on this screen at all? |
| `destructive` | noul | Would this be hard to undo? |
| `needs_text` | noul | Does this need text from the caller? |

A Choice accepts 255 options, so up to 254 elements go in one pass. When the
first pass lands under 0.70 the top five are re-asked with richer descriptions.
A screen with more than 254 elements says so in `notes`; use `--root @ref`.

## Deciding through OpenRouter

The decision endpoint is `TYPESAFE_BASE_URL`, defaulting to TypeSafe's native
`https://api.typesafe.ai/v1/systemone`. Point it at OpenRouter's Decisions
router to reuse an OpenRouter key instead of a TypeSafe one (live-tested
against `typesafe/jev-1.13`):

```sh
TYPESAFE_BASE_URL=https://openrouter.ai/api/alpha/decisions \
TYPESAFE_MODEL=typesafe/jev-1.13 \
TYPESAFE_API_KEY=sk-or-... \
node scripts/jev/run.mjs --app Finder "open the Applications folder"
```

`TYPESAFE_MODEL` may stay at its `jev-latest` default — bare names are scoped
through OpenRouter's registered `~typesafe/…` alias — or pin a routed id such
as `typesafe/jev-1.13`. One live-checked trap: the scoped-but-unregistered
`typesafe/jev-latest` answers `400 Model does not exist`. The wire contract is
otherwise the same, so nothing else changes. OpenRouter's Decisions endpoint
is alpha and may change shape.

## Known limits

- One snapshot per call, about 3 s on a dense app. `--root` is far cheaper.
- No retry and no backoff. A 429 ends the call.
- Held input is unavailable, so no sustained key and no drag with hold. See
  `crates/core/src/commands/input_hold_policy.rs`.
- `press` is not resolved here. It needs no element, so send it directly.

## Files

| File | Holds |
| --- | --- |
| `scripts/jev/policy.mjs` | The operations, how a screen is described, and when to stop. No application is touched here. |
| `scripts/jev/desktop.mjs` | The only code that speaks to agent-desktop. |
| `scripts/jev/run.mjs` | The loop and its command line. |
| `scripts/jev/act.mjs` | The single-step resolver. |

## Checks

```sh
node scripts/jev/act.test.mjs
node scripts/jev/run.test.mjs
```
