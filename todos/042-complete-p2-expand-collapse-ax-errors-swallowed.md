---
status: pending
priority: p2
issue_id: "042"
tags: [code-review, correctness, macos, error-handling]
---

# Expand/Collapse/Select/Scroll Silently Swallow AX Errors — Return False Success

## Problem Statement

Four action implementations use `let _ =` to discard the `AXError` return code from `AXUIElementPerformAction`. If the AX call fails (element not actionable, permission changed, element destroyed), the function returns `Ok(ActionResult::new(...))` — a success. The agent receives `"ok": true` and proceeds on false confirmation. `Click` and `SetValue` correctly check the error; Expand/Collapse/Select/Scroll do not.

## Findings

**File:** `crates/macos/src/actions.rs:70-85`

```rust
Action::Expand => {
    let ax_action = CFString::new("AXExpand");
    let _ = unsafe { AXUIElementPerformAction(el.0, ax_action.as_concrete_TypeRef()) };
    // error discarded — always returns Ok below
}
Action::Collapse => { /* same pattern */ }
Action::Select(_) => { /* maps to kAXPressAction, error discarded */ }
Action::Scroll(_, _) => { /* maps to kAXPressAction, error discarded */ }
```

Compare to Click (correct):
```rust
let err = unsafe { AXUIElementPerformAction(el.0, ax_action.as_concrete_TypeRef()) };
if err != kAXErrorSuccess {
    return Err(AdapterError::action_failed("click", err));
}
```

Additionally, Select maps to `kAXPressAction` (a click) discarding the option value, and Scroll maps to `kAXPressAction` discarding direction/amount (covered separately in issue 027).

## Proposed Solutions

### Option A: Apply the same error-check pattern as Click (Recommended)
```rust
Action::Expand => {
    let err = unsafe { AXUIElementPerformAction(el.0, CFString::new("AXExpand").as_concrete_TypeRef()) };
    if err != kAXErrorSuccess {
        return Err(AdapterError::action_failed("expand", err));
    }
}
```
- **Effort:** Small
- **Risk:** Low

## Recommended Action

Option A: apply the click error-check pattern to all four actions.

## Technical Details

- **File:** `crates/macos/src/actions.rs`
- **Lines:** 70–85
- **Component:** Expand, Collapse, Select, Scroll action handlers

## Acceptance Criteria

- [ ] All four actions propagate AX errors as `AppError`
- [ ] An element that does not support Expand returns `ACTION_NOT_SUPPORTED`
- [ ] `ok: true` is only returned when the AX action actually succeeded

## Work Log

- 2026-02-19: Finding identified by data-integrity-guardian review agent
