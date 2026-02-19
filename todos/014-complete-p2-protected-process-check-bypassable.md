---
status: pending
priority: p2
issue_id: "014"
tags: [security, code-review, access-control]
---

# Protected-Process Check Is Substring-Based and Easily Bypassed

## Problem Statement

`close_app` has a "protected process" check that prevents killing critical system processes, but it uses substring matching on the app name string. This is bypassable: an app named `Finder-safe` would match `Finder` and be blocked; an app named `Finder_` would not. More importantly, the check lives only in the command layer, not in the adapter, so future refactoring could bypass it.

## Findings

**File:** `crates/core/src/commands/close_app.rs`

The protection is implemented as a string contains check:
```rust
if PROTECTED_APPS.iter().any(|p| args.id.contains(p)) {
    return Err(AppError::permission_denied(...));
}
```

Issues:
1. Substring match: `"windowserver"` blocks `"com.apple.windowserver"` but not `"WindowServer"` (case-sensitive)
2. False positive: `"FinderExtension"` blocked because it contains `"Finder"`
3. Check is in command layer only — adapter layer has no guard

## Proposed Solutions

### Option A: Move check to adapter + use exact bundle-ID matching (Recommended)
Define `PROTECTED_BUNDLE_IDS: &[&str]` with exact bundle IDs (`com.apple.finder`, `com.apple.dock`). Match against `NSRunningApplication.bundleIdentifier` (not user-supplied name).
- **Effort:** Small
- **Risk:** Low

### Option B: Case-insensitive exact match
Convert both sides to lowercase, use exact equality not contains. Prevents false positives on substrings.
- **Effort:** Tiny
- **Risk:** Medium — still in command layer

### Option C: OS-level validation
Rely on macOS SIP (System Integrity Protection) to prevent termination of system-protected processes. Don't implement our own check.
- **Effort:** None
- **Risk:** Low — but SIP doesn't protect all system apps

## Recommended Action

Option A: exact bundle-ID matching in adapter layer. More robust and harder to bypass.

## Technical Details

- **File:** `crates/core/src/commands/close_app.rs`
- **Component:** close_app command, macOS adapter

## Acceptance Criteria

- [ ] Protected app check uses exact bundle identifier, not substring
- [ ] `com.apple.finder` is blocked; `FinderExtension` is not
- [ ] Check is enforced at adapter level, not only command level

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
