---
title: Make interrupted physical drags end in a known safe state
date: 2026-07-11
category: best-practices
module: crates/macos input
problem_type: best_practice
component: tooling
severity: high
applies_when:
  - "Implementing a multi-event physical input sequence"
  - "An event may fail after mouse-down or another irreversible input event"
  - "Reporting whether a physical action may have been delivered"
tags: [drag, cgevent, delivery, cleanup, physical-input]
---

# Make interrupted physical drags end in a known safe state

## Context

A physical drag starts before its final drop. Once mouse-down is posted, later
deadline, event, or dwell failures cannot be represented as an ordinary
pre-dispatch failure: the system may still believe the button is down.

## Guidance

`crates/macos/src/input/mouse_drag.rs` owns a `DragReleaseGuard` for the whole
sequence. It arms immediately before mouse-down is posted, so the guard is
already live when the first committed event goes out, then records delivery
once that post returns. It retains the destination release until the post
succeeds, and uses `Drop` to post a dragged and up event at the origin when
the sequence aborts. The happy path disarms only after the destination up
event has been posted.

The guard also tracks delivery. Public errors are enriched from that state, so
callers can distinguish a pre-delivery failure from an interrupted physical
operation and avoid an unsafe automatic retry.

The origin is deliberate: an abort must not become a destination drop. Cleanup
is still best effort; if the operating system cannot accept the corrective
events, the error must preserve that uncertainty rather than claiming no input
was delivered.

## Prevention

- Allocate and arm cleanup before the first committed input event.
- Do not disarm a cleanup guard before the final event post succeeds.
- Test failures before mouse-down, after mouse-down, and during final release.
- Keep sequence cleanup local to the input primitive; command code should not
  need to reconstruct partial pointer state.

## Related

- [Build desktop actions as an observe-resolve-preflight-dispatch contract](playwright-grade-desktop-reliability-2026-06-02.md)
