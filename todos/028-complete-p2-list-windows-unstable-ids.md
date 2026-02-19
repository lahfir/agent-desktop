---
status: pending
priority: p2
issue_id: "028"
tags: [code-review, correctness, macos]
---

# list_windows Assigns Window IDs as Sequential Line Indices (Not Stable)

## Problem Statement

Window IDs are assigned by line number in the `osascript` output (`w-1`, `w-2`, ...). If any filter is applied or lines change order, IDs become inconsistent across calls. The `is_focused` heuristic (first-listed window = focused) is also incorrect.

## Findings

**File:** `crates/macos/src/adapter.rs:229, 249-256`

```rust
id: format!("w-{}", idx + 1),  // index of osascript output line
is_focused: idx == 0,          // first line != focused window
```

Window IDs must be stable identifiers so that `focus-window --window-id w-3` always refers to the same window regardless of how many windows are open. The CGWindow API provides `kCGWindowNumber` (a stable integer per-session) for this purpose.

## Proposed Solutions

### Option A: Use CGWindowNumber as window ID
`CGWindowListCopyWindowInfo` returns `kCGWindowNumber` (stable per session), `kCGWindowOwnerName` (app name), `kCGWindowName` (title), `kCGWindowOwnerPID`. IDs become `w-{cgWindowNumber}`.
- **Effort:** Medium (part of osascript replacement — see issue 008)
- **Risk:** Low

### Option B: Use {pid}-{title-hash} as window ID
Compute `w-{pid}-{hash(title)}`. Stable as long as title and pid don't change.
- **Effort:** Small
- **Risk:** Medium — hash collisions, not deterministic for untitled windows

## Recommended Action

Option A: implement as part of the `CGWindowListCopyWindowInfo` migration (issue 008). Same PR.

## Technical Details

- **File:** `crates/macos/src/adapter.rs`
- **Lines:** 229, 249–256
- **Component:** list_windows, window ID assignment

## Acceptance Criteria

- [ ] Window IDs are stable across consecutive `list-windows` calls
- [ ] `is_focused` reflects the actual frontmost window
- [ ] `focus-window --window-id {id}` reliably targets the correct window

## Work Log

- 2026-02-19: Finding identified by architecture-strategist review agent
