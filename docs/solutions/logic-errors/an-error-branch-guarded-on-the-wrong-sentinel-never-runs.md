---
title: An error branch guarded on the wrong sentinel never runs
date: 2026-08-16
category: logic-errors
module: crates/windows/src/system/thread_walk.rs, crates/windows/src/system/app_ops.rs, crates/windows/src/system/process_identity.rs
problem_type: silent_failure
component: win32-ffi
symptoms:
  - "A native call's failure path is written, reviewed, and unreachable: the guard tests a sentinel the API never returns."
  - "A failed enumeration returns an empty result instead of an error, so a native failure is reported as a confident negative answer."
  - "The wrong guard looks idiomatic because a neighbouring call in the same file genuinely does use that sentinel."
---

## Problem

`CreateToolhelp32Snapshot` reports failure by returning `INVALID_HANDLE_VALUE`.
Four call sites across three files guarded it with `is_null()`:

```rust
let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
if snapshot.is_null() {
    return Err(open_failure_error());
}
```

`INVALID_HANDLE_VALUE` is `-1i32 as HANDLE`; `is_null()` tests for `0`. The two
bit patterns are unrelated, so the comparison is never true when the call
actually fails and `open_failure_error()` is dead code on the only path that
can reach it.

Nothing crashes, which is what makes it expensive. Execution falls through with
an invalid handle: `Thread32First` fails, the `while ok != 0` loop body never
runs, the accumulator stays empty, `CloseHandle(INVALID_HANDLE_VALUE)` is a
harmless no-op, and the function returns `Ok(empty)` — byte-identical to
"enumerated everything, found nothing."

For the menu predicate that mattered concretely. Classic menu-mode detection is
the only source covering Win32 menu-bar dropdowns; the UIA source deliberately
does not. A snapshot failure therefore made `wait --menu` poll to its full
timeout against an application whose menu was open the entire time, and report
`No menu opened before the deadline` — a masked native failure delivered to the
caller as a confident negative.

## Root cause

**Win32 does not have one failure sentinel, and the two in play look
interchangeable at the call site.** Handle-returning functions split:

| function | failure value |
|---|---|
| `CreateToolhelp32Snapshot`, `CreateFileW` | `INVALID_HANDLE_VALUE` (`-1`) |
| `OpenProcess`, `CreateMutexW`, `FindWindowW` | `NULL` (`0`) |

Both are typed `HANDLE`, so the compiler accepts either guard against either
function. Nothing in the type system distinguishes them.

The specific trap here: `process_identity.rs` calls **both** APIs, and its
`OpenProcess` guard is correctly `is_null()` a few lines from a
`CreateToolhelp32Snapshot` guard that is wrong. The incorrect guard reads as the
file's established convention, so pattern-matching against the surrounding code
actively confirms the error.

It survived a simplification pass, a three-axis adversarial dogfood and a
nine-lens code review before one lens caught it. It also survived a
consolidation: refactoring the duplicated walk into one shared helper carried
the defect into the new home unchanged, because consolidation preserves
behaviour — including behaviour that is wrong.

## Solution

Compare against the sentinel the API documents:

```rust
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;

let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
if snapshot == INVALID_HANDLE_VALUE {
    return Err(open_failure_error());
}
```

Fix the class rather than the reported instance. The review named one site; a
grep for every `CreateToolhelp32Snapshot` in the crate found four, two of them
in files the sub-phase under review had never touched. A convention error
spreads by being copied, so the population is every call site, not the one
someone happened to read.

Pin the distinction with a test. This defect has no runtime signature to assert
against — the failure is unreachable by construction — so what is testable is
that the two sentinels are not the same value:

```rust
assert!(!INVALID_HANDLE_VALUE.is_null());
assert_eq!(INVALID_HANDLE_VALUE as isize, -1);
```

That reads as tautology and is not: it fails the moment someone reintroduces the
`is_null()` guard on the reasoning that "the handle type is a pointer, so null
is the failure."

## Prevention

- **When a native call's failure branch is written but has never been observed
  to run, treat it as unverified.** An error path nothing has executed is a
  hypothesis about an API contract, not a handled case.
- **Check the sentinel against the API's own documentation, not against the
  neighbouring call.** Adjacent Win32 calls in one file routinely disagree on
  their failure value, and imitating the neighbour is how the wrong guard gets
  written by someone being careful.
- **`Ok(empty)` and `Err(...)` are not interchangeable outcomes for an
  enumeration.** Ask what a caller does with an empty result. If "found nothing"
  and "could not look" lead to different decisions — and for a predicate that
  gates a wait, they always do — the failure must not be able to collapse into
  the empty case.
- **Consolidation preserves defects.** Merging duplicated code into one helper
  is worth doing, and it is not a review of the code being merged. Read the
  behaviour being unified rather than assuming that two copies agreeing means
  either one is right — here they agreed and were both wrong.
- **A finding of this class is a reason to grep the class.** `fix-the-class-not-
  the-reported-instance.md` records the general rule; this is what it looks like
  for an FFI convention, where the population is every call site of the same API
  in the tree.
