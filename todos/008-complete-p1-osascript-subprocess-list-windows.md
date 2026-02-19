---
status: pending
priority: p1
issue_id: "008"
tags: [performance, code-review, macos, subprocess]
---

# osascript Subprocess for list_windows (200–500ms per Call)

## Problem Statement

`list_windows_impl` spawns an `osascript` subprocess to enumerate windows. Each subprocess invocation has ~200–500ms cold-start overhead from interpreter initialization. This makes `list-windows` unusable for any workflow that calls it more than once per second, and it re-introduces the command-injection vector (see issues 001–002) via AppleScript in a different method.

## Findings

**File:** `crates/macos/src/adapter.rs:201-265`

The implementation launches `osascript` with an inline AppleScript that calls `tell application "System Events"`. This is:
- **Slow:** 200–500ms per call vs. <1ms for native API
- **Fragile:** System Events must be running and authorized
- **Security risk:** AppleScript interpolation vector

macOS provides `CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID)` which returns all visible windows in a single native call with <1ms latency.

## Proposed Solutions

### Option A: Use CGWindowListCopyWindowInfo (Recommended)
Call `CGWindowListCopyWindowInfo` to get window list. Extract `kCGWindowOwnerName`, `kCGWindowName`, `kCGWindowOwnerPID`, `kCGWindowBounds`. All fields available natively.
- **Effort:** Medium
- **Risk:** Low — stable public API used by macOS system tools

### Option B: Use NSWorkspace + AXUIElementCreateApplication
Enumerate `NSWorkspace.sharedWorkspace.runningApplications`, then for each app call `AXUIElementCreateApplication(pid)` and read `kAXWindowsAttribute` to get window list.
- **Effort:** Small
- **Risk:** Low — requires AX permission already granted

### Option C: Keep osascript but cache results
Cache window list for 500ms. Amortizes the subprocess cost for repeated calls.
- **Effort:** Tiny
- **Risk:** Medium — stale data; doesn't fix security issue; still slow on first call

## Recommended Action

Option A: `CGWindowListCopyWindowInfo` for on-screen windows. Option B as supplementary for off-screen/minimized windows.

## Technical Details

- **File:** `crates/macos/src/adapter.rs`
- **Lines:** 201–265
- **Component:** macOS adapter, `list_windows_impl`

## Acceptance Criteria

- [ ] `list-windows` does not spawn osascript subprocess
- [ ] `list-windows` returns results in < 10ms
- [ ] Window list includes title, app name, pid, bounds for all visible windows

## Work Log

- 2026-02-19: Finding identified by performance-oracle review agent
