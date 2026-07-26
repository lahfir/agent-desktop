---
title: "fix: Add fallback chains and focus guards to all commands"
type: fix
date: 2026-02-21
---

# Add Fallback Chains and Focus Guards to All Commands

> Current contract note (2026-05-12): this historical plan is superseded where it
> treats focus or CGEvent paths as automatic fallbacks for normal ref commands.
> The current command path is headless by default; physical/headed behavior is
> explicit.

## Overview

Audit every command in agent-desktop to ensure: (1) every action has an AX-first multi-step fallback chain, (2) every CGEvent path has a focus guard, and (3) edge cases where AX fails due to missing focus/state are handled. Commands like `type`, `press`, `set-value`, `set-focus`, `expand`, `collapse`, `scroll-to`, and `select` currently have single-strategy or no fallback — they either succeed on the first try or return an error.

## Problem Statement

Three categories of issues:

### 1. Commands with NO fallback chain (single AX strategy, error on failure)

| Command | Current Implementation | Problem |
|---------|----------------------|---------|
| `set-value` | `AXUIElementSetAttributeValue(AXValue)` | Must verify app-observable state and fail clearly if unchanged |
| `set-focus` | `AXUIElementSetAttributeValue(AXFocused, true)` | Fails on some elements, no fallback to click-to-focus |
| `scroll-to` | `AXPerformAction(AXScrollToVisible)` | Fails on elements not in scroll area, no other strategies |
| `clear` | `AXUIElementSetAttributeValue(AXValue, "")` | Same as set-value — fails on read-only |
| `expand` | AXExpand → AXDisclosing=true (2 steps) | Missing: click fallback, AXPress on disclosure triangle |
| `collapse` | AXCollapse → AXDisclosing=false (2 steps) | Missing: click fallback, AXPress on disclosure triangle |
| `select` (popup) | AXPress → search children → AXPress item | Missing: retry with app focus, fallback to AXShowMenu |
| `type` | Set AXFocused → `synthesize_text()` via system-wide AX | No focus guard — text goes to whichever app is frontmost |
| `press` | `synthesize_key()` via system-wide AX | No focus guard — keystroke goes to whatever is focused |

### 2. Edge cases where app focus is needed but not ensured

| Command | Scenario | Impact |
|---------|----------|--------|
| `type @e5 "hello"` | Terminal is focused, TextEdit behind | Text types into Terminal instead of TextEdit |
| `press cmd+s` | Target app not frontmost | Saves wrong app's document |
| `select` (popup) | Popup opens in background app | Menu items can't be found |

### 3. Commands that are correct (no changes needed)

| Command | Why it's fine |
|---------|--------------|
| `click` | Semantic activation chain; CGEvent requires explicit physical policy in current code |
| `double-click` | Uses smart_activate (14-step) |
| `right-click` | 7-step chain with explicit ensure_app_focused |
| `triple-click` | Uses smart_activate (14-step) |
| `scroll` | Semantic scroll chain; wheel input requires explicit physical policy in current code |
| `toggle` / `check` / `uncheck` | Delegates to smart_activate (14-step) |
| `mouse-*` / `drag` / `hover` | Intentionally raw CGEvent (agent controls coordinates) |
| All read-only commands | No action to fail |
| `press --app` | Already has focus guard in key_dispatch.rs |

## Proposed Solution

### Phase 1: Focus Guards for TypeText and PressKey

**File: `crates/macos/src/actions/dispatch.rs`**

#### TypeText — ensure target element's app is focused before typing

Current code (line 125-135):
```rust
Action::TypeText(text) => {
    let cf_attr = CFString::new(kAXFocusedAttribute);
    unsafe { AXUIElementSetAttributeValue(...) };
    crate::input::keyboard::synthesize_text(text)?;
}
```

Change to:
```rust
Action::TypeText(text) => {
    if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
        let _ = crate::system::app_ops::ensure_app_focused(pid);
    }
    let cf_attr = CFString::new(kAXFocusedAttribute);
    unsafe { AXUIElementSetAttributeValue(...) };
    std::thread::sleep(std::time::Duration::from_millis(30));
    crate::input::keyboard::synthesize_text(text)?;
}
```

#### PressKey — ensure target element's app is focused before keystroke

Current code (line 137-139):
```rust
Action::PressKey(combo) => {
    crate::input::keyboard::synthesize_key(combo)?;
}
```

Change to: Use the SAME strategy as `press --app` in `key_dispatch.rs` — try menu bar shortcut first, then AX keyboard event to app (not system-wide), then fall back to system-wide:
```rust
Action::PressKey(combo) => {
    if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
        let _ = crate::system::app_ops::ensure_app_focused(pid);
        let app_el = crate::tree::element_for_pid(pid);
        // Try AX action on focused element first (return/escape/space)
        // Then try menu bar shortcut
        // Then AXUIElementPostKeyboardEvent to app
        // Last: system-wide synthesize_key
    }
    crate::input::keyboard::synthesize_key(combo)?;
}
```

### Phase 2: SetValue / Clear Fallback Chain

**File: `crates/macos/src/actions/dispatch.rs`**

Replace `ax_set_value(el, val)` with a multi-step chain:

```
1. AXUIElementSetAttributeValue(AXValue, val) — direct set
2. If element is a textfield/textarea: focus → select-all → type replacement text
3. If element has AXInsertText action (parameterized): use it
4. Focus + clear via keyboard (Cmd+A, then type new value)
```

```rust
fn smart_set_value(el: &AXElement, val: &str) -> Result<(), AdapterError> {
    // Step 1: Direct AX value set
    if ax_set_value(el, val).is_ok() {
        return Ok(());
    }

    // Step 2: Focus + select all + type
    let role = element_role(el);
    if matches!(role.as_deref(), Some("textfield" | "textarea" | "combobox")) {
        if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
            let _ = crate::system::app_ops::ensure_app_focused(pid);
        }
        let cf_attr = CFString::new(kAXFocusedAttribute);
        let focus_err = unsafe {
            AXUIElementSetAttributeValue(
                el.0, cf_attr.as_concrete_TypeRef(),
                CFBoolean::true_value().as_CFTypeRef(),
            )
        };
        if focus_err == kAXErrorSuccess {
            std::thread::sleep(Duration::from_millis(30));
            // Select all existing text
            crate::input::keyboard::synthesize_key(&KeyCombo {
                key: "a".to_string(),
                modifiers: vec![Modifier::Cmd],
            })?;
            std::thread::sleep(Duration::from_millis(30));
            // Type replacement
            crate::input::keyboard::synthesize_text(val)?;
            return Ok(());
        }
    }

    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        "SetValue failed: element value not settable",
    ).with_suggestion("Try 'click' to focus, then 'type' to enter text"))
}
```

For **Clear**: Same chain but with empty string (select-all + delete):
```
1. AXUIElementSetAttributeValue(AXValue, "")
2. Focus + Cmd+A + Delete
```

### Phase 3: SetFocus Fallback Chain

**File: `crates/macos/src/actions/dispatch.rs`**

Current: single `AXUIElementSetAttributeValue(AXFocused, true)` call.

Add fallback:
```
1. Set AXFocused=true
2. If fail: click the element (smart_activate) — clicking often focuses
3. If fail: Set AXSelected=true (some list elements gain focus via selection)
```

### Phase 4: Expand/Collapse Enhanced Chain

**File: `crates/macos/src/actions/dispatch.rs`**

Current chain: AXExpand → AXDisclosing=true (2 steps).

Enhanced chain:
```
1. AXExpand / AXCollapse action
2. Set AXDisclosing = true/false
3. AXPress on element itself (toggles disclosure state in many apps)
4. AXPress on disclosure triangle child (look for child with subrole AXDisclosureTriangle)
5. smart_activate (click fallback)
```

### Phase 5: Select with Focus Guard

**File: `crates/macos/src/actions/extras.rs`**

For popup/menubutton: ensure app focused before opening popup.

```rust
Some("popupbutton") | Some("menubutton") => {
    // Ensure app is focused so popup menu is visible
    if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
        let _ = crate::system::app_ops::ensure_app_focused(pid);
    }
    ax_press_or_fail(el, "select (open popup)")?;
    std::thread::sleep(Duration::from_millis(200));
    // ... rest of menu search
}
```

### Phase 6: ScrollTo Fallback

**File: `crates/macos/src/actions/dispatch.rs`**

Current: single `AXScrollToVisible` action.

Enhanced:
```
1. AXScrollToVisible on element
2. If fail: scroll parent to bring element into view using ax_scroll()
3. If fail: error with suggestion
```

## Implementation Checklist

### Phase 1: Focus Guards
- [ ] Add `ensure_app_focused(pid)` before `synthesize_text()` in TypeText handler
- [ ] Add 30ms sleep after focus before AXFocused set
- [ ] For PressKey: focus app, try AX action on focused element, try menu bar, then AX keyboard to app, then system-wide fallback
- [ ] Test: open TextEdit behind Terminal, `type @e1 "test"` should type into TextEdit

### Phase 2: SetValue/Clear Chain
- [ ] Extract `smart_set_value(el, val)` function
- [ ] Step 1: Direct AXValue set
- [ ] Step 2: Focus + Cmd+A + type replacement (for text fields)
- [ ] Update Clear to use same chain with empty/delete
- [ ] Test: `set-value @textfield "new text"` on a read-only label should fail gracefully

### Phase 3: SetFocus Chain
- [ ] Add click fallback after AXFocused=true fails
- [ ] Add AXSelected fallback for list items
- [ ] Test: `focus @e5` on an element that doesn't support AXFocused

### Phase 4: Expand/Collapse Chain
- [ ] Add AXPress on element after AXDisclosing fails
- [ ] Add disclosure triangle child search
- [ ] Add smart_activate as last AX resort
- [ ] Test: expand/collapse on Finder sidebar disclosure triangles

### Phase 5: Select Focus Guard
- [ ] Add ensure_app_focused before popup open in select_value
- [ ] Test: select dropdown value in background app

### Phase 6: ScrollTo Fallback
- [ ] Add parent scroll fallback when AXScrollToVisible fails
- [ ] Test: scroll-to on element outside visible area

## File Changes

| File | Change | LOC Impact |
|------|--------|-----------|
| `crates/macos/src/actions/dispatch.rs` | TypeText/PressKey focus, SetValue/Clear/Focus/Expand/Collapse/ScrollTo chains | +80 (~447 → need to extract) |
| `crates/macos/src/actions/extras.rs` | Select focus guard | +5 |

**LOC concern:** dispatch.rs is currently 367 LOC. Adding ~80 LOC pushes to ~447, over the 400 limit. Solution: extract `smart_set_value()` and the enhanced expand/collapse into a new file `crates/macos/src/actions/value_ops.rs` (~80 LOC).

| File | Final LOC |
|------|----------|
| `dispatch.rs` | ~380 (after extracting value_ops) |
| `value_ops.rs` | ~80 (new: smart_set_value, smart_clear, smart_expand, smart_collapse) |
| `extras.rs` | ~400 (unchanged + 5 for focus guard) |

## Verification

```bash
cargo build && cargo clippy --all-targets -- -D warnings

# Phase 1: TypeText focus guard
cargo run -- launch "TextEdit"
cargo run -- launch "Finder"  # Finder now focused
cargo run -- snapshot --app "TextEdit" -i
cargo run -- type @e1 "hello world"  # Should focus TextEdit first, then type

# Phase 1: PressKey focus guard
cargo run -- snapshot --app "TextEdit" -i
cargo run -- press cmd+a  # Should select all in TextEdit, not Finder

# Phase 2: SetValue fallback
cargo run -- snapshot --app "TextEdit" -i
cargo run -- set-value @e1 "replaced text"

# Phase 4: Expand/Collapse
cargo run -- snapshot --app "Finder" -i
cargo run -- expand @eN   # disclosure triangle
cargo run -- collapse @eN

# Phase 5: Select with focus
cargo run -- launch "System Settings"
cargo run -- snapshot --app "System Settings" -i
cargo run -- select @eN "Dark"  # appearance dropdown

# LOC checks
wc -l crates/macos/src/actions/dispatch.rs   # <= 400
wc -l crates/macos/src/actions/value_ops.rs  # <= 400
wc -l crates/macos/src/actions/extras.rs     # <= 400
```

## Execution Order

1. Phase 1: Focus guards for TypeText and PressKey (highest impact, most common edge case)
2. Phase 2: SetValue/Clear fallback chain
3. Phase 3: SetFocus fallback chain
4. Phase 4: Expand/Collapse enhanced chain
5. Phase 5: Select focus guard
6. Phase 6: ScrollTo fallback
7. Build, lint, test everything
8. Update README if chain descriptions changed
