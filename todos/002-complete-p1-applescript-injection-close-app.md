---
status: pending
priority: p1
issue_id: "002"
tags: [security, code-review, injection, macos]
---

# AppleScript Injection in close_app_impl (Graceful Path)

## Problem Statement

`close_app_impl` interpolates an untrusted `id` (app bundle identifier or name) directly into an AppleScript string. Same injection vector as issue 001 but on the close path.

## Findings

**File:** `crates/macos/src/adapter.rs:376-380`

```rust
let script = format!(r#"tell application "{id}" to quit"#);
```

The `id` parameter arrives from the CLI as an unvalidated string. An attacker (or misbehaving caller) can pass `Finder" to do shell script "malicious" tell application "Finder` to escape the AppleScript context.

## Proposed Solutions

### Option A: Use NSRunningApplication (Recommended)
Use `NSRunningApplication` API to find the app by bundle ID and call `-[NSRunningApplication terminate]`. No AppleScript needed.
- **Effort:** Small
- **Risk:** Low

### Option B: Use SIGTERM via kill(pid, SIGTERM)
Use the known PID (looked up from NSRunningApplication or the process table) and send SIGTERM directly.
- **Effort:** Small
- **Risk:** Low — avoids all shell/AppleScript entirely

### Option C: Sanitize input
Validate that `id` matches a strict bundle-ID or process-name regex (`[a-zA-Z0-9._-]+`) before interpolating.
- **Effort:** Tiny
- **Risk:** Medium — still relies on AppleScript; regex can miss edge cases

## Recommended Action

Option A: replace with `NSRunningApplication` + `terminate`. Pairs naturally with fix for issue 001.

## Technical Details

- **File:** `crates/macos/src/adapter.rs`
- **Lines:** 376–380
- **Component:** macOS adapter, `close_app_impl` graceful path

## Acceptance Criteria

- [ ] `close_app_impl` does not invoke osascript or AppleScript string interpolation
- [ ] App can be closed by bundle identifier containing dots, hyphens, underscores
- [ ] Force-close path (pkill) is also addressed (see issue 003)

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
