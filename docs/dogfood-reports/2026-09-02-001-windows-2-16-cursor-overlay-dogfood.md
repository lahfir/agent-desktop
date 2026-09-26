# Dogfood: the Windows cursor overlay

- **Date:** 2026-09-02
- **Branch:** `feat/windows-2.16-cursor-overlay`, cut from and merging back into
  `feat/windows-adapter`.
- **Run as a stranger.** The operator was given two things and nothing else: the shipped
  skill package and the built release binary, both staged outside the repository. Reading
  the source tree, the plan or the tests was prohibited, and the prohibition was stated in
  the terms that matter — *if you catch yourself wanting to check the source to explain a
  behaviour, that wanting is the finding.*
- **Channels exercised:** the release binary against Notepad and the machine's real
  desktop, judged from screenshots that were opened and looked at, never from the
  command's own `ok` or `rendered` field.
- **Environment:** Windows Server 2019 Datacenter, build 17763, single 1608×780 display,
  interactive console session.
- **Capture safety:** shapes, counts and colours only — no titles, paths, pids, machine
  names, user names or message text.

## Disposition rule

Every finding takes exactly one of three dispositions, and **"recorded" is not one of
them**:

- **Fixed here** — with a named test that is invert-verified: the fix is broken, the test
  is watched failing, the fix is restored.
- **Owned elsewhere** — written into `docs/phases.md` in this same PR, in enough detail to
  act on without reading this report.
- **Accepted** — with the reason stated.

## Why this run counts

The author had already driven this surface repeatedly and believed it sound. The stranger
found a shipping blocker in its first five commands, using the skill's own example. That
is the whole argument for the rule: author reflexes are what the run exists to strip out.

## Findings

| # | Finding | Severity | Disposition |
|---|---------|----------|-------------|
| D1 | `--label` is ignored; a placeholder greeting is drawn instead | blocker | **Fixed here** |
| D2 | `SNAPSHOT_NOT_FOUND` suggestion sends the caller round the failing loop | functional | **Fixed here** |
| D3 | The label card covers the UI the action just opened | cosmetic | **Owned elsewhere** |
| D4 | At `--size 4.0` the card flips left and sits flush against x=0 | cosmetic | **Owned elsewhere** |
| D5 | Default white card is near-invisible on a white application | cosmetic | **Owned elsewhere** |
| D6 | Parse-time errors report `command: "unknown"` | cosmetic | **Accepted** |
| D7 | Skill frontmatter said 0.8.3; the binary reports 0.8.4 | cosmetic | **Fixed here** |

### D1 — `--label` is ignored; every overlay drew a placeholder. **Blocker.**

Every invocation drew *"Hey, let's play with this computer!"* regardless of `--label`,
including invocations with no label at all. `--max-words` was inert on screen for the same
reason. Reproduced eight times across five independent process launches, with seven opened
4× crops as evidence, and confirmed independently afterwards with a screenshot showing
`SUBMIT THE FORM` on the fix.

This is the worst shape a defect can take. The envelope echoed the caller's own label back
to them — parsed, validated and word-limited — while the screen showed something else, so
no output could reveal it. The card is the entire point of the overlay: it is how a person
watching an agent knows what the agent thinks it is doing, and it was telling every viewer
a placeholder joke. The shipped example in the Windows skill is the exact command that
demonstrates it.

**Cause.** `src/dispatch/cursor_overlay.rs` built the Enable control from the style alone
and dropped the config's label; `CursorOverlayControl::enable` then hardcoded the greeting.
The label pipeline was otherwise sound — a screenshot taken *during an action* showed the
caller's text correctly, alongside a correct highlight ring and ripple.

**Fixed here.** The constructor takes the caller's label, falling back to the greeting only
when none was given. Two tests, and the second exists because the first was not enough: a
core test pins the constructor, but reverting the dispatch call site alone left it green —
the constructor was never what was wrong. `the_callers_label_reaches_the_adapter_rather_than_only_the_envelope`
records the control the adapter is actually handed, which is the only place the label can
be checked, because the envelope echoes it back from the session config either way.
Invert-verified: the call site was reverted and that test, and only that test, failed.

### D2 — the `SNAPSHOT_NOT_FOUND` suggestion misdirects on the failure the overlay causes

A snapshot taken outside a session and acted on inside one fails `SNAPSHOT_NOT_FOUND`, with
a suggestion to re-run the snapshot and retry. Following that literally re-snapshots into
the same global namespace and fails again, forever. The snapshot was seconds old and
perfectly valid; the namespace was the problem, and nothing in the message, the suggestion
or the recovery block mentioned sessions.

This matters more than it looks: enabling the overlay is what drags a session into an
otherwise session-free workflow, so it is the trap this feature creates for the workflow
the docs recommend.

**Fixed here.** The suggestion names the namespace rule. Measured in both directions before
it was written — snapshot-then-session fails with this code, session-then-snapshot succeeds
against the same element — and pinned by
`snapshot_not_found_tells_the_caller_that_snapshots_are_session_scoped`, invert-verified by
removing the session sentence and watching it fail. The ordering is now also stated in the
Windows skill beside the overlay, not only in the general session prose.

### D3, D4, D5 — the label card's placement and default contrast

The card is anchored beside the cursor, and the cursor sits on the element being acted on,
so at the moment of a click on a menu the card covers the menu that just opened. At
`--size 4.0` it flips to the left of the cursor and lands flush against x=0 with no margin
while most of the screen sits empty to the right. And the default white fill leaves it
separated from a white application by only a hairline border.

**Owned elsewhere.** All three share one cause and none is Windows-specific:
`crates/core/src/cursor_overlay/` owns placement and both renderers follow it, so any change
alters macOS — the GA line for the whole platform phase. Written into `docs/phases.md` as
promotion step 6a, to be settled at the one gate where both adapters are reviewed together.

### D6 — parse-time errors report `command: "unknown"`

`cursor-overlay enable --max-words 99` answers `command: "unknown"`, while a semantically
rejected value keeps `command: "cursor-overlay"`.

**Accepted.** This is the documented CLI contract: argument and parse errors exit 2 and are
raised by the argument parser before dispatch resolves a command, so there is no command
name to report. The messages themselves are good — all three rejections named the valid
range or format. Changing it would mean parsing far enough to name a command before
deciding the arguments are invalid.

### D7 — skill version drift

Skill frontmatter declared `0.8.3`; the binary reports `0.8.4`. Trivial, but it is the
first thing a stranger checks when documentation and behaviour disagree — and in this run
they did disagree. **Fixed here**, bumped to `0.8.4`.

## What the run confirmed working

Stated because a report that only lists faults misrepresents the surface.

- The glyph is a correct pointer: filled body, contrasting rim, tip at the upper-left, and
  the tip lands on the element centre (measured tip ≈ (176,33) against an element centre of
  (176.5, 32.5)). No rotation or resize mid-flight.
- Ripple and highlight both draw, and **both suppression flags work in both directions**:
  default gave 30 accent frames with the ripple growing 25×29 → 49×29 then holding;
  `--no-ripple` gave 26 frames opening directly at the steady outline; `--no-highlight`
  gave 4 frames of a growing blob that then vanished; `--headed` gave zero.
- Travel is a clean ease-in over ~280–300 ms, inside the documented 90–320 ms.
- Style flags reach the renderer: `--size` scales the whole assembly, `--fill`/`--rim`
  recolour it.

## What could not be evaluated, and why

- **Motion smoothness was measured but not judged.** A ~64 Hz sampler against a renderer
  running near 60 Hz produces duplicate frames as a beat artefact; two duplicates in twelve
  samples is what that predicts, so calling it stutter would have been naming whichever
  cause happened to be present. A capture rate well above the renderer's would be needed.
- **Teardown was verified numerically, not visually** — 147 consecutive frames over 5.2 s
  with zero overlay pixels and a renderer process count of zero, but no post-teardown image
  was opened. The four live teardown tests and the e2e teardown leg cover it independently.
- **Idle fade at 6 s and revival by the next command** — not exercised.
- **The taskbar-overdraw claim (A29-3)** — not re-exercised here; nothing clickable sat
  under the taskbar. It is measured in the probe corpus.
- **Mixed-DPI and multi-monitor mapping** — single display. Already recorded as unverified
  live (A29-6) and stated as such in the skill.

## Undocumented knowledge the run needed

Each of these is a place the operator had to guess, and each is now documented:

- **Start the session before taking the snapshot** — otherwise the ref is invisible to the
  session-scoped action (D2).
- **Where the cursor rests** before any action moves it: the primary monitor's work-area
  centre, not over the application being driven.
- **That the first frame lands shortly after `enable` returns**, so a screenshot taken
  immediately can miss it.
- **That `--fill` and `--rim` colour the label card too** — body takes the fill, text takes
  the rim, which is why the default white card disappears on a white window (D5).
- **What the overlay costs per action.** Now measured and stated: `+355 ms` (A30-5).

## Cost

Probe corpus methodology — min of seven, warm-up discarded, min reported with median and
max beside it. `scripts/perf-baseline-compare.sh` is not the vehicle: it opens the macOS
`.app` fixture bundle and cannot run here.

| Measurement | min | median | max |
|---|---|---|---|
| One-time `enable` (renderer spawn + window) | 49.9 ms | 52.1 ms | 80.6 ms |
| Headless `click`, no overlay | 427.0 ms | 449.8 ms | 521.4 ms |
| Headless `click`, overlaid | 782.2 ms | 818.0 ms | 875.2 ms |

**Steady-state delta: +355 ms per action.** A first attempt reported +18.5 ms by clicking
one button repeatedly: with the cursor already on the target, a motion with nowhere to go
arrives at once, so that run measured the degenerate case. The targets alternate now and
every click travels. Recorded as ledger row A30-5.
