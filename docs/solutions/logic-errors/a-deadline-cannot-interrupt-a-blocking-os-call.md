---
title: A deadline cannot interrupt a blocking OS call
date: 2026-08-09
category: logic-errors
module: crates/windows/src/system/window_op.rs, crates/windows/src/system/window_activate.rs, crates/windows/src/system/window_identity.rs
problem_type: logic_error
component: platform-adapter
symptoms:
  - "A command with a bounded Deadline never returns."
  - "A retry budget is finite but wall-clock time is not, because one attempt blocks."
  - "A test against a non-pumping fixture window hangs instead of failing."
applies_when:
  - "Calling a Win32 API that delivers to another thread's message queue"
  - "Guarding a native call with a Deadline or a retry budget"
  - "Reading a window property that is implemented as a sent message"
root_cause: logic_error
resolution_type: probe_liveness_before_the_call
severity: high
tags: [win32, deadline, blocking, message-queue, hang, windows]
---

# A deadline cannot interrupt a blocking OS call

## Problem

`window_op` and `focus_window` were deadline-bounded in the ordinary way: the
budget was checked at entry and again between retry attempts. Against a window
whose owning thread had stopped dispatching messages, both hung — measured as a
call that had not returned after ten minutes.

The budget was never wrong; it was never consulted. `ShowWindow`,
`SetWindowPos` and `SetForegroundWindow` deliver to the owning thread's message
queue, and when that thread never dispatches, the call blocks *inside the OS*.
A `Deadline` checked before the call cannot fire during it, and a retry budget
counted between attempts cannot bound an attempt that never ends:

```rust
for attempt in 0..BUDGET {
    ensure_budget(deadline)?;       // checked here
    bring_to_foreground(handle)?;   // blocks forever in here
}
```

This is the same shape the crate already had on record for
`ElementFromHandle` (A14-11), where the mitigation was a
`SendMessageTimeoutW(WM_NULL, SMTO_ABORTIFHUNG)` pre-probe. The lesson had been
learned for the observation path and not carried to the write path.

## Root cause

Two assumptions, each reasonable alone.

The first is that a deadline bounds a *region* of code. It does not — it bounds
the points where it is checked. Every call between two checks is unbounded, and
a call that blocks indefinitely makes the surrounding budget decorative.

The second is that a hang is a property of the *caller's* thread. Here it is a
property of the *target's* thread, which the caller does not control and cannot
inspect from the return value: the call has no timeout parameter and no failure
mode for "the other side is not listening". The only way to find out is to ask
first, with a call that does have a timeout.

A subtler instance sat in the same path and was easy to miss. Identity
verification read the window's live title, and `GetWindowTextW` **sends**
`WM_GETTEXT` when the window belongs to the calling process — so verification
blocked before any write was reached. Cross-process it does not send, and
returns the caption directly, which is why the hazard only appears in-process
and is invisible in ordinary use.

## Solution

Ask a bounded question before making an unbounded call, and reuse the crate's
existing probe rather than inventing a second one:

```rust
fn ensure_window_is_pumping(handle: WindowHandle) -> Result<(), AdapterError> {
    if window_is_responsive(handle) {
        return Ok(());
    }
    Err(AdapterError::new(ErrorCode::AppUnresponsive, "...not processing messages..."))
}
```

Two placement details decide whether the guard is correct:

- **After identity verification, not before.** Probing first makes a destroyed
  or re-owned handle report `APP_UNRESPONSIVE` — a worse, misleading envelope
  than the stale-identity error it earned. The guard belongs between
  verification and the native writes.
- **Bound the reads inside verification too.** Once the guard sits after
  verification, verification itself must not be able to block, so the live
  title read takes its own short probe and treats an unanswered window as
  having no readable title. Identity still rests on the owner and generation
  checks, which never touch the message queue.

## Prevention

- For any Win32 call that reaches another thread's message queue — `ShowWindow`,
  `SetWindowPos`, `SetForegroundWindow`, `SendMessage`, and the property reads
  implemented on top of them — assume it can block and guard it. The API list
  is short enough to check against.
- Treat "the deadline covers it" as a claim to verify, not a default. Ask which
  single call between two checks could take forever; if one can, the deadline
  does not cover it.
- A retry budget bounds attempts, not time. State which one you are bounding.
- **A test that hangs is a finding, not a flake.** The hang against a
  non-pumping fixture is the defect reproducing, and it is the cheapest proof
  the guard works: remove the guard and the test stops returning.
- When a mitigation is adopted for one call site, grep for the other calls with
  the same delivery mechanism. A14-11 fixed the resolver; the writes kept the
  bug for four sub-phases.

## Related

- [A zero success value is not the answer you asked for](a-zero-success-value-is-not-the-answer-you-asked-for.md)
- [A test that cannot fail is not coverage](../best-practices/a-test-that-cannot-fail-is-not-coverage.md)
- [Abort-state guidance for multi-step physical input](../best-practices/abort-state-guidance-multi-step-physical-input.md)
