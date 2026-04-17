---
title: Guard OS-reordered resources with an identity fingerprint, not a raw index
date: 2026-04-16
category: best-practices
module: crates/core, crates/macos, crates/ffi
problem_type: best_practice
component: notifications
severity: high
applies_when:
  - The API exposes a numeric index (or handle) obtained from a list-then-act flow
  - The underlying OS can reorder, add, or remove entries between the list and act calls
  - Acting on the wrong entry has user-visible consequences (Reply to the wrong sender, Dismiss the wrong notification, Press the wrong button)
  - Existing design assumed lists are stable across calls and did not re-verify
tags:
  - notification-center
  - confused-deputy
  - fingerprint
  - identity
  - ffi
  - reordering
  - fail-closed
---

## Problem

macOS Notification Center reassigns the `index` of every visible
notification on each listing. Between `list_notifications()` and
`notification_action(index, ...)`:

- a new notification arrives → everything shifts down by 1
- an unrelated notification is dismissed → everything shifts up by 1
- the user opens the Notification Center sidebar → grouping changes
  can renumber entries entirely

Any tool that round-trips `(app, title) → index → "press Reply"` is a
**confused deputy** at the OS boundary: the tool thinks it's acting on
the notification it showed to the user, but by the time the action
call reaches NC the slot points elsewhere.

This class of bug exists for any OS API that:

- returns an ordered list whose ordering is defined by "current state",
  not by a stable identity
- accepts an index (or some other positional handle) as a subsequent
  parameter

Examples beyond NC: running-process lists by PID reuse, window lists
after `raise()`, clipboard history, filesystem listings used for bulk
operations.

## Solution

Pass an optional identity fingerprint alongside the index. Verify the
row at that index against the fingerprint **before** acting. Fail
closed if it doesn't match.

```rust
pub struct NotificationIdentity {
    pub expected_app: Option<String>,
    pub expected_title: Option<String>,
}

impl NotificationIdentity {
    pub fn matches(&self, info: &NotificationInfo) -> bool {
        if let Some(ref app) = self.expected_app {
            if app != &info.app_name { return false; }
        }
        if let Some(ref title) = self.expected_title {
            if title != &info.title { return false; }
        }
        true
    }
}
```

Adapter layer:

```rust
let entry = list_entries(&filter)?.into_iter()
    .find(|e| e.info.index == index)
    .ok_or_else(|| AdapterError::notification_not_found(index))?;

if let Some(id) = identity {
    if !id.is_empty() && !id.matches(&entry.info) {
        return Err(AdapterError::new(
            ErrorCode::NotificationNotFound,
            "row at this index does not match the expected fingerprint — NC likely reordered",
        ));
    }
}

// safe to press
```

## Design choices

**Optional fields, not mandatory.** The fingerprint is a safety feature,
not a required parameter. Hosts that already reconcile (e.g. by
re-listing just before acting) can leave both fields null and get
the legacy behavior. This keeps the API ergonomic for simple scripts
while making the safe path available.

**Re-use an existing error code** (`NotificationNotFound`) rather than
adding a new one. From the host's perspective, "the notification I
intended to act on is gone or moved" is semantically the same as "the
notification at that index disappeared": in both cases the right
recovery is re-list and retry. Introducing a distinct
`IDENTITY_MISMATCH` code would fork callers' error handling without a
real behavioral difference.

**Tri-state UTF-8 decoding at the FFI boundary.** The identity strings
come in as `*const c_char`. Null means "no fingerprint"; invalid UTF-8
must NOT be silently coerced to "no fingerprint" (that would defeat
the guard). Use `try_c_to_string` which returns
`Ok(None)` / `Ok(Some(_))` / `Err(())` and map `Err` to
`InvalidArgs`.

## When NOT to use this

- If the OS API returns a stable opaque handle that the caller can
  hold across calls (e.g. `HWND` on Windows), plumb the handle
  through instead of an index. Fingerprinting is a fallback for
  APIs whose handles we can't or shouldn't persist.
- If the operation is idempotent and harmless (e.g. "list children
  of this group"), mismatch handling adds cost without value.

## References

- `crates/core/src/notification.rs` — `NotificationIdentity` type and
  tests
- `crates/macos/src/notifications/actions.rs` — adapter-layer check
- `crates/ffi/src/notifications/action.rs` — C-ABI surface with
  optional app/title pointers
- Todo `003-ready-p1-stable-notification-action-identity` — the bug
  this pattern resolves
