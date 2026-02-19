---
status: pending
priority: p1
issue_id: "003"
tags: [security, code-review, command-injection, macos]
---

# pkill -f with Arbitrary User Input

## Problem Statement

The force-close path in `close_app_impl` passes an unvalidated user-supplied string directly to `pkill -f`. `pkill -f` pattern-matches against the full command line of every running process. A crafted input can match and kill unrelated processes (e.g., passing `""` kills all processes the user owns), causing denial of service or system instability.

## Findings

**File:** `crates/macos/src/adapter.rs:370-374`

```rust
Command::new("pkill")
    .arg("-f")
    .arg(&args.id)
    .status()
```

With `-f` flag, `pkill` matches against the full command-line string of every running process. Input `""` (empty string) would match everything. Input `"-9"` could cause unexpected flag interpretation.

## Proposed Solutions

### Option A: Use kill(pid, SIGKILL) directly (Recommended)
Look up the PID via `NSRunningApplication(bundleIdentifier:)` or process enumeration, then use libc `kill(pid, SIGKILL)`. No subprocess, no pattern matching.
- **Effort:** Small
- **Risk:** Low

### Option B: Use pkill with exact bundle-ID validation
Validate `id` against strict regex `^[a-zA-Z0-9][a-zA-Z0-9._-]{0,255}$` before passing to pkill. Replace `-f` with `-x` (exact match on process name, not full cmdline).
- **Effort:** Small
- **Risk:** Medium — subprocess invocation remains, but risk is bounded

### Option C: Use `kill` command with PID
Look up PID, pass `-{signal} {pid}` — but still a subprocess.
- **Effort:** Small
- **Risk:** Medium

## Recommended Action

Option A: use `libc::kill(pid, libc::SIGKILL)` after PID lookup. Completely eliminates the process-matching risk.

## Technical Details

- **File:** `crates/macos/src/adapter.rs`
- **Lines:** 370–374
- **Component:** macOS adapter, `close_app_impl` force path

## Acceptance Criteria

- [ ] No subprocess invocation with user-supplied strings in close_app
- [ ] Force-close terminates the correct process by PID
- [ ] Passing empty string or special characters does not affect unrelated processes

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
