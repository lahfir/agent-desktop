---
status: pending
priority: p1
issue_id: "004"
tags: [security, code-review, unsoundness, rust, concurrency]
---

# Unsound unsafe impl Send+Sync for NativeHandle

## Problem Statement

`NativeHandle` wraps a raw `*const c_void` (AXUIElementRef) and has `unsafe impl Send for NativeHandle {}` and `unsafe impl Sync for NativeHandle {}`. AXUIElement is a Core Foundation object that is NOT thread-safe — it must be accessed only from the thread that created it. This will cause undefined behavior when Phase 3 introduces a tokio async runtime, where handles could cross thread boundaries.

## Findings

**File:** `crates/core/src/adapter.rs:66-69`

```rust
unsafe impl Send for NativeHandle {}
unsafe impl Sync for NativeHandle {}
```

AXUIElement documentation explicitly states it is not thread-safe. The `PhantomData<*const ()>` was added precisely to prevent auto-derived Send/Sync, but the manual unsafe impls override that protection. This is a soundness hole — the compiler cannot catch it.

In Phase 3, tokio tasks run on a thread pool. Any `await` point can move the task to a different thread, causing an AXUIElement to be accessed from a thread that didn't create it → crash or UB.

## Proposed Solutions

### Option A: Remove unsafe impls; make NativeHandle !Send + !Sync (Recommended)
Remove both `unsafe impl` blocks. This propagates to the adapter trait: mark all platform methods with `&self` (not `&mut self`) and gate concurrent access at the Phase 3 dispatch layer with a `Mutex<Box<dyn PlatformAdapter>>` or dedicated AX thread.
- **Effort:** Medium (touches adapter trait signature)
- **Risk:** Low — forces correct threading model explicitly

### Option B: Run all AX operations on a dedicated thread
Keep `!Send + !Sync`, create a dedicated "AX thread" in Phase 3. Send work items as messages (channel), execute on AX thread, send results back.
- **Effort:** Medium
- **Risk:** Low — matches Apple's recommended pattern for AX

### Option C: Use a thread-local AXUIElement cache
Store elements in a thread-local, identified by ref ID. Resolve on same thread always.
- **Effort:** Large
- **Risk:** Medium

## Recommended Action

Option A now (remove unsafe impls), Option B in Phase 3 (AX dispatcher thread). Document threading constraint in PlatformAdapter trait doc-comment.

## Technical Details

- **File:** `crates/core/src/adapter.rs`
- **Lines:** 66–69
- **Component:** NativeHandle, core adapter types

## Acceptance Criteria

- [ ] `NativeHandle` does not implement `Send` or `Sync`
- [ ] Compilation still succeeds (adapter trait adjusted accordingly)
- [ ] Threading model is documented in a `///` doc-comment on `NativeHandle`

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
