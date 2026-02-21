---
title: "feat: Smart AX-First Click Chain — Universal Element Activation Without CGEvent"
type: feat
date: 2026-02-20
---

# Smart AX-First Click Chain

## Overview

Replace hardcoded `AXPress -> AXConfirm -> CGEvent` fallbacks with a **dynamic, discovery-based activation chain** that queries each element's capabilities at runtime and exhausts every pure-AX strategy before touching the cursor. The goal: any interactive element in any app activates without moving the mouse.

## Problem Statement

The current click implementation tries 2 hardcoded AX actions, then falls back to CGEvent mouse synthesis. CGEvent:
- **Moves the visible cursor** — disruptive to the user
- **Requires the window to be frontmost** — blocks background operation
- **Fails on multi-monitor setups** where bounds are in different coordinate spaces
- **Is non-deterministic by nature** — relies on screen geometry, not semantic interaction

Meanwhile, macOS exposes multiple pure-AX strategies (selection attributes, hierarchy walking, focus+confirm) that would work silently in the background. We just never try them.

## Proposed Solution

A `smart_activate()` function that dynamically discovers and attempts every available AX strategy in priority order, falling through to CGEvent only as absolute last resort.

### The 10-Step Activation Chain

```
1. AXPress on element           (if in action list)
2. AXConfirm on element         (if in action list)
3. AXOpen on element             (if in action list — Finder tree items)
4. AXPick on element             (if in action list — menu items)
5. Set AXSelected=true           (if attribute is writable on element)
6. Set AXSelectedRows=[el]       (on parent table/outline/list)
7. Focus + AXConfirm/AXPress     (set AXFocused, wait, retry)
8. Walk DOWN: try child actions  (first child, then grandchild)
9. Walk UP: try parent actions   (parent, then grandparent)
10. CGEvent click at center      (absolute last resort)
```

Steps 1-9 are pure AX. Step 10 is the existing CGEvent fallback. The chain is **non-deterministic** — it queries `AXUIElementCopyActionNames()` and `AXUIElementIsAttributeSettable()` at runtime for each element, never assuming what an app supports.

## Technical Approach

### Prerequisite: Folder Restructure

Before implementing the activation chain, the macOS crate must be restructured from flat files into the standard subfolder layout (see CLAUDE.md "Platform Crate Folder Structure"). This is a prerequisite because:
- `tree.rs` (512 LOC) and `adapter.rs` (438 LOC) exceed the 400 LOC limit
- The new `activate.rs` file belongs in `actions/`, not at the root

The restructure splits files into `tree/`, `actions/`, `input/`, and `system/` subfolders. All `mod` paths and `crate::` imports update accordingly.

### Architecture

#### New file: `crates/macos/src/actions/activate.rs` (~180 LOC)

Contains the smart activation logic. Lives in the `actions/` subfolder alongside `dispatch.rs` and `extras.rs`.

**Public API:**

```rust
pub fn smart_activate(el: &AXElement) -> Result<(), AdapterError>
pub fn smart_double_activate(el: &AXElement) -> Result<(), AdapterError>
pub fn smart_right_activate(el: &AXElement) -> Result<(), AdapterError>
```

**Internal helpers:**

```rust
fn list_ax_actions(el: &AXElement) -> Vec<String>
fn try_action_from_list(el: &AXElement, actions: &[String], targets: &[&str]) -> bool
fn try_set_selected(el: &AXElement) -> bool
fn try_select_via_parent(el: &AXElement) -> bool
fn try_focus_then_activate(el: &AXElement) -> bool
fn try_child_activation(el: &AXElement) -> bool
fn try_parent_activation(el: &AXElement) -> bool
fn is_attr_settable(el: &AXElement, attr: &str) -> bool
```

#### Modified file: `crates/macos/src/actions/dispatch.rs` (~350 LOC, net decrease)

Replace inline fallback logic with calls to `activate::smart_activate()`:

```rust
Action::Click => {
    crate::actions::activate::smart_activate(el)?;
}
Action::DoubleClick => {
    crate::actions::activate::smart_double_activate(el)?;
}
Action::RightClick => {
    crate::actions::activate::smart_right_activate(el)?;
}
Action::Toggle => {
    crate::actions::activate::smart_activate(el)?;
}
Action::TripleClick => {
    crate::actions::activate::smart_triple_activate(el)?;
}
```

`check_uncheck()` also delegates to `smart_activate()` after its role/value checks.

#### Modified file: `crates/macos/src/actions/mod.rs`

Add `pub mod activate;` registration.

### Implementation Phases

#### Phase 0: Folder Restructure (prerequisite, no behavior change)

Split the flat macOS crate into the standard subfolder layout:

```
crates/macos/src/
├── lib.rs              # mod declarations only
├── adapter.rs          # PlatformAdapter impl (~175 LOC)
├── tree/
│   ├── mod.rs
│   ├── element.rs      # AXElement struct + attribute readers (~180)
│   ├── builder.rs      # build_subtree, tree traversal (~250)
│   ├── roles.rs        # Role mapping (75)
│   ├── resolve.rs      # Element re-identification (~100)
│   └── surfaces.rs     # Surface detection (252)
├── actions/
│   ├── mod.rs
│   ├── dispatch.rs     # perform_action match arms (~350)
│   ├── activate.rs     # NEW — smart AX-first chain (~180)
│   └── extras.rs       # select_value, ax_scroll (182)
├── input/
│   ├── mod.rs
│   ├── keyboard.rs     # Key synthesis (281)
│   ├── mouse.rs        # Mouse events (147)
│   └── clipboard.rs    # Clipboard get/set (55)
└── system/
    ├── mod.rs
    ├── app_ops.rs      # launch, close, focus (165)
    ├── window_ops.rs   # window operations (113)
    ├── key_dispatch.rs # app-targeted key press (105)
    ├── permissions.rs  # permission checks (19)
    ├── screenshot.rs   # screen capture (79)
    └── wait.rs         # wait utilities (77)
```

**Verification:** `cargo build && cargo test --workspace && cargo clippy --all-targets -- -D warnings` — zero behavior change, only file moves and import path updates.

#### Phase 1: Core Smart Activate (~130 LOC in `actions/activate.rs`)

**`list_ax_actions()`** — Query element's action list dynamically:
```
AXUIElementCopyActionNames → iterate CFArray → collect Vec<String>
```
This is the foundation — every decision is based on what the element actually reports.

**`is_attr_settable()`** — Check if a writable attribute exists:
```
AXUIElementIsAttributeSettable(el, attr) → bool
```
Required import from `accessibility_sys`: `AXUIElementIsAttributeSettable`.

**`smart_activate()`** — The 10-step chain:

```
Step 1-4: Query actions once, try matching activation actions in order
Step 5:   is_attr_settable("AXSelected") → set true
Step 6:   copy_element_attr(el, "AXParent") → check parent role →
          is_attr_settable(parent, "AXSelectedRows") → set [el]
Step 7:   set AXFocused=true → sleep(50ms) → retry AXConfirm/AXPress
Step 8:   copy_ax_array(el, "AXChildren") → take(3) → try actions on each
Step 9:   copy_element_attr(el, "AXParent") → try actions, then grandparent
Step 10:  click_via_bounds(el, Left, 1) — existing CGEvent fallback
```

**Key design decisions:**

- **Query actions ONCE per activate call** — store in a local Vec, don't re-query per step
- **Parent role check for step 6** — only try AXSelectedRows on AXTable/AXOutline/AXList roles
  (prevents setting selection attributes on random groups)
- **Child walk limit** — try first 3 children only (avoid expensive deep traversal)
- **Parent walk limit** — 2 levels max (parent + grandparent)
- **Focus settle delay** — 50ms between AXFocused set and retry (matches existing codebase pattern)

#### Phase 2: Specialized Variants (~50 LOC in `actions/activate.rs`)

**`smart_double_activate()`:**
```
1. AXOpen (if in action list)
2. smart_activate() twice with 50ms gap
3. CGEvent double-click fallback
```

**`smart_right_activate()`:**
```
1. AXShowMenu (if in action list)
2. CGEvent right-click fallback (no AX alternative for context menus)
```

**`smart_triple_activate()`:**
```
1. smart_activate() three times with 30ms gaps
2. CGEvent triple-click fallback
```

Right-click has fewer AX alternatives — `AXShowMenu` is the only pure-AX option. CGEvent is a reasonable fallback here since context menus are inherently visual.

#### Phase 3: Wire Into actions/dispatch.rs (net LOC decrease)

Replace the inline `try_ax_action` + `click_via_bounds` chains in `perform_action()` with single-line calls to `activate.rs`. This will **reduce** `dispatch.rs` LOC since the complex fallback logic moves out.

The `try_ax_action()`, `click_via_bounds()`, and `has_ax_action()` functions stay in `dispatch.rs` as they're used by other code paths. But they also get `pub(crate)` visibility for `activate.rs` to call `click_via_bounds()` as the final fallback.

#### Phase 4: Expand/Collapse AXDisclosing Fallback (~15 LOC in actions/dispatch.rs)

For `Action::Expand` and `Action::Collapse`, add `AXDisclosing` attribute as fallback:

```
Expand: AXExpand action → set AXDisclosing=true → error
Collapse: AXCollapse action → set AXDisclosing=false → error
```

This uses `is_attr_settable()` from `actions/activate.rs`. Small addition, stays in `actions/dispatch.rs`.

### Selection via Parent: Building the CFArray

For step 6 (set AXSelectedRows on parent), we need to build a CFArray containing the target element. The pattern from tree.rs:

```rust
// Retain the element, build a single-element array
unsafe { CFRetain(el.0 as CFTypeRef) };
let el_as_cftype = unsafe { CFType::wrap_under_create_rule(el.0 as CFTypeRef) };
let arr = CFArray::from_CFTypes(&[el_as_cftype]);
AXUIElementSetAttributeValue(parent.0, attr, arr.as_CFTypeRef())
```

**Important:** The element must be CFRetained before wrapping, since `from_CFTypes` does its own retain, and `wrap_under_create_rule` takes ownership.

## File Changes Summary

### Phase 0: Folder Restructure (move only, no behavior change)

| Old Path | New Path |
|----------|----------|
| `crates/macos/src/tree.rs` | Split → `tree/element.rs`, `tree/builder.rs` |
| `crates/macos/src/actions.rs` | → `actions/dispatch.rs` |
| `crates/macos/src/action_extras.rs` | → `actions/extras.rs` |
| `crates/macos/src/keyboard.rs` | → `input/keyboard.rs` |
| `crates/macos/src/mouse.rs` | → `input/mouse.rs` |
| `crates/macos/src/clipboard.rs` | → `input/clipboard.rs` |
| `crates/macos/src/roles.rs` | → `tree/roles.rs` |
| `crates/macos/src/surfaces.rs` | → `tree/surfaces.rs` |
| `crates/macos/src/app_ops.rs` | → `system/app_ops.rs` |
| `crates/macos/src/window_ops.rs` | → `system/window_ops.rs` |
| `crates/macos/src/key_dispatch.rs` | → `system/key_dispatch.rs` |
| `crates/macos/src/permissions.rs` | → `system/permissions.rs` |
| `crates/macos/src/screenshot.rs` | → `system/screenshot.rs` |
| `crates/macos/src/wait.rs` | → `system/wait.rs` |

Plus extract `tree/resolve.rs` from `adapter.rs` (element re-identification logic).

### Phases 1-4: Smart Activation Chain

| File | Action | LOC Change |
|------|--------|------------|
| `crates/macos/src/actions/activate.rs` | **Create** | +180 |
| `crates/macos/src/actions/dispatch.rs` | Modify (simplify) | -30 (~350 LOC) |
| `crates/macos/src/actions/mod.rs` | Add `pub mod activate` | +1 |

**Net:** +151 LOC. No file exceeds 400 LOC.

## Acceptance Criteria

### Functional Requirements

- [ ] `click @ref` on System Settings treeitems works without moving cursor (AXSelected or AXSelectedRows)
- [ ] `click @ref` on standard buttons still works via AXPress (no regression)
- [ ] `click @ref` on Finder sidebar items works (AXOpen or AXSelected)
- [ ] `click @ref` on custom SwiftUI controls works (hierarchy walking or focus+confirm)
- [ ] `double-click @ref` works via AXOpen or repeated activation
- [ ] `right-click @ref` works via AXShowMenu where possible
- [ ] `toggle`, `check`, `uncheck` use smart activation
- [ ] `expand`/`collapse` falls back to AXDisclosing attribute

### Non-Functional Requirements

- [ ] No app-specific code — all behavior is discovered at runtime via `AXUIElementCopyActionNames` and `AXUIElementIsAttributeSettable`
- [ ] CGEvent is only reached when ALL pure-AX strategies fail
- [ ] No file exceeds 400 LOC
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes

### Quality Gates

- [ ] Smoke test on 5+ apps: System Settings, Finder, TextEdit, Safari, Xcode
- [ ] Verify cursor does NOT move during activation on at least 3 test cases

## Verification Plan

```bash
cargo build
cargo clippy --all-targets -- -D warnings
cargo fmt --all
cargo test --workspace

# Smoke tests
cargo run -- launch "System Settings"
cargo run -- snapshot --app "System Settings" -i
cargo run -- click @eN   # Appearance treeitem — should NOT move cursor
cargo run -- click @eN   # Light button — should activate

cargo run -- launch "Finder"
cargo run -- snapshot --app "Finder" -i
cargo run -- click @eN   # Sidebar item

cargo run -- launch "TextEdit"
cargo run -- snapshot --app "TextEdit" -i
cargo run -- click @eN   # Toolbar button

wc -l crates/macos/src/actions/activate.rs  # must be <= 400
wc -l crates/macos/src/actions/dispatch.rs  # must be <= 400
```

## References

- Brainstorm research: AX action catalog (11 documented + app-specific custom actions)
- `AXUIElementIsAttributeSettable` available in `accessibility-sys 0.2.0`
- Existing parent-walking pattern: `actions/extras.rs` (`find_scroll_area`)
- Existing action list query: `actions/dispatch.rs` (`has_ax_action`)
- `copy_element_attr` supports "AXParent": `tree/element.rs`
