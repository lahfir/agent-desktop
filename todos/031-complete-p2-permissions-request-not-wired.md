---
status: pending
priority: p2
issue_id: "031"
tags: [code-review, correctness, commands]
---

# permissions --request Flag Does Not Call Adapter (System Dialog Never Triggered)

## Problem Statement

The `permissions --request` flag returns a stub response claiming the dialog was triggered, but never calls any adapter method. The macOS adapter has a working `check_with_request()` function that calls `AXIsProcessTrustedWithOptions(prompt: true)`, but it is never invoked.

## Findings

**File:** `crates/core/src/commands/permissions.rs:9-13`

```rust
if args.request {
    return Ok(json!({
        "requested": true,
        "note": "Permission dialog triggered via --request flag"
    }));
}
```

The `PlatformAdapter` is not even passed to this code path. The macOS adapter has `check_with_request()` implemented in `crates/macos/src/permissions.rs` but it is not exposed via the `PlatformAdapter` trait.

## Proposed Solutions

### Option A: Add request_permissions() to PlatformAdapter (Recommended)
```rust
fn request_permissions(&self) -> PermissionStatus {
    self.check_permissions()  // default: no-op
}
```
MacOS adapter overrides this to call `AXIsProcessTrustedWithOptions`. The command calls `adapter.request_permissions()`.
- **Effort:** Small
- **Risk:** Low

### Option B: Call check_with_request via downcast
Downcast `&dyn PlatformAdapter` to `MacOSAdapter` when on macOS. Fragile.
- **Effort:** Small
- **Risk:** High — breaks dependency inversion

## Recommended Action

Option A: add `request_permissions()` to the trait with a default no-op implementation.

## Technical Details

- **File:** `crates/core/src/commands/permissions.rs:9-13`, `crates/macos/src/permissions.rs`
- **Component:** permissions command, PlatformAdapter trait

## Acceptance Criteria

- [ ] `permissions --request` triggers the system accessibility permission dialog on macOS
- [ ] `PlatformAdapter` trait has `request_permissions()` method
- [ ] Response reflects the actual permission state after the dialog interaction

## Work Log

- 2026-02-19: Finding identified by architecture-strategist review agent
