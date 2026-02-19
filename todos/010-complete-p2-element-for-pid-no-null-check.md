---
status: pending
priority: p2
issue_id: "010"
tags: [security, code-review, null-safety, macos]
---

# resolve_element_impl No Null Check on element_for_pid

## Problem Statement

`resolve_element_impl` calls `AXUIElementCreateApplication(pid)` but does not check whether the returned element is null before using it. If `pid` refers to a terminated process, this returns null, and subsequent attribute reads are UB.

## Findings

**File:** `crates/macos/src/adapter.rs:125-128`

```rust
let app_element = AXUIElementCreateApplication(pid);
// No null check here
let role = copy_string_attr(app_element, "AXRole");  // UB if app_element is null
```

A race condition between snapshot (when PID was valid) and action execution (after process terminated) can trigger this path regularly in production.

## Proposed Solutions

### Option A: Null check + ELEMENT_NOT_FOUND error
Check `app_element.is_null()`. If null, return `AppError` with `ErrorCode::ElementNotFound` and suggestion to re-run snapshot.
- **Effort:** Tiny
- **Risk:** Low

### Option B: Verify PID is alive first
Call `kill(pid, 0)` to check if process is still running before `AXUIElementCreateApplication`. Return `APP_NOT_FOUND` if dead.
- **Effort:** Small
- **Risk:** Low

## Recommended Action

Option A (null check on element) + Option B (PID liveness check) together. Defense in depth.

## Technical Details

- **File:** `crates/macos/src/adapter.rs`
- **Lines:** 125–128
- **Component:** macOS adapter, `resolve_element_impl`

## Acceptance Criteria

- [ ] Null pointer is checked before use
- [ ] Stale PID returns structured error, not a crash
- [ ] Error code is `ELEMENT_NOT_FOUND` or `APP_NOT_FOUND` as appropriate

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
