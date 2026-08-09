---
title: Guard OS-reordered resources with an identity fingerprint
date: 2026-07-11
category: best-practices
module: notification commands and FFI, crates/windows/src/system/window_activate.rs
problem_type: best_practice
component: tooling
severity: high
applies_when:
  - "Acting on an item selected by an OS-managed list index"
  - "Designing an action API for notifications or another reorderable surface"
  - "Adding a notification mutation through CLI or FFI"
  - "Restoring, foregrounding, or otherwise mutating a resource addressed by a recyclable OS handle (HWND, PID, or similar) when the check and the write cannot be issued as one atomic operation"
tags: [notifications, identity, reordering, fail-closed, ffi, windows, check-then-act]
---

# Guard OS-reordered resources with an identity fingerprint

## Context

An OS-managed resource can be inserted, removed, recycled, or reassigned
between the moment a caller observes it and the moment an action mutates it.
Notification Center can insert, remove, regroup, or reorder entries between a
list call and a mutation. Windows can destroy a window and hand its `HWND` to
an unrelated process before the next call that uses it. An index or a handle
alone is not an identity: either can end up addressing a different resource
by the time it is used, and wherever the check and the act cannot be issued
as one atomic operation, that gap is a live window for exactly this.

## Guidance

**Notification identity.** Notification mutation commands require at least
one identity field from the same list result: expected app or expected
title. Core constructs this with
`commands::notification_identity::required_identity`; an empty identity is
`INVALID_ARGS`. The macOS adapter checks the live entry against
`NotificationIdentity` immediately before it acts and fails closed on
mismatch. FFI follows the same rule: invalid or missing identity data is
rejected instead of being coerced into an unconstrained action, and callers
recover by listing again and selecting a fresh entry.

**Window-handle identity.** A Windows `HWND` is a recyclable handle: once a
window is destroyed, the OS is free to hand the same numeric value to an
unrelated process's next window. Verifying identity once, at entry, is not
enough when the write that follows is not atomic with that check — a handle
can be destroyed and recycled in the gap between the check and the native
write. `focus_window` (`crates/windows/src/system/window_activate.rs`)
re-reads the owning pid immediately before every native write that follows
entry verification: `restore_if_iconic` re-checks ownership before its
`ShowWindow` call, and `bring_to_foreground` re-checks before `ShowWindow` /
`SetForegroundWindow`. A recycle caught by either of those internal checks
fails closed as `WINDOW_NOT_FOUND` / not-delivered, never a foreground write
to a foreign window.

Re-verifying the precondition before each write is necessary but not
sufficient on its own: the success check has to be identity-qualified too,
or a recycle that wins the race between the last write and the result check
reports success over the wrong window. `is_owned_foreground` requires both
`is_foreground_window(handle)` **and** `live_window_owner(handle) ==
Some(expected)` — handle equality alone would accept a recycled HWND that
happens to be foreground for an unrelated reason. See `focus_window`'s own
doc comment in `window_activate.rs` for the stated rationale.

## Prevention

- Never expose a mutating list-index or bare-handle API without an identity
  precondition.
- Prefer a stable opaque handle when the platform supplies one; otherwise use
  the smallest reliable fingerprint and verify immediately before mutation.
- When the check and the act cannot be made atomic, a precondition verified
  once at entry is not enough: re-verify identity immediately before *every*
  native write the action performs, not only the first.
- Fold identity into the success predicate itself, not only the
  precondition. A success check built from handle equality alone
  (`GetForegroundWindow() == handle`) cannot tell a genuine success from a
  recycled handle that happens to satisfy it; the check must also confirm
  the resource is still the one the action targeted, not merely that
  something answered at the expected address.
- Test insertion, removal, and title/app mismatch between list and act; for
  a recyclable handle, test the destroy-and-recycle race directly against
  both the precondition and the success predicate.

## Related

- `crates/core/src/notification_identity.rs`
- `crates/macos/src/notifications/actions.rs`
- `crates/windows/src/system/window_activate.rs`
