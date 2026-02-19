---
status: pending
priority: p1
issue_id: "006"
tags: [security, code-review, command-injection, macos]
---

# Arbitrary App Launch via open -a

## Problem Statement

`launch_app_impl` passes a user-supplied `id` string directly to `open -a {id}`, allowing an agent to launch any `.app` bundle on the system by name. There is no allowlist, validation, or sandboxing. Combined with the pkill issue (003), this means the agent can launch and terminate arbitrary applications.

## Findings

**File:** `crates/macos/src/adapter.rs:326-329`

```rust
Command::new("open")
    .arg("-a")
    .arg(&args.id)
    .status()
```

`open -a` accepts any application name or path. Inputs like `Terminal`, `/Applications/Utilities/Terminal.app`, or `../../../Applications/SomeApp` can all launch unintended applications. The calling AI agent may be tricked (prompt injection) into launching an app that facilitates further attacks.

## Proposed Solutions

### Option A: Use NSWorkspace to launch by bundle ID (Recommended)
`NSWorkspace.open(withBundleIdentifier:)` accepts only bundle identifiers (e.g., `com.apple.finder`), which are registered in the system's application database. Arbitrary paths are rejected. Bundle IDs can be validated against a regex.
- **Effort:** Small
- **Risk:** Low

### Option B: Validate id against strict regex before open -a
Require `id` to match `^[a-zA-Z0-9][a-zA-Z0-9._-]{1,255}$` (bundle ID format). Reject paths (containing `/` or `..`).
- **Effort:** Tiny
- **Risk:** Medium — still invokes open, but constrains inputs

### Option C: Add optional allowlist configuration
Load an allowlist from config file. Only apps in the list may be launched.
- **Effort:** Medium
- **Risk:** Low — good defense in depth, but deferred to Phase 4

## Recommended Action

Option A: use `NSWorkspace` for launch. Combine with Option B-style bundle-ID validation as defense-in-depth.

## Technical Details

- **File:** `crates/macos/src/adapter.rs`
- **Lines:** 326–329
- **Component:** macOS adapter, `launch_app_impl`

## Acceptance Criteria

- [ ] App launch does not invoke `open -a` with unvalidated input
- [ ] Path inputs (containing `/` or `..`) are rejected with `INVALID_ARGS`
- [ ] Launch by bundle ID works for standard system apps (Finder, TextEdit, Safari)

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
