---
status: pending
priority: p2
issue_id: "040"
tags: [code-review, correctness, macos]
---

# launch_app Returns Fabricated WindowInfo (pid:0, id:"w-0") When wait=false

## Problem Statement

`launch_app` with `wait: false` returns a fabricated `WindowInfo` with `pid: 0` and `id: "w-0"`. These values do not correspond to any real window. An agent using this ID for `snapshot --window-id w-0` receives `WindowNotFound`. Any snapshot based on `pid: 0` traverses the kernel process.

## Findings

**File:** `crates/macos/src/adapter.rs:349-356`

```rust
Ok(WindowInfo {
    id: "w-0".into(),
    title: id.to_string(),
    app: id.to_string(),
    pid: 0,
    bounds: None,
    is_focused: true,
})
```

This response tells a lie: the window is claimed as `is_focused: true` when it may not even exist yet. The agent has no way to know the returned data is synthetic.

## Proposed Solutions

### Option A: Return an error when wait=false (Recommended)
Return `AppError` with `ErrorCode::ActionNotSupported` and message "App launched but window not yet available; use wait=true or call list-windows after a delay."
- **Effort:** Tiny
- **Risk:** Low — honest failure beats silent wrong data

### Option B: Block briefly with a short timeout
Wait up to 2 seconds for the app to create its first window, then return. If no window appears, return Option A error.
- **Effort:** Small
- **Risk:** Low

### Option C: Return partial info with explicit placeholder flag
```json
{ "id": null, "pid": actual_pid, "ready": false }
```
Agent can poll `list-windows` until `ready`.
- **Effort:** Small
- **Risk:** Low

## Recommended Action

Option A immediately. Option B as a follow-up improvement.

## Technical Details

- **File:** `crates/macos/src/adapter.rs`
- **Lines:** 349–356
- **Component:** launch_app_impl, no-wait path

## Acceptance Criteria

- [ ] `launch --no-wait` does not return fabricated pid/id
- [ ] The returned data is either real or an explicit error
- [ ] `snapshot --window-id {returned-id}` does not fail silently

## Work Log

- 2026-02-19: Finding identified by data-integrity-guardian review agent
