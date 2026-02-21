---
title: "fix: Sub-window tree, duplicate error output, AX-only commands"
type: fix
date: 2026-02-19
---

# fix: Sub-window tree traversal, duplicate error output, and AX-only command execution

## Overview

Three bugs identified during live testing against macOS System Settings:

1. **Sub-window tree missing** — `snapshot` only captures the sidebar; the content pane (a child AXWindow) is never reached because `window_element_for` queries only `kAXWindowsAttribute`, which excludes embedded sub-windows.
2. **Error emitted twice** — An invalid-argument error JSON appears twice in combined stdout+stderr output. Clap may write its own formatted output to stderr while `main.rs` independently emits the JSON to stdout.
3. **CGEvent and osascript in command paths** — `press`, `type`, `click` (fallback), `double-click`, `right-click` (fallback), `scroll`, `toggle` (fallback), `select` (fallback), `close-app` (graceful), `focus-window` all use CGEvent mouse/keyboard injection or osascript System Events. Every command must be AX-API-only; no CGEvent, no HID injection, no osascript for input delivery.

---

## Problem Analysis

### 1 — Sub-window tree (`crates/macos/src/tree.rs:53–75`)

`window_element_for` walks `kAXWindowsAttribute` to find a matching AXWindow, falling back to the first window or the application element. On macOS, `kAXWindowsAttribute` returns only the **primary top-level windows** registered with the window server.

Apps like System Settings expose their content pane as a **separate AXWindow** accessible via `kAXChildrenAttribute` on the app element (or on the main window). This child window never appears in `kAXWindowsAttribute`, so the tree traversal starts from the sidebar window and never crosses into the content pane.

Concrete evidence: snapshotting System Settings shows `AXOutline (Sidebar)` with 40+ tree items but no content-pane elements. Taking a screenshot confirms the content pane (Appearance options) is present but invisible to the AX tree walk.

**Root cause:** `window_element_for` stops after `kAXWindowsAttribute`; it does not consult `kAXChildrenAttribute` on either the app element or on the matched window.

### 2 — Duplicate error output (`src/main.rs:10–30`)

`emit_json` writes exactly once to stdout (`src/main.rs:122–130`). However, the Claude Bash tool (and any caller that merges stdout + stderr) sees two copies of the error. Clap v4's error type, when converted via `.to_string()` or when the process exits via `std::process::exit`, may flush a buffered internal writer to stderr that contains the same message text. Independently, `BufWriter` wrapping stdout's lock means our JSON flushes to stdout. Together both streams appear in a combined capture.

Fix: configure the clap command to redirect its own error output to a null sink so our JSON is the sole output on any stream.

### 3 — CGEvent and osascript usage

| File | Location | Non-AX mechanism |
|------|----------|-----------------|
| `crates/macos/src/actions.rs` | `Action::Click` fallback | `cg_mouse_click` → `CGEvent::new_mouse_event` |
| `crates/macos/src/actions.rs` | `Action::DoubleClick` | `cg_mouse_click` → `CGEvent::new_mouse_event` |
| `crates/macos/src/actions.rs` | `Action::RightClick` fallback | `cg_mouse_click` → `CGEvent::new_mouse_event` |
| `crates/macos/src/actions.rs` | `Action::Toggle` fallback | `cg_mouse_click` → `CGEvent::new_mouse_event` |
| `crates/macos/src/actions.rs` | `Action::Select` fallback | `cg_mouse_click` → `CGEvent::new_mouse_event` |
| `crates/macos/src/actions.rs` | `Action::Scroll` | `CGEvent::new_scroll_event` → HID |
| `crates/macos/src/actions.rs` | `Action::TypeText` | `synthesize_text` → `CGEvent::new_keyboard_event` |
| `crates/macos/src/actions.rs` | `Action::PressKey` | `synthesize_key` → `CGEvent::new_keyboard_event` |
| `crates/macos/src/input.rs` | `synthesize_key` | `CGEvent::new_keyboard_event` + HID post |
| `crates/macos/src/input.rs` | `synthesize_text` | `CGEvent::new_keyboard_event` + HID post |
| `crates/macos/src/app_ops.rs:27–39` | `press_for_app_impl` | `osascript` → System Events `keystroke`/`key code` |
| `crates/macos/src/app_ops.rs:9–18` | `focus_window_impl` | `osascript` → `tell application X to activate` |
| `crates/macos/src/app_ops.rs:150–173` | `close_app_impl` | `osascript` → quit / `pkill` |

**AX replacements available on macOS:**

| Replaced mechanism | AX replacement |
|-------------------|----------------|
| Mouse click (CGEvent) | `AXUIElementPerformAction(kAXPressAction)` |
| Double-click (CGEvent) | `AXUIElementPerformAction("AXOpen")` or two sequential `kAXPressAction` calls |
| Right-click (CGEvent) | `AXUIElementPerformAction("AXShowMenu")` (already implemented; just remove fallback) |
| Scroll (CGEvent) | `AXUIElementPerformAction(kAXScrollDownAction / kAXScrollUpAction / kAXScrollLeftAction / kAXScrollRightAction)` |
| Text input (CGEvent keyboard) | `AXUIElementSetAttributeValue(kAXValueAttribute, newText)` after `kAXFocusedAttribute = true` |
| Key press for Return/Escape/Space | `AXUIElementPerformAction(kAXConfirmAction / kAXCancelAction / kAXPressAction)` on focused element |
| Key combo shortcuts (cmd+c etc.) | Traverse menu bar AX tree: find `AXMenuItem` where `AXMenuItemCmdChar` + `AXMenuItemCmdModifiers` match, then `kAXPressAction` |
| Window activation (osascript) | `AXUIElementSetAttributeValue(windowEl, kAXMainAttribute, kCFBooleanTrue)` |
| Window close (osascript) | `AXUIElementPerformAction(kAXCloseButtonAttribute child, kAXPressAction)` |

**Limitation:** Arbitrary key combos with no menu-bar equivalent (e.g., `f5`, custom shortcuts not in any menu) cannot be delivered via pure AX API. These should return `ACTION_NOT_SUPPORTED` with a clear suggestion rather than silently falling back to HID injection.

---

## Proposed Solution

### Fix 1 — Expand sub-window discovery (`crates/macos/src/tree.rs`)

After exhausting `kAXWindowsAttribute`, `window_element_for` must also:
1. Query `kAXChildrenAttribute` on the **application element** and collect any child whose role is `AXWindow`, `AXPanel`, `AXSheet`, or `AXDrawer`.
2. Attempt title-exact, then title-fuzzy match on those children.
3. If the matched window has **itself** an `AXSplitGroup` or secondary `AXWindow` child visible via `kAXChildrenAttribute`, include those in the subtree — `build_subtree` already recurses `kAXChildrenAttribute` so this comes for free once the root is correct.

No changes to `build_subtree` are required; only `window_element_for` needs expanding.

### Fix 2 — Silence clap's stderr (`src/main.rs`)

Replace bare `Cli::try_parse()` with an explicit command build that redirects clap's error writer to stderr (discarded via a custom `Write` impl that no-ops), OR use clap's `override_usage` to suppress the automatic error echo.

Concrete approach: configure the `Command` to use a null stderr writer before calling `try_get_matches_from`. In clap 4.x this is done by building the command with `.error_format(clap::ErrorFormat::Plain)` combined with `stderr_fn` override so our `emit_json` path is the only output.

Simplest viable fix: add an unconditional `eprintln!` suppression by redirecting stderr on the command, or simply verify with `2>/dev/null` that only one copy appears on stdout (confirming stderr is the source). If confirmed, wrap the `Cli::command()` call to set the error writer to `std::io::sink()` before `try_get_matches_from`.

### Fix 3 — AX-only command execution

**`crates/macos/src/actions.rs`** — Remove all CGEvent imports and `cg_mouse_click`. Update each action:

- `Action::Click` → `kAXPressAction` only; if it returns error, propagate `ACTION_FAILED` (no CGEvent fallback).
- `Action::DoubleClick` → `AXUIElementPerformAction("AXOpen")` first; if unsupported, `kAXPressAction` twice with a short sleep; if still unsupported, return `ACTION_NOT_SUPPORTED`.
- `Action::RightClick` → `AXUIElementPerformAction("AXShowMenu")` only; remove CGEvent fallback.
- `Action::Toggle` → `kAXPressAction` only; remove CGEvent fallback.
- `Action::Select` → `kAXPressAction` only; remove CGEvent fallback.
- `Action::Scroll` → Use `kAXScrollDownAction` / `kAXScrollUpAction` / `kAXScrollLeftAction` / `kAXScrollRightAction` from `accessibility_sys`; apply `amount` times. Remove `CGEvent::new_scroll_event`.
- `Action::TypeText` → `kAXFocusedAttribute = true` then `kAXValueAttribute = newText`. For append semantics, read current value first and concatenate. Remove `synthesize_text`.
- `Action::PressKey` → Implement `ax_press_key` (see below); remove `synthesize_key`.

**`crates/macos/src/input.rs`** — Remove `synthesize_key` and `synthesize_text` entirely. File may become empty/deleted; if so, remove from `lib.rs` too.

**`crates/macos/src/app_ops.rs`** — Replace:

- `press_for_app_impl` → Activate target app via AX (`kAXMainAttribute`), then call `ax_press_key`.
- `focus_window_impl` → `AXUIElementSetAttributeValue(windowEl, kAXMainAttribute, kCFBooleanTrue)`.
- `close_app_impl` (graceful) → Query `kAXCloseButtonAttribute` on the main window element, then `kAXPressAction` on the close button.
- `close_app_impl --force` → Keep `pkill` (explicit process termination is not an AX concern).
- `launch_app_impl` → Keep `open -a` (launching an app has no AX equivalent).

**New: `ax_press_key` function** (`crates/macos/src/actions.rs` or new `crates/macos/src/ax_key.rs`)

```
ax_press_key(app_pid: Option<i32>, combo: &KeyCombo) -> Result<ActionResult, AdapterError>
```

Strategy:
1. Get the focused element via `kAXFocusedUIElement` on the app (or system-wide element if no app given).
2. Map simple keys to AX actions:
   - `return` / `enter` → `kAXConfirmAction`
   - `escape` / `esc` → `kAXCancelAction`
   - `space` → `kAXPressAction`
   - `up` / `down` / `left` / `right` on sliders → `kAXDecrementAction` / `kAXIncrementAction`
3. For modifier combos (cmd+c, cmd+v, etc.) with no focused-element AX action:
   - Enumerate the menu bar: `kAXMenuBarAttribute` → iterate `AXMenuBarItem` → expand each → walk `AXMenuItem` children
   - Match against `AXMenuItemCmdChar` and `AXMenuItemCmdModifiers`
   - `kAXPressAction` on the matched `AXMenuItem`
4. If no match in steps 2–3: return `Err(AdapterError::new(ErrorCode::ActionNotSupported, "No AX equivalent for key combo '...'; this combo has no menu-bar action and no AX direct action"))`

---

## Acceptance Criteria

- [ ] `agent-desktop snapshot --app "System Settings"` returns both sidebar **and** content pane elements in a single tree
- [ ] Any invalid-argument invocation emits exactly **one** JSON error object on stdout; stderr is silent
- [ ] `agent-desktop press return` (focused text field active) succeeds without CGEvent
- [ ] `agent-desktop press cmd+z` finds and activates the Undo menu item via AX, returns success
- [ ] `agent-desktop press f5` returns `{"ok":false,"error":{"code":"ACTION_NOT_SUPPORTED",...}}` with a descriptive message
- [ ] `agent-desktop click @e1` uses only `kAXPressAction`; no CGEvent in the code path
- [ ] `agent-desktop scroll @e1 --direction down` uses `kAXScrollDownAction`
- [ ] `agent-desktop type @e1 "hello"` sets value via `kAXValueAttribute`
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes

---

## Files to Change

| File | Change |
|------|--------|
| `crates/macos/src/tree.rs` | Expand `window_element_for` to include `kAXChildrenAttribute` discovery of sub-windows |
| `src/main.rs` | Redirect clap's stderr error writer to suppress duplicate output |
| `crates/macos/src/actions.rs` | Remove all `cg_mouse_click` / `CGEvent` usage; rewrite each action with AX equivalents |
| `crates/macos/src/input.rs` | Remove `synthesize_key`, `synthesize_text`; delete file if empty |
| `crates/macos/src/app_ops.rs` | Replace `press_for_app_impl` and `focus_window_impl` with AX; update `close_app_impl` graceful path |
| `crates/macos/src/lib.rs` | Remove `input` module export if file deleted |

New file (optional, if `ax_press_key` grows beyond ~80 LOC and would push `actions.rs` over 400 LOC):

| File | Purpose |
|------|---------|
| `crates/macos/src/ax_key.rs` | `ax_press_key` — focused-element action dispatch + menu-bar shortcut traversal |

---

## References

- `kAXScrollDownAction` / `kAXScrollUpAction` / `kAXScrollLeftAction` / `kAXScrollRightAction` — standard AX scroll actions in `accessibility_sys`
- `kAXConfirmAction`, `kAXCancelAction` — `accessibility_sys` constants for Return / Escape on focused elements
- `kAXMenuBarAttribute`, `AXMenuItemCmdChar`, `AXMenuItemCmdModifiers` — AX menu traversal attributes
- `kAXCloseButtonAttribute` — AX attribute for window close button element
- `kAXMainAttribute` — AX attribute to make a window the main window
- `kAXFocusedUIElementAttribute` — system-wide or app-level focused element query
- Clap v4 `Command::error_format` / null error writer — suppress stderr echo of parse errors
