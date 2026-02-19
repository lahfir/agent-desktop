---
status: pending
priority: p1
issue_id: "001"
tags: [security, code-review, injection, macos]
---

# AppleScript Injection in focus_window_impl

## Problem Statement

`focus_window_impl` interpolates an untrusted `app_name` field directly into an AppleScript string, allowing arbitrary AppleScript execution. An AI agent controlling desktop apps with malicious data in window titles or app names can execute arbitrary code on the host machine.

## Findings

**File:** `crates/macos/src/adapter.rs:306-308`

```rust
let script = format!(r#"tell application "{}" to activate"#, win.app);
```

The `win.app` field comes from `WindowInfo.app_name`, populated from AXUIElement attributes queried from the OS. A malicious or compromised app can set its `AXTitle` to something like `Finder" to do shell script "rm -rf ~/Documents" tell application "Finder` and escape the AppleScript string context entirely.

## Proposed Solutions

### Option A: Use NSRunningApplication (Recommended)
Replace osascript with the macOS-native `NSRunningApplication` API, using PID-based lookup to call `-[NSRunningApplication activateWithOptions:]`. No shell, no string interpolation.
- **Effort:** Small
- **Risk:** Low — uses a stable AppKit API

### Option B: Quote-escape the app name
Escape double quotes in `win.app` before interpolation: `win.app.replace('"', r#"\""#)`.
- **Effort:** Tiny
- **Risk:** Medium — still relies on shell/AppleScript execution; incomplete mitigation

### Option C: Use PID-based focus via CGWindowLevel
Call `CGWindowListCopyWindowInfo` + `SetFrontProcess` using the PID from `WindowInfo.pid`.
- **Effort:** Medium
- **Risk:** Low

## Recommended Action

Implement Option A: use `NSRunningApplication` via Cocoa FFI with PID-based activation to completely eliminate AppleScript for focus operations.

## Technical Details

- **File:** `crates/macos/src/adapter.rs`
- **Line:** 306–308
- **Component:** macOS adapter, `focus_window_impl`

## Acceptance Criteria

- [ ] `focus_window_impl` does not invoke osascript or AppleScript string interpolation
- [ ] Focus works correctly for apps with special characters in their name
- [ ] Existing integration tests pass

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
