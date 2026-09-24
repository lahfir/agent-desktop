---
title: Background keyboard delivery makes one window key inside its own process
date: 2026-09-24
category: best-practices
module: core background_keyboard and macos input/keyboard_background
problem_type: architecture_pattern
component: tooling
severity: medium
applies_when:
  - "Typing into or pressing keys in a window that is hidden, offscreen, or behind the user's app"
  - "Driving VS Code (Monaco) or another Electron editor without taking the user's keyboard focus"
  - "Debugging a press/type --background command that returned ok but typed nothing"
tags: [background-keyboard, cgeventposttopid, skylight, electron, monaco, delivery-semantics, macos]
---

# Background keyboard delivery makes one window key inside its own process

## Context

For some controls the default key paths cannot type into an inactive window
without taking the user's focus. Native AppKit text fields usually accept
headless `type` (`AXSelectedText`); the gap is controls that ignore it,
notably Electron editors such as VS Code's Monaco. Live on VS Code (window
`w-15592`, hidden workspace):

- `press enter --app Code` created a file, but through `AXConfirm`, not a
  key event: headless `press` first maps simple keys and menu shortcuts to
  accessibility actions (`cmd+n` opened a new window the same way).
- `press <char> --app Code` once landed `frombackground`: `space` maps to
  `AXPress`, so every space was lost. Later attempts failed in
  `require_focused_element` because an inactive Electron window exposes no
  verified `AXFocusedUIElement`.
- Headless `type <ref>` writes `AXSelectedText`, which Monaco does not honor,
  and fails the readback with `ACTION_FAILED`.

`--background` on `press` and `type` is the keyboard counterpart of the
[background pointer](background-pointer-delivery-2026-09-23.md). It posts
key events to the process that owns one exact window and never uses the HID
tap, app activation, `_SLPSSetFrontProcessWithOptions` on the target, or any
menu or accessibility key mapping.

## Guidance

### Surface

```bash
agent-desktop press cmd+s --background --window-id w-15592
agent-desktop type @s8f3k2p9:e7 "hello from background, 42!" --background
agent-desktop type --background --window-id w-15592 "into the focused field"
```

- `press --background` needs `--window-id` (from `list-windows`) and rejects
  `--app`; `--window-id` without `--background` is `INVALID_ARGS`.
- `type --background <ref>` takes the pid, process instance, and exact
  window from the ref and fails closed on focus: the element gets an
  accessibility `SetFocus` (headless policy, never a physical fallback),
  and the keys are posted only when that write succeeds and the read-back
  confirms the element is focused (`data.background.ax_focus.status:
  "verified"`). An error or an unconfirmed read-back is `ACTION_FAILED`,
  not delivered, because the keys would otherwise land in whatever field
  the window focused before. A stale ref is `STALE_REF`, not delivered.
- `type --background --window-id w-N TEXT` (no ref) types into whatever
  that window has focused, like `press --background --window-id`. Use it
  after placing the caret with `mouse-click --background`, or when an
  inactive Electron window will not confirm accessibility focus.
- Both reject `--headed`. Dangerous combos still need `--force`. Text is
  limited to 10,000 bytes. Batch uses `"background": true` and
  `"window_id"`.
- The result is `delivered_unverified` with `retry: unsafe`, plus the same
  `data.background` report as the pointer (`pid`, `window_id`,
  `focus_change`, `layers`, `degraded`, `focus_guard`, frontmost pids).
  The ref form reads focus back through `SetFocus`
  (`crates/macos/src/actions/chain_value_write.rs` `focus_write_had_effect`
  -> `actions/activation_effect.rs` `focus_state`, which reads the app's
  `AXFocusedUIElement`); `press` and `type --window-id` read nothing. The
  field value is never read; observe the effect with `snapshot`. A
  `--wait-for` observes the target window, not the user's frontmost app.

### Why the window must be made key

AppKit sends a key event to its application's key window, and an inactive
app's windows are not key. A pid-targeted key therefore reaches whichever
window the target last had as key, or nothing. The default recipe, in
posting order (`crates/macos/src/input/background_delivery.rs`):

1. **activate.** The pointer path's target-only focus record (`0x0D`).
2. **keywindow.** yabai's `window_manager_make_key_window` pair, sent to the
   target only: `[0x04]=0xF8`, `[0x08]=0x01` then `0x02`, `[0x20..0x30]=0xFF`,
   `[0x3A]=0x10`, `[0x3C..0x40]` = window number. yabai sends it after making
   the process frontmost; background-computer-use's `press_key` sends focus
   plus this pair to the target alone, which is what this path copies. Each
   record sits in a zeroed 256-byte buffer (`SLPSPostEventRecordTo`
   over-reads past 0xF8 on macOS 14.2.1+). The two records of the pair are
   posted back to back and followed by one 50 ms settle, as is the single
   focus record before them.
3. **route.** Every key event carries the target pid (field 40) and window
   number (field 51). None of the reference projects set these on keys;
   they are cheap and can be ablated.
4. **skylight + auth.** Events go through `SLEventPostToPid` with an
   `SLSEventAuthenticationMessage` attached, as cua does: its source says the
   window server passes synthetic keys to Chromium targets on macOS 15+ only
   with one. The message is built by
   `+[SLSEventAuthenticationMessage messageWithEventRecord:pid:version:]`
   from the event record pointer at offset 24 of `__CGEvent`; macOS 14 lacks
   the selector and reports `auth:SLSEventAuthenticationMessage_unavailable`.
5. **guard.** The pointer's focus guard, unchanged.

### Events

`crates/macos/src/input/background_key_events.rs`:

- A combo is one key down and one key up with the modifiers as flags on both
  (cua's background shortcuts; no separate modifier key events), 8 ms apart.
  Return and Tab combos carry the same Unicode text (`\r`, `\t`) as a typed
  line break or tab: `press enter --background` without it once typed no
  newline in VS Code while a typed `\n` did, and the missing string was the
  only difference in how the two events were built.
- Text is one down/up pair per `char`, the UTF-16 text set with
  `CGEventKeyboardSetUnicodeString` on both events, key code 0 and no flags
  (cua, background-computer-use, and Warp all do this, so no keyboard layout
  is assumed). `\n`/`\r` (with `\r\n` counted once) become Return and `\t`
  becomes Tab so editors run their own handling. Each pair holds 8 ms and
  pauses 8 ms, cua's pacing; the command budget adds 25 ms per character.
- Every event is built, and authenticated, before anything is posted, so a
  bad key name or window id is `not_delivered`. The deadline (including an
  enclosing batch deadline) is checked before every key down; a key up for
  a key already down is always posted. When the budget runs out mid-text the
  command stops with `TIMEOUT`: `not_delivered` if nothing was posted,
  otherwise `delivered_unverified` (`retry: unsafe`) with
  `details.delivered_events` and `details.planned_events`.

### Ablation

`AGENT_DESKTOP_BG_LAYERS` selects layers for diagnosis, shared with the
pointer: unset means the path default (keyboard: `route,skylight,auth,
activate,keywindow,guard`), `none` means bare `CGEventPostToPid`, otherwise a
comma list of `route`, `skylight`, `auth`, `activate`, `keywindow`, `guard`,
`primer`. Layers that do not apply are dropped (`primer` for keys, `auth` and
`keywindow` for the pointer), and `data.background.layers` lists the layers
that were requested; `degraded` lists the ones that were unavailable.

## Risks

- **The app decides.** A Command combo may still hit the target's own menu
  items through its main menu; the command never looks one up. cua notes that
  authenticated keys bypass the route `NSMenu` key equivalents use, so a menu
  shortcut may need `AGENT_DESKTOP_BG_LAYERS` without `auth`.
- **Key window changes inside the target.** The target's own idea of its key
  window is left on the typed window.
- **Private SPI and memory layout.** The make-key record, the auth factory,
  and the `__CGEvent` record offset are undocumented. Missing symbols show
  up in `degraded`; a changed layout would not.
- **Focus within the window.** Keys reach the window's first responder. The
  ref form refuses to type unless accessibility focus on the element is
  confirmed; inactive Electron windows often cannot confirm it, and then
  `mouse-click --background` on the field plus `type --window-id` is the
  route.
- **Brief focus steal.** Focus preservation is best effort. As with the
  pointer, the guard restores the user's app when the target takes the
  front, leaves a switch to any other app alone, and reports both.

## Verification

Unit tests (MockAdapter-style, no events posted) cover routing, target
resolution and rejection, the fail-closed AX focus check and its headless
policy, the no-ref `--window-id` form,
blocked combos, text limits, deadlines, CLI and batch parsing, dispatch, the
key-window record bytes, event plans (order, text, key codes, pacing), built
event fields and Unicode text, and layer parsing. The auth attachment is
smoke-tested on a built event. Live behavior has not been verified: test one
layer set at a time by snapshot before, the command, snapshot after, and a
check that the frontmost app and cursor did not change.
