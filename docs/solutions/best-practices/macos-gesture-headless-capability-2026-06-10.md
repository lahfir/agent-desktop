---
title: Which desktop gestures have a headless path (macOS) and why the command never decides
date: 2026-06-10
category: best-practices
module: crates/macos
problem_type: best_practice
component: macos-adapter
severity: high
applies_when:
  - Implementing or using a ref-action command and deciding whether it needs --headed
  - A headless ref action returns POLICY_DENIED and you are unsure if that is correct
  - Adding a Windows (UIA) or Linux (AT-SPI) adapter and mapping gesture commands
  - Writing an E2E that drives a gesture and must assert the real effect, not ok:true
tags:
  - interaction-policy
  - headless-headed
  - accessibility
  - macos
  - platform-adapter
  - swiftui
---

# Which desktop gestures have a headless path (macOS) and why the command never decides

## Context

Ref actions run in two modes (Playwright-style): **headless** (default — accessibility-only, no cursor, fail closed with `POLICY_DENIED`) and **headed** (`--headed` — permits cursor movement and focus stealing so physical fallbacks can complete). A recurring question when using or extending the tool is: *which* interactions actually work headlessly, and which inherently need `--headed`? Getting this wrong leads to either surprise `POLICY_DENIED` errors or the false assumption that a command physically interacted when it only did an AX no-op.

The answer is **per-gesture and per-platform**, because a gesture is headless-capable only when the OS accessibility API exposes a semantic action for it. On macOS the reality is:

| Gesture / control | Headless path (macOS) | Notes |
|-------------------|-----------------------|-------|
| `click`, `set-value`, `type`, `check`, `select`, `scroll`, `expand`, `toggle`, … | yes | semantic AX actions; the default and most reliable surface |
| `double-click` | partial | `AXOpen` works headless on elements that advertise it (Finder/list/outline rows, table cells). Falls back to `--headed` only for gesture-only targets with no `AXOpen`. |
| `triple-click` | no | macOS exposes no triple-click action; it is purely 3 physical clicks → `--headed` only |
| `hover` | no | hovering *is* moving the cursor over an element; no AX equivalent |
| `drag` / drop | no | dragging *is* a cursor press-move-release; no general AX drag. Native cross-app drop needs the OS `NSDraggingSession`/pasteboard protocol that synthetic CGEvents cannot start (works for same-view source-tracked gestures and web/Electron mouse-DnD) |
| menu bar | enumerate / open | readable and openable via `snapshot --surface menubar`; **SwiftUI `CommandMenu` items accept AXPress but do not route to their action closure** (a SwiftUI limitation, like its `Slider`) — native AppKit menu items fire. `.contextMenu` item selection works. |
| SwiftUI `Slider` / `Stepper` / `DisclosureGroup` | no | not AX-actionable; the native AppKit `NSSlider`/`NSStepper` equivalents are (so `set-value`/`expand` work on those) |

## Guidance

1. **The command is platform-agnostic; the adapter owns headless-vs-physical.** A ref-action command builds an `Action` (e.g. `Action::TripleClick`) and calls `adapter.execute_action`. The macOS adapter's dispatch decides how to perform it (AX action vs policy-gated CGEvent). Core never encodes platform behavior — it cannot, because core may never import a platform crate (CI enforces this with `cargo tree -p agent-desktop-core`).

2. **A new platform that exposes a headless path lights it up automatically — adapter-only change.** If a future Windows (UIA) or Linux (AT-SPI) adapter has a headless action for `double-click`/`triple-click`, it maps the `Action` there and the command succeeds headlessly on that platform with **zero change to the command or core**. The `InteractionPolicy` flows through the request; each adapter honors it per its own capabilities. The agent just sees success (or `POLICY_DENIED` → retry `--headed`) — it never needs to know the platform.

3. **`hover`/`drag`/`mouse-*` are modeled as raw cursor gestures, not semantic `Action`s** (they call `adapter.mouse_event`/`adapter.drag` with coordinates). They stay physical on every platform by design, because hovering/dragging *are* cursor operations universally. A semantic drag (AX reorder) would be a *new* `Action`, not a change to `drag`. Headless gestures return `POLICY_DENIED` before resolving refs or moving the cursor. Under `--headed`, ref-addressed gestures first ensure the target app is frontmost (`focus_for_physical_input`, gated on `InteractionPolicy::allow_focus_steal`; the response reports `"focused": true` when confirmed — already-frontmost apps skip the raise). `--xy` input never focuses — the caller owns the target there. The chain's physical click fallback goes one step further than the gesture focus path: it also raises the target element's **own window** (AXRaise, AXMain fallback) before posting events, because CGEvents land on the topmost window at the click point and app-frontmost alone is not enough when the element lives in a background window of that app.

4. **`POLICY_DENIED` on a headless gesture is correct, not a bug** — it is the fail-closed signal that the headless AX path is unavailable and the caller must opt into `--headed`. Never widen the default policy to make it disappear.

5. **When verifying a gesture in a test, observe the real effect, never the command's `ok:true`.** An AX action can report `verified_press:succeeded` while the underlying control's handler never ran (SwiftUI `CommandMenu`, `Slider`). Re-read the target's state to confirm.

## Why This Matters

- It keeps the **headless-first reliability guarantee** honest: the tool only claims a headless effect when the OS actually provides one, and fails closed otherwise.
- It preserves **cross-platform extensibility**: the same 54-command surface works identically across macOS/Windows/Linux, and each adapter contributes whatever headless capability its platform has — without touching the command layer.
- It prevents the **vacuous-success trap**: assuming `ok:true` means the gesture happened, when an AX action succeeded at the API layer but the control ignored it.

## When to Apply

- Deciding whether a command needs `--headed` (consult the matrix; `POLICY_DENIED` headless = needs `--headed` or has no headless path).
- Implementing a new platform adapter: map each `Action` to the platform's best headless path first, gate physical fallbacks on the policy, and return `POLICY_DENIED` when only a physical gesture would work.
- Building fixtures/tests: use **native AppKit** controls (`NSSlider`/`NSStepper`) when you need a genuinely AX-actionable target; SwiftUI equivalents will not validate the AX path.

## Examples

Double-click is headless-capable only via `AXOpen`:

```bash
# Finder list row advertises AXOpen -> headless double-click opens it
agent-desktop double-click @e12

# A plain button with no AXOpen -> headless fails closed, needs --headed
agent-desktop double-click @e3            # POLICY_DENIED
agent-desktop --headed double-click @e3   # physical double-click completes
```

The macOS dispatch gates the physical path on the policy (so it is reachable only under `--headed`):

```rust
// crates/macos/src/actions/chain_defs.rs
pub(crate) fn double_click(el, _caps, policy) -> Result<(), AdapterError> {
    if ax_helpers::has_ax_action(el, "AXOpen") && ax_helpers::try_ax_action(el, "AXOpen") {
        return Ok(());                       // headless AX path
    }
    crate::actions::dispatch::click_via_bounds(el, MouseButton::Left, 2, policy) // gated; POLICY_DENIED headless
}
```

The menu bar is readable/openable but SwiftUI `CommandMenu` items do not fire via AX:

```bash
agent-desktop snapshot --app MyApp --surface menubar   # enumerates the full menu bar with refs
# clicking a SwiftUI CommandMenu item: verified_press:succeeded, but the action closure never runs
# native AppKit menu items DO fire; .contextMenu item selection DOES fire
```

## Related

- `best-practices/preserve-command-policy-semantics-during-refactor-2026-05-12.md` — why `type` keeps a `focus_fallback` base and the shared helper takes the caller's policy.
- `best-practices/keep-ffi-action-policy-aligned-with-cli-2026-05-12.md` — FFI and CLI run the same `ref_action::execute_resolved` ladder, so policy semantics stay identical.
- `best-practices/playwright-grade-desktop-reliability-2026-06-02.md` — strict late resolution, actionability preflight, and the headless-first contract this builds on.
- `best-practices/abort-state-guidance-multi-step-physical-input.md` — what happens when a physical drag sequence fails mid-flight: the button is released back at the origin (never the unreached destination) and the error describes the end state.
