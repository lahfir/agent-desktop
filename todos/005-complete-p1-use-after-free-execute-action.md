---
status: pending
priority: p1
issue_id: "005"
tags: [security, code-review, memory-safety, use-after-free, macos]
---

# Use-After-Free Risk in execute_action_impl

## Problem Statement

`execute_action_impl` calls `CFRetain` on the handle's raw pointer before use, but does not check whether the pointer is null or already invalid. An AXUIElement becomes stale when the underlying UI element is destroyed (window closed, app quit). Using a stale element pointer is undefined behavior in C FFI — it can crash or, in a use-after-free scenario, access reallocated memory.

## Findings

**File:** `crates/macos/src/adapter.rs:107-117`

The code performs:
```rust
let element_ref = handle.as_raw() as AXUIElementRef;
CFRetain(element_ref as *const c_void);  // UB if element_ref is null or stale
// ... perform action
```

AXUIElement does not provide a validity check API. The correct pattern is:
1. Check for null before CFRetain
2. Use `AXUIElementCopyAttributeValue(element_ref, kAXRoleAttribute, ...)` as a liveness probe — it returns `kAXErrorInvalidUIElement` for stale refs
3. Return `STALE_REF` error code on invalid element

## Proposed Solutions

### Option A: Add null check + AX liveness probe (Recommended)
Check `element_ref.is_null()` first. Then probe with a cheap AX attribute read. If `kAXErrorInvalidUIElement` is returned, map to `AppError` with code `STALE_REF`.
- **Effort:** Small
- **Risk:** Low

### Option B: Wrap in catch_unwind
Wrap the action execution in `std::panic::catch_unwind`. Converts crashes to panics and then to errors.
- **Effort:** Small
- **Risk:** Medium — catch_unwind does not prevent UB; it only catches panics, not segfaults

### Option C: Maintain a validity epoch
Track a snapshot epoch; invalidate all handles on each new snapshot. Return `STALE_REF` if epoch mismatch.
- **Effort:** Medium
- **Risk:** Low — complements Option A

## Recommended Action

Option A: null check + liveness probe before CFRetain. This is the standard pattern for AXUIElement safety.

## Technical Details

- **File:** `crates/macos/src/adapter.rs`
- **Lines:** 107–117
- **Component:** macOS adapter, `execute_action_impl`

## Acceptance Criteria

- [ ] Null check before CFRetain
- [ ] Stale element returns `AppError` with code `STALE_REF`, not a crash
- [ ] Test: performing an action after closing the target window returns STALE_REF

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
