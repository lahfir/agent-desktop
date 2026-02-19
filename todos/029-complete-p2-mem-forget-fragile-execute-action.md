---
status: pending
priority: p2
issue_id: "029"
tags: [code-review, memory-safety, macos]
---

# execute_action_impl Uses mem::forget — Fragile Memory Management

## Problem Statement

`execute_action_impl` calls `CFRetain` on the AX element pointer, wraps it in `AXElement`, then calls `std::mem::forget(el)` to prevent the destructor from releasing it. This is correct only when `perform_action` succeeds. On the error path (`?`), `forget` is never reached, so `drop` calls `CFRelease`, netting zero change — correct by accident. This pattern is extremely fragile: any change to the error path could silently create a double-release or memory leak.

## Findings

**File:** `crates/macos/src/adapter.rs:112-116`

```rust
unsafe { CFRetain(handle.as_raw() as CFTypeRef) };
let el = AXElement(handle.as_raw() as AXUIElementRef);
let result = crate::actions::perform_action(&el, &action)?;
std::mem::forget(el);   // Only reached on success path
```

If `perform_action` returns an error, `?` returns early. `el` goes out of scope, calling `CFRelease`. Since `CFRetain` was called above, this results in a net no-op — but only by coincidence of control flow, not by design.

## Proposed Solutions

### Option A: Use ManuallyDrop instead of forget (Recommended)
```rust
let el = ManuallyDrop::new(AXElement(handle.as_raw() as AXUIElementRef));
let result = crate::actions::perform_action(&*el, &action)?;
// ManuallyDrop prevents drop whether success or error
```
- **Effort:** Tiny
- **Risk:** Low — explicit, correct, and self-documenting

### Option B: Don't retain; use a reference
If `el` borrows the pointer without taking ownership (no CFRetain), the `AXElement` destructor's `CFRelease` would be incorrect. Instead, don't create an `AXElement` at all — call the AX API directly with the raw pointer.
- **Effort:** Small
- **Risk:** Low

## Recommended Action

Option A: `ManuallyDrop`. Makes intent explicit and correct for both success and error paths.

## Technical Details

- **File:** `crates/macos/src/adapter.rs`
- **Lines:** 112–116
- **Component:** execute_action_impl

## Acceptance Criteria

- [ ] Memory management is correct on both success and error paths
- [ ] No `std::mem::forget` in action execution path
- [ ] Comment explains retain/release semantics

## Work Log

- 2026-02-19: Finding identified by architecture-strategist review agent
