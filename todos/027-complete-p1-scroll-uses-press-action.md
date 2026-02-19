---
status: pending
priority: p1
issue_id: "027"
tags: [code-review, correctness, commands, macos]
---

# Scroll Action Uses kAXPressAction Instead of Scroll Event

## Problem Statement

The `scroll` command is implemented as an AX press action, not a scroll event. The direction and amount parameters are silently discarded. Any agent using `scroll @e5 --direction down --amount 5` will click the element instead of scrolling.

## Findings

**File:** `crates/macos/src/actions.rs:82-85`

```rust
Action::Scroll(_, _) => {
    let ax_action = CFString::new(kAXPressAction);
    let _ = unsafe { AXUIElementPerformAction(el.0, ax_action.as_concrete_TypeRef()) };
}
```

`kAXPressAction` performs a click. `Direction` and `u32` scroll amount are received but immediately pattern-matched with `_`. Scroll requires `CGEventCreateScrollWheelEvent(source, kCGScrollEventUnitLine, 2, deltaY, deltaX, 0)` dispatched to the window.

## Proposed Solutions

### Option A: Implement via CGEventCreateScrollWheelEvent (Recommended)
```rust
Action::Scroll(direction, amount) => {
    let (dx, dy) = match direction {
        Direction::Down  => (0, -(amount as i32)),
        Direction::Up    => (0,  (amount as i32)),
        Direction::Right => (-(amount as i32), 0),
        Direction::Left  => ( (amount as i32), 0),
    };
    // CGEventCreateScrollWheelEvent + CGEventPost
}
```
- **Effort:** Small
- **Risk:** Low — standard macOS input event API

### Option B: Use AXScrollArea scroll actions
Some scrollable elements expose `kAXScrollDownAction`, `kAXScrollUpAction`, etc. as AX actions. More limited but simpler.
- **Effort:** Tiny
- **Risk:** Medium — not all scroll targets support AX scroll actions

## Recommended Action

Option A: use `CGEventCreateScrollWheelEvent`. Correct, universal, matches how real input works.

## Technical Details

- **File:** `crates/macos/src/actions.rs`
- **Lines:** 82–85
- **Component:** scroll action implementation

## Acceptance Criteria

- [ ] `scroll @e5 --direction down --amount 3` scrolls 3 lines down
- [ ] Direction and amount parameters are used (not discarded)
- [ ] The element is not clicked as a side effect

## Work Log

- 2026-02-19: Finding identified by architecture-strategist review agent
