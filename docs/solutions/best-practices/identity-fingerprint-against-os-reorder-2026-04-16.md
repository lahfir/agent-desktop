---
title: Guard reordered notifications with required identity evidence
date: 2026-07-11
category: best-practices
module: notification commands and FFI
problem_type: best_practice
component: tooling
severity: high
applies_when:
  - "Acting on an item selected by an OS-managed list index"
  - "Designing an action API for notifications or another reorderable surface"
  - "Adding a notification mutation through CLI or FFI"
tags: [notifications, identity, reordering, fail-closed, ffi]
---

# Guard reordered notifications with required identity evidence

## Context

Notification Center can insert, remove, regroup, or reorder entries between a
list call and a mutation. An index alone is not an identity and can target a
different notification by the time it is used.

## Guidance

Notification mutation commands require at least one identity field from the
same list result: expected app or expected title. Core constructs this with
`commands::notification_identity::required_identity`; an empty identity is
`INVALID_ARGS`. The macOS adapter checks the live entry against
`NotificationIdentity` immediately before it acts and fails closed on mismatch.

FFI follows the same rule. Invalid or missing identity data is rejected instead
of being coerced into an unconstrained action. Callers recover by listing again
and selecting a fresh entry.

## Prevention

- Never expose a mutating list-index API without an identity precondition.
- Prefer a stable opaque handle when the platform supplies one; otherwise use
  the smallest reliable fingerprint and verify immediately before mutation.
- Test insertion, removal, and title/app mismatch between list and act.

## Related

- `crates/core/src/notification_identity.rs`
- `crates/macos/src/notifications/actions.rs`
