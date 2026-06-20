# End-to-End Tests (real binary vs. real app)

These tests drive the **release `agent-desktop` binary** against a **real macOS
application** and verify every effect by **independent observation** — never the
command's own `ok: true`. A command that returns success without producing the
effect is caught, because each check re-reads the UI (status label, element
value, scroll offset, `list-apps`, …) and asserts the observed `before`/`after`.

This is the layer that mock-adapter unit tests cannot cover: it exercises the
contract against the actual macOS Accessibility API.

## Layout

| Path | Role |
|------|------|
| `tests/fixture-app/AgentDeskFixture.swift` | SwiftUI + AppKit fixture exposing a fixed, diverse AX surface |
| `tests/fixture-app/build.sh` | Compiles the fixture into `build/AgentDeskFixture.app` |
| `tests/e2e/run.sh` | The harness: launches the fixture, runs the binary, asserts by observation |

The fixture is a **principal-engineer-grade slice of real UI**, not a target
tuned to make the CLI pass. It deliberately mixes AX-actionable native AppKit
controls (`NSSlider`, `NSStepper`) with gesture-only and ambiguous patterns. **A
failure here is a finding about the CLI or the harness — never a reason to edit
the fixture so the CLI passes.**

## Running

```bash
cargo build --release            # the harness uses target/release/agent-desktop
bash tests/e2e/run.sh
```

Requirements:

- macOS with **Accessibility permission** granted to the terminal running the
  harness (System Settings → Privacy & Security → Accessibility).
- The harness builds the fixture `.app` automatically if it is missing.
- `--headed` checks move the real cursor; run on a machine where that is OK.

Exit code is `0` when every scenario passes, `1` on any failure (with the
observed values printed inline for each failing check), `2` on setup error.

## What it verifies

- **Observation:** snapshot role diversity, `find` vocabulary + `roles_present`
  hint, textarea→textfield alias.
- **Interaction in BOTH modes (the headless/headed contract):** every ref-action
  command (`click`, `type`, `set-value`, `clear`, `check`, `uncheck`, `select`,
  `set-value` on slider/stepper, `scroll`) is driven **twice** — default
  **headless**, then **`--headed`** — with mode-specific target values so a
  regression in either mode is caught by an independent `before`/`after`
  observation. Headed must not regress the AX path (it is tried first); it only
  adds cursor/physical fallbacks.
- **The headless/headed discriminator:** `double-click` on a gesture-only button
  (no `AXOpen`) **fails closed with `POLICY_DENIED` headless**, and **completes
  with `--headed`** (physical double-click). This is the crisp proof the two
  modes differ and that `--headed` unlocks the physical fallback.
- **Strict resolution:** ambiguous twins do not silently act; a removed element
  fails closed with `STALE_REF`.
- **Reliability core:** `wait` predicates (`enabled`, `actionable`, `visible`,
  `value`), `wait --text` async appearance, skeleton traversal + scoped
  drill-down, session isolation + session-independent explicit snapshots, trace
  JSONL with secret redaction.
- **Surfaces / drag / expand / close:** open a sheet and act inside it,
  source-tracked drag gesture verified by a tracking canvas, expand a
  press-toggled disclosure, `close-app --force` verified via `list-apps`.

## Documented limitations (tracked, not failures)

- **Cross-target native drag-and-drop (`onDrop`)** needs the OS dragging-session
  / pasteboard protocol. Synthetic mouse events route mouse-up to the drag
  origin, so they cannot drop onto a separate native target. Source-tracked
  gestures (and web/Electron mouse-DnD) work; the harness verifies the gesture
  via a tracking canvas.
- **SwiftUI `Slider`/`Stepper`/`DisclosureGroup` are not AX-actionable.** The
  fixture uses native AppKit `NSSlider`/`NSStepper` (which are), so `set-value`
  and `expand` are proven on controls that actually expose the capability.

## Adding a scenario

1. Expose the surface in `AgentDeskFixture.swift` with a stable
   `accessibilityLabel` and a `statictext` status readout that reflects the
   real state (so the harness can observe the effect, not the command's claim).
2. Add a check in `run.sh` using `verify <label> <status> <expected> <subcmd…>`
   (mode-injected) or an explicit `before`/`after` + `assert` block.
3. If the command is a ref action, add it to `interaction_suite` so it is
   covered in **both** headless and headed modes.
