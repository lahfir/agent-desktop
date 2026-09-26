---
title: A test that acts on its own runner acts on every other test
date: 2026-08-09
category: best-practices
module: crates/windows/src/system/close_tests.rs, crates/windows/src/system/lifecycle_envelope_parity_tests.rs, crates/windows/src/tree/fixture.rs
problem_type: process_gap
component: tooling
symptoms:
  - "A test targets std::process::id() to exercise a process-scoped operation."
  - "Unrelated tests fail intermittently, in different combinations on each run, and pass in isolation."
  - "A fixture window disappears mid-test with no error from the code under test."
applies_when:
  - "Testing an operation whose scope is a pid rather than a single handle"
  - "Using the test runner as a convenient live process"
  - "Adding a test to a suite that shares process-wide OS state"
root_cause: process_gap
resolution_type: quarantine_in_a_child_process
severity: high
tags: [testing, isolation, process-scope, fixtures, parallelism, windows]
---

# A test that acts on its own runner acts on every other test

## Problem

Two lifecycle tests needed a live process to close, and reached for the most
convenient one:

```rust
let pid = ProcessId::from(std::process::id());
let app = app_for_pid("test-runner", pid);
let error = close_app_impl(&app, false, short).expect_err("timeout");
```

The property under test was sound — a graceful close whose target does not
exit within the deadline must report `TIMEOUT` with `delivered_unverified` —
and the assertion that the process is still alive afterwards is a real one.

The problem is what "graceful close" means. It is deliberately **pid-scoped**:
it enumerates every top-level window the target process owns and posts
`WM_CLOSE` to each, because the window that owns an application's shutdown may
be any of them. Pointed at the test runner, it posts `WM_CLOSE` to every window
the *runner* owns — which includes every in-process fixture window created by
every other test running in parallel. Those fixtures' window procedure falls
through to `DefWindowProcW`, which destroys them.

So a test that never touches another test's data destroys another test's
*fixtures*, mid-test, nondeterministically. The victim fails somewhere
unrelated, with no indication of the cause, and passes when re-run alone.

## Root cause

The runner looked like an ideal target: guaranteed alive, guaranteed to have a
readable identity, guaranteed not to exit during the test — exactly the
properties the test needed. What it also has, and what the test did not
account for, is **everything else the suite owns**.

The mismatch is between the scope of the assertion and the scope of the
operation. The assertion is about one process's exit behaviour; the operation
is about every window that process owns. When those scopes differ, choosing the
runner silently widens the blast radius to the whole suite, and the widening is
invisible in the test body — nothing in `close_app_impl(&app, ...)` mentions
windows at all.

Parallelism converts that into a heisenbug rather than a hard failure, so it
survives review: the test passes, the suite passes most of the time, and the
occasional red is attributed to flakiness.

## Solution

Give the test a process whose entire window set belongs to the test. The suite
already had the shape — a child-process fixture:

```rust
let fixture = HostedFixture::spawn_swallowing_wm_close()?;
let pid = ProcessId::from(fixture.process_id());
```

A child process quarantines the pid-scoped operation: the fan-out reaches only
that process's windows, and the surrounding suite is untouched. The
`swallow_wm_close` variant additionally makes the target *ignore* the request,
which is what forces the timeout the test is asserting — so the quarantine
strengthens the test rather than weakening it.

## Prevention

- Before targeting `std::process::id()`, ask what the operation's scope is. If
  it is anything wider than the one object under test — a pid, a window class,
  a session, the foreground — the runner is the wrong target.
- Prefer a child process for any operation that is process-scoped. It is the
  only target whose full inventory the test controls.
- When a suite shares OS state (windows, foreground, cwd, clipboard), treat a
  new test as a potential source of interference, not only a potential victim.
  The question is not "can this test be disturbed" but "what does this test
  disturb".
- Intermittent failures in *unrelated* tests are a signal to look for a test
  that mutates shared state, not a signal to retry. A test that passes alone
  and fails in the suite is describing its neighbours.
- The same reasoning applies to any process-wide handle the runner owns: the
  current directory, environment variables, and the clipboard have the same
  shape as the window list.

## Related

- [A test that cannot fail is not coverage](a-test-that-cannot-fail-is-not-coverage.md)
- [A deadline cannot interrupt a blocking OS call](../logic-errors/a-deadline-cannot-interrupt-a-blocking-os-call.md)
- [A cited measurement must match its capture](a-cited-measurement-must-match-its-capture.md)
