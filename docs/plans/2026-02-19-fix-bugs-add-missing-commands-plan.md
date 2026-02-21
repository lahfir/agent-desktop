---
title: "fix: Phase 1 bug fixes, AX-first execution, missing commands, adapter-ready architecture"
type: fix
date: 2026-02-19
deepened: 2026-02-19
---

# fix: Phase 1 Bug Fixes + AX-First Execution + Missing Commands + Adapter-Ready Architecture

## Enhancement Summary

**Deepened on:** 2026-02-19
**Plans merged:** `fix-bugs-add-missing-commands-plan.md` + `fix-subtree-traversal-error-dedup-ax-only-plan.md`
**Research agents used:** AX-only execution, select semantics, agent-browser gap analysis, sub-window tree traversal, stderr duplication, cross-platform adapter design, drag-and-drop feasibility

### Key Improvements Over Original Plan
1. **AX-first philosophy**: Every command uses Accessibility API as primary execution path. CGEvent/osascript kept ONLY for operations with no AX equivalent (drag, hover, modified key combos without menu bar entries)
2. **Corrected root causes**: Stderr duplication is NOT a code bug (removed from plan). Sub-window tree fix is a ONE-LINE change in `list_windows_impl`, not `window_element_for`
3. **Role-aware `select`**: Branches by AX role — AXPopUpButton (press→menu→press item), AXComboBox (set kAXValueAttribute), AXList/AXTable (set kAXSelectedChildrenAttribute)
4. **New commands from agent-browser gap analysis**: `check`/`uncheck` (idempotent), `scroll-to`, `wait --text`, `find` enhancements (`--count`, `--first`, `--nth`), `hover`, `drag`
5. **Adapter extensibility**: Flat `#[non_exhaustive]` Action enum, WindowOp enum, MouseEvent struct, single `mouse_event` method

### AX-First Principle

> Every command MUST use the Accessibility API as its primary execution mechanism. CGEvent mouse/keyboard synthesis and osascript are NEVER the primary path. They are kept ONLY as documented fallbacks for operations that have no AX equivalent (drag, hover, arbitrary screen-coordinate clicks, modified key combos not in any menu bar).

**AX-Only Commands** (no CGEvent/osascript at all):
- `click`, `right-click`, `toggle`, `select`, `expand`, `collapse`, `set-value`, `focus`, `type` (via kAXValueAttribute)
- `focus-window` (via kAXMainAttribute + kAXRaiseAction)
- `close-app` graceful (via AX menu bar "Quit" item or kAXCloseButtonAttribute)
- `press` for Return/Escape/Space (via kAXConfirmAction/kAXCancelAction/kAXPressAction)
- `press` for cmd+shortcuts (via AX menu bar traversal matching kAXMenuItemCmdChar)
- `scroll` (via AXScrollBar kAXIncrementAction/kAXDecrementAction)
- `check`/`uncheck` (via kAXValueAttribute set on checkbox/switch)
- Window geometry: `resize-window`, `move-window`, `minimize`, `maximize`, `restore`

**Hybrid Commands** (AX resolve coordinates → CGEvent execute):
- `drag` — AX resolves element bounds, CGEvent performs LeftMouseDown→Dragged→Up
- `hover` — AX resolves bounds, CGEvent MouseMoved (tooltips require real cursor movement)
- `mouse-move`, `mouse-click`, `mouse-down`, `mouse-up` — coordinate-based, CGEvent only

**CGEvent-Only Commands** (no AX equivalent exists):
- `key-down`/`key-up` — holding modifier keys has no AX equivalent
- `press` for arbitrary key combos not in menu bar (e.g., `f5`, `ctrl+left`) — falls back to `AXUIElementPostKeyboardEvent`, then returns `ACTION_NOT_SUPPORTED` if both fail

---

## Phase A: Critical Bug Fixes (P0 + P1)

### A1. Fix sub-window tree missing elements

**Problem:** `snapshot --app "System Settings"` only captures the sidebar; the content pane is never reached.

**Root Cause (CORRECTED from original plan):** The bug is NOT in `window_element_for` or `build_subtree`. It's in `list_windows_impl` in `crates/macos/src/adapter.rs`. The CGWindow enumeration filters out windows with empty `kCGWindowName` via a `_ => continue` match arm. System Settings has a window whose CGWindow title is empty, so it's silently skipped. The content pane is NOT a separate AXWindow — it's inside a single AXSplitGroup within the one AXWindow, and `build_subtree` already recurses `kAXChildrenAttribute`, so it would find the content pane IF the window weren't filtered out.

**Files:** `crates/macos/src/adapter.rs`

**Fix:** ONE-LINE change in `list_windows_impl`:
```rust
// BEFORE (adapter.rs, inside the CGWindow title match):
_ => continue,

// AFTER:
_ => app_name.clone(),
```

When `kCGWindowName` is empty or missing, use the application name as fallback instead of skipping the window entirely. This ensures System Settings' content pane window (and similar empty-titled windows in other apps) appears in the window list and gets traversed.

**Verification:** `agent-desktop snapshot --app "System Settings"` returns both sidebar AND content pane elements (AXOutline + content controls like AXCheckBox, AXPopUpButton, etc.)

### A2. Fix `select` false success (P0)

**Problem:** `select @e4 "Courier"` returns `ok:true` but value stays "Helvetica".

**Root cause:** `actions.rs` maps `Action::Select` to a bare `kAXPressAction` click — it just clicks the element instead of performing role-aware selection.

**Files:** `crates/macos/src/actions.rs`

**Critical insight from research:** Popup menu children DON'T EXIST in the AX tree until the menu is physically opened. You cannot query kAXChildrenAttribute on a closed AXPopUpButton to enumerate its options. The `select` command's purpose is to SET a value when the caller already knows what value they want. Option DISCOVERY uses a different workflow: `click @popup` → `snapshot` → see menu items → `click @menuitem`.

**Fix — role-aware branching:**

1. **AXPopUpButton** (dropdown menus):
   - `AXUIElementPerformAction(kAXPressAction)` to open the menu
   - Wait up to 500ms for menu to appear (poll kAXChildrenAttribute on the popup)
   - Walk menu children, find `AXMenuItem` whose `kAXTitleAttribute` matches the target value (case-insensitive, trimmed)
   - `AXUIElementPerformAction(kAXPressAction)` on the matched menu item
   - If no match found: press Escape to close menu, return `ELEMENT_NOT_FOUND` with suggestion listing available options
   - Read back `kAXValueAttribute` to confirm change, include in `post_state`

2. **AXComboBox** (editable dropdowns):
   - `AXUIElementSetAttributeValue(kAXValueAttribute, value)` directly
   - Read back to confirm, include in `post_state`

3. **AXList / AXTable** (list selections):
   - Find child element matching `value` by name
   - `AXUIElementSetAttributeValue(kAXSelectedChildrenAttribute, [matched_child])`
   - Read back to confirm

4. **Any other role**: Return `ACTION_NOT_SUPPORTED` with suggestion: "Element role '{role}' doesn't support select. Use 'click' or 'set-value' instead."

### A3. Fix `toggle` false success (P0)

**Problem:** `toggle @e1` on a textfield returns `ok:true` with no effect.

**Root cause:** `actions.rs` maps `Action::Toggle` to `kAXPressAction` unconditionally — no role check.

**Files:** `crates/macos/src/actions.rs`, `crates/core/src/commands/toggle.rs`

**Fix:**
- Before executing, check element role from RefEntry
- Supported roles: `checkbox`, `switch`, `radiobutton`, `togglebutton`, `menuitemcheckbox`, `menuitemradio`
- For supported roles: `AXUIElementPerformAction(kAXPressAction)`, then read back `kAXValueAttribute` to confirm state changed
- For unsupported roles: return `ACTION_NOT_SUPPORTED` with suggestion: "Toggle only works on checkboxes, switches, and radio buttons. Use 'click' for other elements."

### A4. Fix `launch` window detection (P1)

**Problem:** `launch Calculator` returns error "no window found" even though the app launched fine.

**Root cause:** `app_ops.rs::launch_app_impl` non-wait path only sleeps 500ms. Many apps need 2-5s for window.

**Files:** `crates/macos/src/app_ops.rs`

**Fix:**
- Default behavior: poll `list_windows()` every 200ms for up to 5s (matching the wait path's pattern)
- When the app process exists but no window: return success with `"window": null` and `"note": "App launched but no window detected yet. Use 'wait --window' to poll."`
- Only return error if the `open -a` command itself fails (non-zero exit code)

### A5. Fix `close-app` not actually closing (P1)

**Problem:** `close-app Calculator` returns `ok:true` but app keeps running.

**Root cause:** The graceful quit via osascript doesn't verify termination.

**Files:** `crates/macos/src/app_ops.rs`

**Fix (AX-first):**
- **Primary:** Find "Quit" menu item via AX menu bar traversal (`kAXMenuBarAttribute` → last `AXMenuBarItem` → walk `AXMenuItem` children → find item with title containing "Quit"), then `kAXPressAction` on it
- **Fallback:** If no menu bar accessible, use `AXUIElementSetAttributeValue(kAXHiddenAttribute)` or the existing osascript quit
- After sending quit, poll `list_apps()` every 200ms for up to 3s to verify app exited
- If still running after timeout: return `{ "ok": true, "data": { "closed": false, "note": "Quit requested but app may still be running. Use --force to kill." } }`

### A6. Fix `expand`/`collapse` wrong error code (P1)

**Problem:** Returns `ACTION_FAILED` with raw AX error code (-25205) instead of `ACTION_NOT_SUPPORTED`.

**Files:** `crates/macos/src/actions.rs`

**Fix:**
- Before attempting `AXExpand`/`AXCollapse`, call `AXUIElementCopyActionNames` and check if the action string exists
- If not present: return `AdapterError::new(ErrorCode::ActionNotSupported, "This element doesn't support expand/collapse").with_suggestion("Try 'click' to open it instead.")`
- If present but fails: keep `ACTION_FAILED` with the AX error detail

### A7. Fix `get --property bounds` returning null

**Problem:** `get @e1 --property bounds` returns `null` even though element has screen position.

**Files:** `crates/core/src/commands/get.rs`, `crates/macos/src/adapter.rs`

**Fix:**
- `get bounds` should resolve the element handle via `resolve_element`, then query `kAXPositionAttribute` + `kAXSizeAttribute` LIVE from the AX tree
- Don't rely on RefEntry's cached bounds — query the adapter directly
- Add `get_element_bounds(handle: &NativeHandle) -> Result<Option<Rect>>` to PlatformAdapter trait (with default `not_supported()`)
- macOS impl queries `AXUIElementCopyAttributeValue` for position and size

### A8. Fix `focus-window` to use AX instead of osascript

**Problem:** `focus_window_impl` uses `osascript` → `tell application X to activate`.

**Files:** `crates/macos/src/app_ops.rs`

**Fix (AX-first):**
- Get AXApplication element for the target PID
- `AXUIElementSetAttributeValue(appEl, kAXFrontmostAttribute, kCFBooleanTrue)` to bring app to front
- Get the target window element
- `AXUIElementSetAttributeValue(winEl, kAXMainAttribute, kCFBooleanTrue)` to make it the main window
- `AXUIElementPerformAction(winEl, kAXRaiseAction)` to raise it
- Remove osascript path entirely

### A9. Replace CGEvent fallbacks in action execution

**Problem:** `click`, `right-click`, `toggle`, `select` all fall back to `cg_mouse_click` CGEvent synthesis when AX action fails.

**Files:** `crates/macos/src/actions.rs`

**Fix:**
- **`Action::Click`**: `kAXPressAction` only. If AX error, propagate `ACTION_FAILED` (no CGEvent fallback)
- **`Action::RightClick`**: `AXShowMenu` only. Remove CGEvent fallback. Already implemented correctly; just delete the fallback branch
- **`Action::DoubleClick`**: Try `AXOpen` first. If unsupported, `kAXPressAction` twice with 50ms sleep between. If still fails, return `ACTION_FAILED`
- **`Action::Toggle`**: `kAXPressAction` only after role validation (A3). Remove CGEvent fallback
- **`Action::Select`**: Role-aware AX implementation (A2). Remove CGEvent fallback
- **`Action::Scroll`**: Replace `CGEvent::new_scroll_event` with AX scroll (see A10)
- Remove `cg_mouse_click` helper function entirely from `actions.rs`
- Remove CGEvent mouse imports from `actions.rs`

### A10. Replace CGEvent scroll with AX scroll

**Problem:** `Action::Scroll` uses `CGEvent::new_scroll_event` (HID injection).

**Files:** `crates/macos/src/actions.rs`

**Fix:**
- Find the `AXScrollArea` or `AXScrollBar` associated with the target element
- For vertical scroll: find the vertical `AXScrollBar` child, then:
  - Scroll down: `AXUIElementPerformAction(scrollBar, kAXIncrementAction)` repeated `amount` times
  - Scroll up: `AXUIElementPerformAction(scrollBar, kAXDecrementAction)` repeated `amount` times
- For horizontal: same with horizontal `AXScrollBar` and corresponding actions
- If no scroll bar found: return `ACTION_NOT_SUPPORTED` with suggestion "Element is not scrollable"
- Remove `CGEvent::new_scroll_event` import and usage

### A11. Replace CGEvent keyboard input with AX equivalents

**Problem:** `Action::TypeText` uses `synthesize_text` (CGEvent keyboard per character). `Action::PressKey` uses `synthesize_key` (CGEvent keyboard).

**Files:** `crates/macos/src/actions.rs`, `crates/macos/src/input.rs`

**Fix for TypeText (AX-first):**
- Set `kAXFocusedAttribute = true` on target element
- Read current `kAXValueAttribute`
- `AXUIElementSetAttributeValue(kAXValueAttribute, newText)` — this replaces the full value
- For append semantics: read current value, concatenate, set combined value
- The `type` command already has this distinction: if ref provided, set value on ref. If no ref, this is a "type into focused element" which still needs kAXValueAttribute set on the focused element

**Fix for PressKey (AX-first, multi-strategy):**
1. **Simple keys → AX actions on focused element:**
   - `return`/`enter` → `kAXConfirmAction`
   - `escape`/`esc` → `kAXCancelAction`
   - `space` → `kAXPressAction`
   - `tab` → `kAXNextContentsAction` or move focus
   - Arrow keys on sliders → `kAXIncrementAction` / `kAXDecrementAction`

2. **Modifier combos → AX menu bar traversal:**
   - Get `kAXMenuBarAttribute` from app element
   - Walk `AXMenuBarItem` → expand each → walk `AXMenuItem` children
   - Match `kAXMenuItemCmdChar` (e.g., "C" for Cmd+C) and `kAXMenuItemCmdModifiers`
   - `kAXPressAction` on matched `AXMenuItem`

3. **Fallback for keys with no AX equivalent:**
   - `AXUIElementPostKeyboardEvent(appEl, 0, keyCode, true/false)` — this is technically AX API (not CGEvent), available in accessibility-sys
   - If that also fails: return `ACTION_NOT_SUPPORTED` with message: "No AX equivalent for key combo '{combo}'. This combo has no menu-bar action."

4. **Remove `synthesize_key` and `synthesize_text` from `input.rs`**
   - If `input.rs` becomes empty, delete it and remove from `lib.rs`

### A12. Replace osascript in `press_for_app_impl`

**Problem:** `press --app TextEdit "cmd+c"` uses osascript System Events `keystroke`/`key code`.

**Files:** `crates/macos/src/app_ops.rs`

**Fix (AX-first):**
- Activate target app via AX: `AXUIElementSetAttributeValue(appEl, kAXFrontmostAttribute, kCFBooleanTrue)`
- Use the same AX menu bar traversal strategy from A11 step 2
- If menu bar match found: `kAXPressAction` on the `AXMenuItem`
- If no match: `AXUIElementPostKeyboardEvent` fallback
- Remove osascript `keystroke`/`key code` path entirely

---

## Phase B: Improvements

### B1. Fix `list-apps` data shape + bundle_id

**Files:** `crates/core/src/commands/list_apps.rs`, `crates/macos/src/adapter.rs`

**Fix:**
- Wrap `data` in `{"apps": [...]}` instead of bare array
- Populate `bundle_id` using `kAXBundleIdentifierAttribute` on the AXApplication element (pure AX, no osascript)
- Add `bundle_id: Option<String>` to `AppInfo` struct if not already present

### B2. Fix `is` returning `false` vs "not applicable"

**Files:** `crates/core/src/commands/is_check.rs`

**Fix:**
- Return `{ "result": false, "applicable": false }` when querying a property that doesn't apply to the element's role
- Applicability rules:
  - `checked` → applies to: checkbox, switch, radiobutton, menuitemcheckbox
  - `expanded` → applies to: treeitem, combobox, disclosure, outlinerow
  - `focused` → applies to: ALL interactive roles
  - `visible` → applies to: ALL roles
  - `enabled` → applies to: ALL interactive roles

### B3. Add `--help` descriptions to all commands and flags

**Files:** `src/cli.rs`

**Fix:**
- Add `#[command(about = "...")]` to every `Commands` variant
- Add `#[arg(help = "...")]` to every flag
- Descriptions should be one-line, imperative mood, agent-oriented

### B4. Improve `find` output for unnamed elements

**Files:** `crates/core/src/commands/find.rs`

**Fix:**
- When `name` is null, fall back to `description`, then `title`, then `"(unnamed {role})"`
- Include `description` field in find results when available

### B5. Filter stale menu items from window snapshots

**Files:** `crates/macos/src/adapter.rs` or snapshot engine

**Fix:**
- In `snapshot --surface window`: filter out nodes with role `menuitem` or `menu` that are direct children of the app/window root (not nested inside actual window content)
- This handles the race condition where pressing escape doesn't immediately remove menu items from the AX tree

---

## Phase C: New Commands — Idempotent State Control

These commands address a critical gap: agents need idempotent state-setting, not just toggling.

### C1. `check` / `uncheck` commands

**Problem:** `toggle` is non-idempotent — calling it twice returns to original state. An agent that wants a checkbox ON must first query state, then conditionally toggle. `check` and `uncheck` are idempotent: `check` is always a no-op if already checked.

**CLI:**
```
agent-desktop check @e5
agent-desktop uncheck @e5
```

**Files:** `crates/core/src/commands/check.rs`, `crates/core/src/commands/uncheck.rs`

**New Action variants:** `Action::Check`, `Action::Uncheck`

**Implementation (macOS, AX-only):**
1. Resolve element, verify role is checkbox/switch/radiobutton/menuitemcheckbox
2. Read current `kAXValueAttribute` (0 = unchecked, 1 = checked)
3. For `check`: if already 1, return success with `"already_checked": true`. If 0, `kAXPressAction`
4. For `uncheck`: if already 0, return success with `"already_unchecked": true`. If 1, `kAXPressAction`
5. Read back value to confirm state change
6. For unsupported roles: return `ACTION_NOT_SUPPORTED`

**Cross-platform:** All platforms support querying checkbox state and pressing — this is pure AX.

### C2. `scroll-to` command (scroll element into view)

**CLI:**
```
agent-desktop scroll-to @e15
agent-desktop scroll-to @e15 --align top
```

**File:** `crates/core/src/commands/scroll_to.rs`

**Implementation (macOS, AX-only):**
1. Resolve target element
2. Query `kAXVisibleCharacterRangeAttribute` or check if element bounds are within scroll area visible rect
3. If not visible: find parent scroll area, use `AXUIElementPerformAction(kAXScrollToVisibleAction)` on the target element (this is a native AX action that scrolls the element into view)
4. If `kAXScrollToVisibleAction` not available: incrementally `kAXIncrementAction`/`kAXDecrementAction` on the scroll bar until element bounds are within viewport

**Cross-platform:** `kAXScrollToVisibleAction` is macOS. Windows has `IUIAutomationScrollItemPattern.ScrollIntoView()`. Linux AT-SPI has `scroll_to` in Component interface.

### C3. `wait --text` variant

**CLI:**
```
agent-desktop wait --text "Loading complete" --app TextEdit --timeout 5000
```

**File:** `crates/core/src/commands/wait.rs` (extend existing)

**Implementation:**
- Poll the app's AX tree every 200ms
- Search for any element whose `kAXValueAttribute` or `kAXTitleAttribute` or `kAXDescriptionAttribute` contains the target text (case-insensitive substring match)
- Return the matching element's ref when found
- Return `TIMEOUT` if not found within timeout

---

## Phase D: Enhanced `find` Command

### D1. `find --count` — return count only

**CLI:** `agent-desktop find --app TextEdit --role button --count`

**Output:** `{ "count": 3 }` instead of the full element list.

**File:** `crates/core/src/commands/find.rs`

### D2. `find --first` / `find --last` / `find --nth N`

**CLI:**
```
agent-desktop find --app TextEdit --role button --first
agent-desktop find --app TextEdit --role button --last
agent-desktop find --app TextEdit --role button --nth 2
```

**Output:** Single element instead of array.

**File:** `crates/core/src/commands/find.rs`

### D3. `find --text` — search by text content

**CLI:** `agent-desktop find --app TextEdit --text "Save"`

**Implementation:** Match against `name`, `value`, `title`, `description` attributes (any match counts).

**File:** `crates/core/src/commands/find.rs`

---

## Phase E: New Commands — Mouse & Coordinate Control

These commands require CGEvent synthesis because the Accessibility API has no concept of cursor movement or raw mouse events. They are the documented exceptions to the AX-first principle.

### New Supporting Types

```rust
pub struct Point {
    pub x: f64,
    pub y: f64,
}

pub enum MouseButton {
    Left,
    Right,
    Middle,
}

pub struct DragParams {
    pub from: Point,
    pub to: Point,
    pub duration_ms: Option<u64>,
    pub steps: Option<u32>,
}
```

### E1. `drag` command

**CLI:**
```
agent-desktop drag --from @e5 --to @e12
agent-desktop drag --from @e5 --to-xy 500,300
agent-desktop drag --from-xy 100,200 --to-xy 500,300
agent-desktop drag --from @e5 --to @e12 --duration 500
```

**File:** `crates/core/src/commands/drag.rs`

**Why CGEvent is required:** Pure AX drag is impossible on ALL three platforms. macOS has no `AXDragAction`. Windows `IDragPattern` is read-only (describes draggable state, doesn't perform drags). Linux AT-SPI has no drag interface.

**Implementation (macOS — hybrid AX+CGEvent):**
1. **AX phase:** Resolve source/target coordinates from refs (via `get_element_bounds`) or use raw `--xy` coordinates
2. **CGEvent phase:**
   - `CGEvent::new_mouse_event(LeftMouseDown, source_point)` → post
   - Sleep 50ms (initial hold for drag registration)
   - 10 intermediate `CGEvent::new_mouse_event(LeftMouseDragged, interpolated_point)` events, 8ms apart
   - `CGEvent::new_mouse_event(LeftMouseUp, target_point)` → post
   - Sleep 50ms (settle time for app to process drop)
3. **Total duration:** ~230ms default, configurable via `--duration`

**Cross-platform mapping:**
- macOS: CGEvent `LeftMouseDown` → `LeftMouseDragged` (N steps) → `LeftMouseUp`
- Windows: `SendInput` with `MOUSEEVENTF_LEFTDOWN` → `MOUSEEVENTF_MOVE` (N steps) → `MOUSEEVENTF_LEFTUP`
- Linux: `atspi_generate_mouse_event` with `b1p` → `abs` (N steps) → `b1r`

### E2. `hover` command

**CLI:**
```
agent-desktop hover @e5
agent-desktop hover --xy 500,300
agent-desktop hover @e5 --duration 1000
```

**File:** `crates/core/src/commands/hover.rs`

**Why CGEvent:** Tooltips and hover states require real cursor movement. `CGWarpMouseCursorPosition` is silent and won't trigger tooltips — must use `CGEvent::new_mouse_event(MouseMoved)`.

**Implementation (macOS):**
- Resolve target point (from ref bounds center or `--xy`)
- `CGEvent::new_mouse_event(MouseMoved, target_point)` → post
- If `--duration` specified, hold position for that time (cursor stays put naturally)

### E3. `mouse-down` / `mouse-up` commands

**CLI:**
```
agent-desktop mouse-down @e5 --button left
agent-desktop mouse-up --xy 500,300 --button left
```

**Files:** `crates/core/src/commands/mouse_down.rs`, `crates/core/src/commands/mouse_up.rs`

**Implementation:** Individual CGEvent mouseDown/mouseUp events. Essential for custom drag sequences and long-press patterns.

### E4. `mouse-move` command

**CLI:**
```
agent-desktop mouse-move --xy 500,300
agent-desktop mouse-move --relative -10,20
```

**File:** `crates/core/src/commands/mouse_move.rs`

**Implementation:** `CGEvent::new_mouse_event(MouseMoved)`. `--relative` uses delta from current cursor position (query via `CGEvent::location()`).

### E5. `mouse-click` command

**CLI:**
```
agent-desktop mouse-click --xy 500,300
agent-desktop mouse-click --xy 500,300 --button right --count 2
```

**File:** `crates/core/src/commands/mouse_click.rs`

**Implementation:** Click at absolute coordinates, bypassing ref system. Useful when AX tree doesn't expose an element (e.g., Calculator display, game UIs, custom-rendered views).

### E6. `triple-click` command

**CLI:** `agent-desktop triple-click @e1`

**File:** `crates/core/src/commands/triple_click.rs`

**Implementation:** AX-first attempt: three rapid `kAXPressAction` calls with 30ms sleep between. If that doesn't trigger line/paragraph selection, fall back to CGEvent triple-click at element center.

---

## Phase F: New Commands — Window Geometry

All window geometry commands are **pure AX** — no CGEvent or osascript needed.

### F1. `resize-window`

**CLI:** `agent-desktop resize-window --app TextEdit --width 800 --height 600`

**File:** `crates/core/src/commands/resize_window.rs`

**Implementation (macOS, AX-only):**
- Get window element for app
- `AXUIElementSetAttributeValue(winEl, kAXSizeAttribute, AXValueCreate(kAXValueCGSizeType, &CGSize { width, height }))`

### F2. `move-window`

**CLI:** `agent-desktop move-window --app TextEdit --x 100 --y 100`

**File:** `crates/core/src/commands/move_window.rs`

**Implementation (macOS, AX-only):**
- `AXUIElementSetAttributeValue(winEl, kAXPositionAttribute, AXValueCreate(kAXValueCGPointType, &CGPoint { x, y }))`

### F3. `minimize` / `maximize` / `restore`

**CLI:**
```
agent-desktop minimize --app TextEdit
agent-desktop maximize --app TextEdit
agent-desktop restore --app TextEdit
```

**Files:** `crates/core/src/commands/minimize.rs`, `crates/core/src/commands/maximize.rs`, `crates/core/src/commands/restore.rs`

**Implementation (macOS, AX-only):**
- Minimize: `AXUIElementSetAttributeValue(winEl, kAXMinimizedAttribute, kCFBooleanTrue)`
- Maximize: `AXUIElementPerformAction(zoomButton, kAXPressAction)` where zoomButton is from `kAXZoomButtonAttribute`
- Restore: `AXUIElementSetAttributeValue(winEl, kAXMinimizedAttribute, kCFBooleanFalse)` for minimized windows, `AXUIElementPerformAction(zoomButton, kAXPressAction)` for fullscreen

**Cross-platform:**
- Windows: `IUIAutomationTransformPattern.Move/Resize`, `IUIAutomationWindowPattern.SetWindowVisualState`
- Linux: `atspi_component_set_extents`, window manager D-Bus calls

---

## Phase G: New Commands — Keyboard

### G1. `key-down` / `key-up`

**CLI:**
```
agent-desktop key-down shift
agent-desktop key-up shift
```

**Files:** `crates/core/src/commands/key_down.rs`, `crates/core/src/commands/key_up.rs`

**Implementation:** `AXUIElementPostKeyboardEvent` with only the down or up flag. If that doesn't work for the key, fall back to `CGEventCreateKeyboardEvent` with only key-down or key-up. Essential for modifier-hold sequences (hold Shift while clicking multiple items).

### G2. `clear` command

**CLI:** `agent-desktop clear @e1`

**File:** `crates/core/src/commands/clear.rs`

**Implementation (AX-only):** `AXUIElementSetAttributeValue(el, kAXValueAttribute, "")`. Simple set-value to empty string.

### G3. `clipboard-clear`

**CLI:** `agent-desktop clipboard-clear`

**File:** `crates/core/src/commands/clipboard_clear.rs`

**Implementation (macOS):** `NSPasteboard.generalPasteboard.clearContents()` via Cocoa FFI.

---

## Phase H: Architecture — Adapter Extensibility

### H1. Make Action enum `#[non_exhaustive]`

**File:** `crates/core/src/action.rs`

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Click,
    DoubleClick,
    RightClick,
    SetValue(String),
    SetFocus,
    Expand,
    Collapse,
    Select(String),
    Toggle,
    Scroll(Direction, u32),
    PressKey(KeyCombo),
    TypeText(String),
    Check,
    Uncheck,
    ScrollTo,
    Drag(DragParams),
    Hover,
}
```

Adding `#[non_exhaustive]` means platform adapters must have a `_ => Err(not_supported())` catch-all, which naturally handles new actions added in future phases.

### H2. WindowOp enum for window geometry

**File:** `crates/core/src/action.rs` (or new `crates/core/src/window_op.rs` if action.rs grows)

```rust
pub enum WindowOp {
    Resize { width: f64, height: f64 },
    Move { x: f64, y: f64 },
    Minimize,
    Maximize,
    Restore,
}
```

Single `window_op(&self, win: &WindowInfo, op: WindowOp)` method on PlatformAdapter instead of 5 separate methods.

### H3. MouseEvent struct for raw mouse control

**File:** `crates/core/src/action.rs` (or new `crates/core/src/mouse.rs`)

```rust
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub point: Point,
    pub button: MouseButton,
}

pub enum MouseEventKind {
    Move,
    Down,
    Up,
    Click { count: u32 },
}
```

Single `mouse_event(&self, event: MouseEvent)` method on PlatformAdapter. Handles `mouse-move`, `mouse-down`, `mouse-up`, `mouse-click`, and `hover` through one trait method.

### H4. Coordinate Resolution Helper

**File:** `crates/core/src/commands/coords.rs`

```rust
pub enum CoordSource {
    Ref(String),
    Absolute(Point),
}

pub fn resolve_coords(
    source: &CoordSource,
    adapter: &dyn PlatformAdapter,
) -> Result<Point, AppError> {
    match source {
        CoordSource::Ref(ref_id) => {
            let (entry, handle) = resolve_ref(ref_id, adapter)?;
            let bounds = adapter.get_element_bounds(&handle)?
                .ok_or(AppError::internal("Element has no bounds"))?;
            Ok(Point {
                x: bounds.x + bounds.width / 2.0,
                y: bounds.y + bounds.height / 2.0,
            })
        }
        CoordSource::Absolute(p) => Ok(p.clone()),
    }
}
```

Used by `drag`, `hover`, `mouse-move`, `mouse-click`, `mouse-down`, `mouse-up`. All coordinate-based commands resolve through the same path.

### H5. Action Pre-Check Pattern

**File:** `crates/core/src/commands/action_check.rs`

```rust
pub fn check_action_supported(
    entry: &RefEntry,
    required_roles: &[&str],
    action_name: &str,
    suggestion: &str,
) -> Result<(), AppError> {
    if !required_roles.contains(&entry.role.as_str()) {
        return Err(AppError::Adapter(AdapterError::new(
            ErrorCode::ActionNotSupported,
            format!("'{}' is not supported on role '{}'", action_name, entry.role),
        ).with_suggestion(suggestion)));
    }
    Ok(())
}
```

Used by `toggle`, `check`, `uncheck`, `expand`, `collapse`, `select` to validate role before attempting action.

### H6. Extended PlatformAdapter Trait

**File:** `crates/core/src/adapter.rs`

```rust
pub trait PlatformAdapter: Send + Sync {
    // --- Discovery ---
    fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> { not_supported() }
    fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError> { not_supported() }
    fn get_tree(&self, win: &WindowInfo, opts: &TreeOptions) -> Result<AccessibilityNode, AdapterError> { not_supported() }
    fn resolve_element(&self, entry: &RefEntry) -> Result<NativeHandle, AdapterError> { not_supported() }
    fn check_permissions(&self) -> PermissionStatus { PermissionStatus::Unknown }

    // --- AX Actions (primary execution path) ---
    fn execute_action(&self, handle: &NativeHandle, action: Action) -> Result<ActionResult, AdapterError> { not_supported() }
    fn get_element_bounds(&self, handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> { not_supported() }

    // --- Window Management ---
    fn focus_window(&self, win: &WindowInfo) -> Result<(), AdapterError> { not_supported() }
    fn window_op(&self, win: &WindowInfo, op: WindowOp) -> Result<(), AdapterError> { not_supported() }
    fn launch_app(&self, id: &str, wait: bool) -> Result<WindowInfo, AdapterError> { not_supported() }
    fn close_app(&self, id: &str, force: bool) -> Result<(), AdapterError> { not_supported() }

    // --- Mouse/Coordinate (CGEvent-based, documented exceptions) ---
    fn mouse_event(&self, event: MouseEvent) -> Result<(), AdapterError> { not_supported() }
    fn drag(&self, params: DragParams) -> Result<(), AdapterError> { not_supported() }

    // --- Media ---
    fn screenshot(&self, target: ScreenshotTarget) -> Result<ImageBuffer, AdapterError> { not_supported() }
    fn get_clipboard(&self) -> Result<String, AdapterError> { not_supported() }
    fn set_clipboard(&self, text: &str) -> Result<(), AdapterError> { not_supported() }
    fn clear_clipboard(&self) -> Result<(), AdapterError> { not_supported() }

    // --- App-Level Keyboard ---
    fn press_key_for_app(&self, app: &str, combo: &KeyCombo) -> Result<ActionResult, AdapterError> { not_supported() }
    fn focused_window(&self) -> Result<Option<WindowInfo>, AdapterError> { not_supported() }

    // --- Surface Management ---
    fn wait_for_menu(&self, pid: i32, open: bool, timeout: u64) -> Result<(), AdapterError> { not_supported() }
    fn list_surfaces(&self, pid: i32) -> Result<Vec<SurfaceInfo>, AdapterError> { not_supported() }
}
```

All methods default to `not_supported()`. Phase 2 Windows/Linux adapters implement what they can — everything else gracefully fails with `PLATFORM_NOT_SUPPORTED`.

---

## Implementation Order

### Sprint 1: Critical Bug Fixes — AX-First Foundation (2-3 days)

Priority: Stop commands from lying about success, fix the tree traversal.

1. **A1** — Sub-window tree fix (ONE LINE in `list_windows_impl`)
2. **A9** — Remove CGEvent fallbacks from click/right-click/toggle/select in `actions.rs`
3. **A2** — Role-aware `select` implementation
4. **A3** — Toggle role validation
5. **A6** — Expand/collapse error codes
6. **A10** — Replace CGEvent scroll with AX scroll bar increment/decrement
7. **B3** — Help descriptions (touches `cli.rs` only, low risk, high value)

**Verify:** `cargo clippy --all-targets -- -D warnings && cargo test --workspace`

### Sprint 2: AX-First Keyboard & Window (2-3 days)

Priority: Eliminate remaining osascript and CGEvent keyboard paths.

8. **A11** — Replace CGEvent keyboard with AX (TypeText via kAXValueAttribute, PressKey via menu bar traversal + AXUIElementPostKeyboardEvent)
9. **A12** — Replace osascript in `press_for_app_impl`
10. **A8** — Replace osascript in `focus_window_impl`
11. **A5** — Fix close-app with AX menu bar "Quit" + verification
12. **A4** — Fix launch window detection polling
13. **A7** — Fix get bounds returning null

**After this sprint:** `input.rs` functions `synthesize_key` and `synthesize_text` should be dead code. Remove them and potentially the file.

### Sprint 3: Improvements + New Idempotent Commands (1-2 days)

14. **B1** — list-apps data shape + bundle_id
15. **B2** — is false vs not-applicable
16. **B4** — find unnamed elements
17. **B5** — stale menu items in snapshots
18. **C1** — `check` / `uncheck` commands (AX-only)
19. **C2** — `scroll-to` command (AX-only)
20. **C3** — `wait --text` variant

### Sprint 4: Architecture + Find Enhancements (1-2 days)

21. **H1** — Make Action enum `#[non_exhaustive]`, add new variants
22. **H2** — WindowOp enum
23. **H3** — MouseEvent struct
24. **H4** — Coordinate resolution helper
25. **H5** — Action pre-check pattern
26. **H6** — Extend PlatformAdapter trait
27. **D1-D3** — find --count, --first/--last/--nth, --text

### Sprint 5: Mouse/Coordinate Commands (2-3 days)

28. **E4** — mouse-move
29. **E2** — hover
30. **E5** — mouse-click
31. **E3** — mouse-down / mouse-up
32. **E6** — triple-click
33. **E1** — drag (most complex, depends on mouse-down/up being solid)

### Sprint 6: Window Geometry + Misc (1-2 days)

34. **F1** — resize-window (AX-only)
35. **F2** — move-window (AX-only)
36. **F3** — minimize / maximize / restore (AX-only)
37. **G1** — key-down / key-up
38. **G2** — clear (AX-only)
39. **G3** — clipboard-clear

### Sprint 7: Testing & Polish (1 day)

40. Full agentic test re-run (same task as original test in `tests/agentic_test_notes.md`)
41. Verify all 7 original bugs are fixed
42. Test all new commands end-to-end
43. Test cross-app drag (Finder → TextEdit file drag)
44. Verify no CGEvent imports remain in `actions.rs`
45. Verify no osascript calls remain in `app_ops.rs` except `launch_app_impl` (`open -a`) and `close_app_impl --force` (`pkill`)

---

## Command Count After Implementation

| Category | Phase 1 (existing) | New | Total |
|----------|-------------------|-----|-------|
| App/Window | 5 | 5 (resize, move, minimize, maximize, restore) | 10 |
| Observation | 7 | 3 (find --count/--first/--nth/--text via flags) | 7* |
| Interaction | 11 | 5 (check, uncheck, triple-click, scroll-to, clear) | 16 |
| Mouse/Coord | 0 | 5 (drag, hover, mouse-move, mouse-click, mouse-down/up) | 5 |
| Keyboard | 1 | 2 (key-down, key-up) | 3 |
| Clipboard | 2 | 1 (clipboard-clear) | 3 |
| Wait | 1 | 1 (wait --text via flag) | 1* |
| System | 3 | 0 | 3 |
| Batch | 1 | 0 | 1 |
| **Total** | **31** | **19** | **49** |

*find enhancements and wait --text are flags on existing commands, not new command files

---

## Cross-Platform Adapter Contract

| Pattern | Commands | macOS | Windows | Linux |
|---------|----------|-------|---------|-------|
| **AX Action** | click, toggle, expand, collapse, select, set-value, focus, check, uncheck, scroll, scroll-to, clear | AXUIElementPerformAction / SetAttributeValue | IUIAutomationElement.Invoke/Toggle/ScrollIntoView | atspi_component_do_action |
| **AX Value Set** | type, set-value, clear | AXUIElementSetAttributeValue(kAXValueAttribute) | IUIAutomationValuePattern.SetValue | atspi_editable_text_set_text_contents |
| **AX Menu Traverse** | press (cmd+shortcuts), close-app (graceful) | kAXMenuBarAttribute → walk → kAXPressAction | IUIAutomationElement.FindAll for menu items | atspi_accessible_get_child_at_index |
| **AX Key Post** | press (fallback for non-menu keys) | AXUIElementPostKeyboardEvent | IUIAutomationElement.SendKeys | atspi_generate_keyboard_event |
| **AX Window Prop** | resize, move, minimize, maximize, restore, focus-window | AXUIElementSetAttributeValue (Position/Size/Minimized/Main) | IUIAutomationTransformPattern / WindowPattern | atspi_component_set_extents |
| **CGEvent Mouse** | drag, hover, mouse-move/click/down/up, triple-click | CGEvent sequences | SendInput sequences | atspi_generate_mouse_event |
| **CGEvent Keyboard** | key-down, key-up | CGEvent / AXUIElementPostKeyboardEvent | SendInput VK_ codes | atspi_generate_keyboard_event |
| **AX Attribute Get** | get, is, snapshot, find, wait --text | AXUIElementCopyAttributeValue | IUIAutomationElement.GetPropertyValue | atspi_accessible_get_* |
| **OS Shell** | launch, close-app --force | `open -a` / `pkill` | CreateProcess / TerminateProcess | xdg-open / kill |
| **OS Clipboard** | clipboard-* | NSPasteboard | Win32 Clipboard API | wl-copy / xclip |

---

## Removed / Downgraded Items

### Stderr Duplication (was A1 in original plan — REMOVED)

**Research finding:** There is NO stderr duplication bug in the code. `emit_json()` writes exactly once to stdout via `BufWriter`. `Cli::try_parse()` does NOT write to stderr on its own. The reported "duplication" was most likely `cargo run` output noise (rustc warnings, build progress) or shell stream merging (`2>&1`). The release binary does NOT exhibit this behavior.

**Action:** No code change needed. Verified by reading `main.rs` error paths — all produce exactly 1 JSON object on stdout. If this resurfaces, the diagnostic is: run `./target/release/agent-desktop click @invalid 2>/dev/null | wc -l` and verify it outputs exactly 1 line.

### Commands NOT Added (from agent-browser gap analysis)

These agent-browser commands were evaluated and intentionally excluded:

| Command | Reason for exclusion |
|---------|---------------------|
| `navigate`, `go_back`, `go_forward`, `reload`, `wait_for_page` | Browser-only, no desktop equivalent |
| `evaluate`, `console`, `network` | Browser DevTools, no desktop equivalent |
| `pdf`, `snapshot_html`, `save_html` | Browser DOM, no desktop equivalent |
| `get_cookies`, `set_cookies`, `clear_cookies` | Browser storage, no desktop equivalent |
| `file_upload`, `file_download` | Browser file dialogs — desktop equivalent would be AX file picker interaction, deferred to Phase 2 |
| `dialog_accept`, `dialog_dismiss` | Browser dialogs — desktop alert/dialog handling is possible via AX but complex; deferred to Phase 2 |

---

## Acceptance Criteria

### Bug Fixes
- [ ] `agent-desktop snapshot --app "System Settings"` returns both sidebar AND content pane elements
- [ ] `agent-desktop select @popup "Courier"` actually changes the value and reports new value in response
- [ ] `agent-desktop toggle @textfield` returns `ACTION_NOT_SUPPORTED` (not false success)
- [ ] `agent-desktop launch Calculator` succeeds and returns window info (with polling)
- [ ] `agent-desktop close-app Calculator` reports actual close status, not premature success
- [ ] `agent-desktop expand @combobox` returns `ACTION_NOT_SUPPORTED` with suggestion (not raw AX error)
- [ ] `agent-desktop get @e1 --property bounds` returns actual coordinates (not null)

### AX-First Execution
- [ ] `actions.rs` contains ZERO `cg_mouse_click` calls or CGEvent mouse imports
- [ ] `actions.rs` Click/RightClick/Toggle/Select use only AXUIElementPerformAction
- [ ] `actions.rs` Scroll uses AXScrollBar increment/decrement (no CGEvent::new_scroll_event)
- [ ] `actions.rs` TypeText uses kAXValueAttribute set (no synthesize_text)
- [ ] `app_ops.rs` focus_window uses AX (no osascript)
- [ ] `app_ops.rs` close_app graceful uses AX menu bar (no osascript quit)
- [ ] `app_ops.rs` press_for_app uses AX menu bar traversal (no osascript keystroke)
- [ ] `input.rs` synthesize_key and synthesize_text removed or only used by CGEvent-based commands (drag, hover, mouse-*, key-down/up)

### New Commands
- [ ] `agent-desktop check @checkbox` sets checked state idempotently
- [ ] `agent-desktop uncheck @checkbox` sets unchecked state idempotently
- [ ] `agent-desktop scroll-to @e15` scrolls element into view
- [ ] `agent-desktop wait --text "Done" --app TextEdit --timeout 3000` waits for text appearance
- [ ] `agent-desktop find --app TextEdit --role button --count` returns count
- [ ] `agent-desktop find --app TextEdit --role button --first` returns single element
- [ ] `agent-desktop drag --from @e1 --to @e5` performs drag via CGEvent
- [ ] `agent-desktop hover @e5` moves cursor to element
- [ ] `agent-desktop minimize --app TextEdit` minimizes window via AX
- [ ] `agent-desktop resize-window --app TextEdit --width 800 --height 600` resizes via AX
- [ ] `agent-desktop mouse-click --xy 500,300` clicks at coordinates
- [ ] `agent-desktop key-down shift` holds shift modifier

### Quality
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] No file exceeds 400 LOC
- [ ] `agent-desktop --help` shows descriptions for all commands
- [ ] Error JSON appears exactly once on stdout for all error cases
- [ ] Full agentic test re-run shows 0 bugs, 0 false successes

---

## Files to Change (Summary)

| File | Change |
|------|--------|
| `crates/macos/src/adapter.rs` | ONE-LINE fix for sub-window tree; add `get_element_bounds`; populate `bundle_id` in list_apps |
| `crates/macos/src/actions.rs` | Remove ALL CGEvent mouse fallbacks; rewrite select (role-aware), toggle (role check), scroll (AX scroll bar), type (kAXValueAttribute), press (menu traverse + AXUIElementPostKeyboardEvent) |
| `crates/macos/src/input.rs` | Remove `synthesize_key`, `synthesize_text`; keep file only if CGEvent mouse commands need keyboard helpers, otherwise delete |
| `crates/macos/src/app_ops.rs` | Replace osascript in focus_window, close_app, press_for_app with AX equivalents |
| `crates/core/src/adapter.rs` | Extend PlatformAdapter with `get_element_bounds`, `window_op`, `mouse_event`, `drag`, `clear_clipboard`; add `#[non_exhaustive]` consideration |
| `crates/core/src/action.rs` | Add `#[non_exhaustive]`, new variants: Check, Uncheck, ScrollTo, Drag, Hover; add Point, MouseButton, DragParams, WindowOp, MouseEvent types |
| `src/cli.rs` | Add `#[command(about)]` to all variants; add new command structs for all Phase C-G commands |
| `src/dispatch.rs` | Add match arms for all new commands |
| `crates/core/src/commands/` | New files: `check.rs`, `uncheck.rs`, `scroll_to.rs`, `drag.rs`, `hover.rs`, `mouse_down.rs`, `mouse_up.rs`, `mouse_move.rs`, `mouse_click.rs`, `triple_click.rs`, `resize_window.rs`, `move_window.rs`, `minimize.rs`, `maximize.rs`, `restore.rs`, `key_down.rs`, `key_up.rs`, `clear.rs`, `clipboard_clear.rs`, `coords.rs`, `action_check.rs` |
| `crates/core/src/commands/find.rs` | Add `--count`, `--first`, `--last`, `--nth`, `--text` flags |
| `crates/core/src/commands/wait.rs` | Add `--text` flag |
| `crates/core/src/commands/get.rs` | Fix bounds to query live from AX tree |
| `crates/core/src/commands/is_check.rs` | Add `applicable` field based on role |
| `crates/core/src/commands/toggle.rs` | Add role validation |
| `crates/core/src/commands/list_apps.rs` | Wrap data in `{"apps": [...]}` |

---

## References

- Agentic test notes: `tests/agentic_test_notes.md`
- PRD v2.0: `docs/agent_desktop_prd_v2.pdf`
- Architecture brainstorm: `docs/brainstorms/2026-02-19-architecture-validation-brainstorm.md`
- Agent-browser command inventory: `github.com/vercel-labs/agent-browser` (used for gap analysis)
- macOS AX API: `AXUIElementPerformAction`, `AXUIElementSetAttributeValue`, `AXUIElementCopyActionNames`, `AXUIElementPostKeyboardEvent`
- macOS AX menu traversal: `kAXMenuBarAttribute` → `AXMenuBarItem` → `AXMenu` → `AXMenuItem` → `kAXMenuItemCmdChar` + `kAXMenuItemCmdModifiers`
- macOS AX scroll: `kAXScrollBarAttribute` → `kAXIncrementAction` / `kAXDecrementAction`
- Pure AX drag is impossible on all 3 platforms — CGEvent mouse synthesis required
- `accessibility-sys` crate: provides all kAX constants and AXUIElement FFI bindings
