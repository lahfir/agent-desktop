---
title: Background pointer delivery posts to one window's process, not the screen
date: 2026-09-23
category: best-practices
module: core background_pointer and macos input/mouse_background
problem_type: architecture_pattern
component: tooling
severity: medium
applies_when:
  - "Hovering or clicking inside a window that is hidden, offscreen, or behind other windows"
  - "Revealing hover-only controls (for example VS Code Explorer header actions) without taking the user's cursor"
  - "Debugging a --background command that returned ok but had no visible effect"
tags: [background-pointer, cgeventposttopid, skylight, electron, hover, delivery-semantics, macos]
---

# Background pointer delivery posts to one window's process, not the screen

## Context

Some controls exist in the accessibility tree only while hovered. VS Code's
Explorer header (New File, New Folder, ...) is the motivating case: with the
window on a hidden AeroSpace workspace, the buttons never appear, and headed
hover would move the user's cursor and raise the window.

`--background` on `hover`, `mouse-move`, and `mouse-click` posts the event to
the process that owns one exact window. It never uses the HID tap,
`CGWarpMouseCursorPosition`, `NSRunningApplication.activate`, or
`_SLPSSetFrontProcessWithOptions` on the target.

The first version (7f6e4f53) posted bare `CGEventPostToPid` events tagged
with fields 91/92. Live on macOS 26 it delivered with focus and pointer
unchanged but had **no effect**: a Finder sidebar click in a hidden window,
VS Code hover and activity-bar clicks, and a ClickUp (Electron) tab click in a
visible inactive window all did nothing. The macOS path is now a layered
recipe modelled on three projects that make background clicks land: Warp
(`mouse.rs`, `activation.rs`), cua (`mouse.rs`, `skylight.rs`), and
background-computer-use (`NativeWindowServerPreparation.swift`,
`NativeBackgroundClickTransport.swift`).

## Guidance

- **Always target an exact window.** A ref supplies pid, process instance, and
  `source_window_id`; raw `--xy` must pass `--window-id`. Core re-verifies the
  window with `resolve_window_strict` under the interaction lease right before
  posting (title left empty because titles change).
- **Check geometry against the window only.** The point must lie inside the
  window bounds. Offscreen and covered windows are valid targets; there is no
  occlusion or on-screen requirement, because nothing is hit-tested against
  the screen.
- **Build everything before posting.** Layer parsing, window-number parsing,
  event planning, and event construction all run before the first post, so
  those failures are `not_delivered`.
- **Never claim silent success.** Success is `delivered_unverified`
  (`retry: unsafe`), including hover: a repeat is harmless, but the contract
  has no delivered-and-safe state, and the right follow-up is a snapshot.
- **Skip the cursor overlay.** It would draw a cursor where the real one is
  not, over a window that may be invisible.

## Layers

`AGENT_DESKTOP_BG_LAYERS` selects the layers for live diagnosis and is
deliberately not a CLI flag. Unset or empty means the recommended set
`route,skylight,activate,guard`; `none` means the bare 7f6e4f53 path; any
other value is a comma-separated subset of `route`, `skylight`, `activate`,
`guard`, `primer` (an unknown name is `INVALID_ARGS`, not delivered). The
result lists the requested layers in `data.background.layers` and any
requested layer that was unavailable or failed in `data.background.degraded`.

Always set: click state (field 1) and `kCGMouseEventWindowUnderMousePointer`
(91) plus `...ThatCanHandleThisEvent` (92) set to the CG window number, with
the location in global CG coordinates. An earlier prototype (c803d040) wrote
fields 28/29 instead; those are the wrong fields.

1. **route.** Adds field 40 (target pid), undocumented fields 51 (window
   number) and 58 (`1`, Warp's click group), field 3 (button number), field 7
   `NSEventSubtypeTouch` (3, set by both cua and background-computer-use), and
   pressure 1.0 on button-down (Warp). It sets the window-local top-left
   location with the private `CGEventSetWindowLocation` so AppKit's
   `locationInWindow` is right without a hit test (degraded as
   `route:CGEventSetWindowLocation_unavailable` if the symbol is gone). A
   click is preceded by a `mouseMoved` at the target (15 ms), as cua does.
2. **skylight.** Posts with SkyLight's `SLEventPostToPid`, falling back to
   `CGEventPostToPid` only when the symbol is missing. Exactly one path posts
   each event.
3. **activate.** Sends a 248-byte window-server event record to the **target
   only** with `SLPSPostEventRecordTo`: `[0x04]=0xF8`, `[0x08]=0x0D`,
   `[0x3C..0x40]` = window number (little-endian), `[0x8A]=0x01`, then waits
   50 ms. This is background-computer-use's `targetOnlyFocus`: the target's
   window believes it is focused, which is intended to satisfy Chromium's
   `shouldIgnoreMouseEvent:` and first-mouse checks (not yet confirmed live). No defocus record is
   ever sent to the user's app (cua and yabai send one; that is what steals
   focus).
4. **guard.** Samples the frontmost app before delivery, after every posted
   event, and every 25 ms for 400 ms afterwards. Only the **target** taking
   the front counts as a steal: then it restores the **user's** app with
   `_SLPSSetFrontProcessWithOptions(userPSN, 0, kCPSNoWindows)`, at most three
   times. When any third app becomes frontmost, the guard assumes the user
   switched apps, stops watching, and never switches back. It reports
   `data.background.focus_guard { interventions, restored, max_steal_ms,
   yielded }`; `focus_change` becomes `restored` with a warning when the front
   moved and came back, and `changed` with a warning when it did not.
   Per-pid event taps (Warp) were not added: they need a run-loop thread and
   teardown on every path, which the guard's polling avoids.
5. **primer (off by default).** Before the real click, a left down/up at
   window-local `(-1, -1)`, one point above and left of the frame, then
   100 ms. That is outside every content view, so it cannot press a control,
   but it is still a real click the app sees; enable it only if a Chromium
   click is still swallowed with the other layers on. cua uses the same
   primer; Warp's centre-of-window primer was rejected because it can hit a
   real control.

The frontmost reader tries the window server (`_SLPSGetFrontProcess` +
`GetProcessPID`), then AX `AXFocusedApplication`, then `NSWorkspace`. AX alone
returned nothing while Comet (Chromium) was frontmost, so every report said
`focus_change: unknown`.

Private symbols are resolved with `dlopen`/`dlsym` once per process
(`input/skylight.rs`); a missing symbol degrades that layer and never fails
the command.

## Risks

- **Private SPI.** SkyLight symbols, fields 51/58, and the record layout are
  undocumented and can change with any macOS update. Missing symbols show up
  in `degraded`; a changed record layout would not, so re-validate after OS
  upgrades.
- **The target believes it is active.** During and after delivery the target
  may draw its window as focused or start caret blinking; its real key window
  state is not restored.
- **Brief focus steal.** An app may activate itself in response. The guard
  restores the user's app within about one poll (25 ms) and reports it, but
  the user can see a flicker and keystrokes typed in that window may go to the
  target.

## Deadline

The command deadline (including an enclosing batch deadline) is checked
before every event that starts something new: a move or a button down. A
button-up for a down already posted is always sent, so nothing stays pressed.
When the budget runs out the delivery stops with `TIMEOUT`: `not_delivered`
if no input event was posted yet (the focus record alone is idempotent), and
otherwise `delivered_unverified` / `retry: unsafe` with
`details.delivered_events` and `details.planned_events`.

## Known limits

- **Chromium/Electron.** `RenderWidgetHostViewCocoa` drops `mouseMoved`
  unless the window is main or key in its app and refuses first-mouse clicks
  unless `acceptFirstMouse` is set; `activate` exists for this. If hover still
  fails, VS Code's `workbench.view.alwaysShowHeaderActions` setting shows the
  header actions without hover. Prefer a semantic `click @ref` (AXPress) once
  hover has revealed a control.
- **Sandboxed or hardened apps** (Mail, Notes, App Store) may drop
  pid-targeted events silently.
- **Keys are out of scope.** macOS delivers keystrokes to the key window.
- **Effects are not verified.** Observe with `snapshot` after delivery.

## Verification

Unit tests cover layer parsing, the focus-record bytes, every routing field,
window-local conversion with negative origins, event plans (move-first,
primer placement, click states), guard decisions with a scripted clock and
frontmost sequence, the frontmost fallback chain, window derivation from refs,
bounds rejection, disposition and focus reporting, CLI/batch parsing
(including negative coordinates), and dispatch routing. The delivery loop runs
against a scripted transport and clock (`background_delivery_tests.rs`):
SkyLight-to-`CGEventPostToPid` fallback, one posting path per event, focus
record ordering, and deadline expiry after activation, between click pairs,
and right after a button down. None of them post a real event.
`tests/e2e/scenarios/background.sh` clicks a fixture button with
`mouse-click --background` and checks the frontmost app and cursor position.
Live behavior must otherwise be checked by observation, one layer set at a time:
snapshot before, the `--background` command, snapshot after, and confirm the
frontmost app and cursor position are unchanged.
