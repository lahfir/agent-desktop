---
title: A fixed sleep is not a synchronization primitive
date: 2026-08-16
category: best-practices
module: crates/core/src/interaction_lease_tests.rs
problem_type: flaky_test
component: test-infrastructure
symptoms:
  - "A test passes locally and on most CI runs, then fails on a loaded runner with an assertion that was true when it was written."
  - "A helper process holds a resource for a fixed duration and the assertion racing it usually wins."
  - "Re-running the job makes it green, so the failure is filed as infrastructure noise rather than a defect."
---

## Problem

A cross-process lock test spawned a helper that acquired the lease, wrote a
readiness marker, and held it by sleeping:

```rust
let _lease = acquire_unix_interaction_lease_at(deadline, &root).unwrap();
std::fs::write(ready_path, b"ready").unwrap();
std::thread::sleep(Duration::from_secs(5));
```

The parent waited for the marker, then asserted that a second acquisition is
refused because the child still holds the lock:

```rust
let ready_deadline = std::time::Instant::now() + Duration::from_secs(2);
while !ready.is_file() && std::time::Instant::now() < ready_deadline {
    std::thread::sleep(Duration::from_millis(10));
}
let error = acquire_unix_interaction_lease_at(Deadline::after(25).unwrap(), &root)
    .err()
    .expect("adopted descriptor must retain the lease");
```

It failed on a two-core CI runner: the acquisition **succeeded**, so `.err()`
returned `None` and `.expect` panicked.

## Root cause

**The 2-second poll deadline bounds the loop, not the gap the assertion depends
on.** The quantity that has to stay under 5 seconds is the span from the child
writing the marker to the parent attempting its acquisition — and nothing
measures that. The loop checks the clock *between* sleeps, so a parent thread
descheduled for six seconds inside a single `sleep(10ms)` wakes up, sees the
marker present, exits the loop, and proceeds. By then the child's hold has
expired and the lock genuinely is free. The assertion is not wrong; the
precondition it was written against no longer holds.

Starvation of that size is ordinary here rather than exotic. The suite runs
~1,000 tests across as many threads as cores, and this test's child spawns a
*further* full test binary and blocks on it before signalling ready — three
processes competing for two cores.

Lengthening the sleep does not fix this. It moves the cliff and makes the
remaining failures rarer, later, and harder to attribute.

## Solution

Replace the timer with a handshake, so the hold lasts exactly as long as the
parent needs it and no assumption about scheduling survives.

Give the child a piped stdin and have it block on end-of-file:

```rust
fn hold_until_parent_releases() {
    use std::io::Read;

    let mut released = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut released);
}
```

```rust
let mut child = std::process::Command::new(std::env::current_exe().unwrap())
    // ...
    .stdin(std::process::Stdio::piped())
    .spawn()
    .unwrap();
```

The child now holds until every write end of that pipe closes. The parent holds
the only one, inside `Child`, so the hold ends when the parent kills the child,
drops the `Child`, or exits — including when it exits by panicking. There is no
duration anywhere, so there is no span to out-race, and the failure mode where
a test leaks a process holding a lock forever is closed by the same mechanism
rather than by a second timer.

**Verify the primitive in both directions before trusting it.** The fix rests
on one claim — that a child blocked in `read_to_end` unblocks when the parent
drops its `ChildStdin` — and a fix built on an unverified claim is the original
defect with more steps. A standalone check proved both halves: the child stayed
blocked for a full 500 ms while the parent held the pipe (so the hold is real,
not a no-op that would make the test vacuous), and released 21 ms after the
parent dropped it. The negative control is the half that matters; without it, a
`hold` that returned immediately would look exactly like success.

## Prevention

- **Ask what the sleep is standing in for.** A `sleep` in a test is almost always
  a placeholder for "wait until X happens". If X is observable — a file, a pipe
  closing, a process exiting, a status flipping — wait on X. A duration is the
  right answer only when the thing being tested *is* a duration.
- **A wall-clock budget on a thread that can be descheduled is a latent flake.**
  This applies to every fixed budget in a test, not just the one that failed:
  the same file's reap-after-kill budget and its "the contended call returned
  promptly" bound were the same shape and were widened in the same change. Fix
  the class, not the reported instance.
- **Bound what the assertion depends on, not what is convenient to bound.** The
  broken loop measured "how long until the marker appears". The assertion needed
  "how long from the marker appearing to the acquisition attempt". Whenever those
  two are different quantities, a passing test is a coincidence.
- **A rerun that goes green is evidence of a race, not evidence of noise.** The
  cheapest moment to fix a flake is the first time it is seen, while the failing
  log still exists and the reason is still legible.
- **Separate "the resource is held" from "the test is finished with it".** Tying
  them to one timer couples two unrelated lifetimes; a handshake gives each its
  own signal and makes the crash-releases property fall out for free.
