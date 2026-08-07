---
title: A test that cannot fail is not coverage
date: 2026-08-01
category: best-practices
module: crates/core/src/accname.rs, crates/windows/src/tree, crates/windows/examples/uia_tree_dump
problem_type: process_gap
component: tooling
symptoms:
  - "A regression test asserts two entry points 'agree', where one entry point is defined as calling the other."
  - "A states producer has a passing unit test for a role/property combination the role mapper can never produce together."
  - "A security-redaction test lives under crates/windows/examples/ and the CI lane runs cargo test -p agent-desktop-windows --lib."
  - "A census tool's redaction guard still passes when a call site is reverted to render text verbatim, because the guard tests the helper, not the call site."
  - "A second hand-maintained list (VALUE_BEARING) can miss an entry the first list (WALK_SET) gained, and nothing fails."
  - "A #[cfg(test)] helper re-types the same boolean expression a production function inlines, so inverting production leaves the mirrored test green."
root_cause: process_gap
resolution_type: label_relocate_extract_seam_or_exhaustive_match
severity: high
tags: [testing, tautology, ci, exhaustiveness, com-free-seam, windows, coverage]
---

# A test that cannot fail is not coverage

## Problem

The Windows vocabulary work shipped, and two review passes plus the
implementer's own pass then found five tests that were green for reasons
unrelated to what they claimed to prove:

1. **Tautology.** `accname_tests.rs`'s
   `the_uncertainty_aware_and_plain_entry_points_agree_on_every_input` asserts
   `resolve_name(&value, &NameSlotStatus::default()).known().cloned() ==
   compute_name(&value)`. `compute_name` (`accname.rs:84-88`) is defined as
   exactly that call. The assertion is `x == x` wearing an agreement claim.
2. **Impossible input.** `states_tests.rs` fed the states producer
   `role == "button"` with `ToggleAvailable` true. `roles.rs`'s `button_role`
   reclassifies any toggle-available `Button` to `Role::Switch` before states
   resolve, so a `button` role reaching the producer has never advertised
   `ToggleAvailable` — the arm under test cannot fire in production.
3. **Gate the runner never executes.** The census redaction guard — proving
   `Name`, `HelpText`, `FullDescription`, `LegacyDefaultAction` never
   serialize into a committed capture — lived in
   `examples/uia_tree_dump/render_node_tests.rs`. The Windows CI lane ran
   `cargo test -p agent-desktop-windows --lib`, which never builds `examples/`.
4. **Guard tested one level from the thing it guards.** Even fixed for #3, the
   redaction *rule* was pinned only against `text_presence`/`field_presence`
   themselves in `render_slots.rs`. The call site deciding which renderer each
   property uses — `node()` in `render.rs` — was reachable only through a live
   `UIAElement`. Reverting one call site from `text_presence(...)` to the
   verbatim `slot(...)` passed `--lib`, `--examples`, and the new CI step.
5. **Hand-maintained parallel list.** `is_value_bearing()` checked membership
   in a `VALUE_BEARING` array. Nothing forced that array to stay in step with
   which properties actually carry target text elsewhere in the walk — a text
   property added to `WALK_SET` without a matching `VALUE_BEARING` entry would
   leak silently.
6. **Test-only mirror.** `crates/windows/src/tree/hit_test.rs`'s pre-probe
   guard ladder was inlined directly in production, and its `#[cfg(test)]`
   coverage (`guard_zero_area`, `guard_point_outside_bounds`,
   `guard_outside_virtual_screen`) re-typed the same boolean expressions
   rather than calling them. The tests drove their own mirror, so inverting
   the production guard left every one of them green.

## Root cause

Every one of these was written from the implementation as it already stood,
not from the failure the test exists to catch.

Shape 1 restates a delegation as an assertion — true by construction,
unchanged by any bug the delegation might reintroduce. Shape 2 was written
against the states producer in isolation, without checking what the role
mapper actually hands it, and never noticed the mapper had closed the gap
first. Shapes 3 and 4 are the same defect at different distances: a test the
runner never builds, and a test that reaches the runner but not the real call
site, both fail to intersect the code path they claim to cover — see
[Never ship platform code that CI cannot execute](never-ship-platform-code-that-ci-cannot-execute.md)
for the lane-flags half of this and
[A verification gate is code and needs its own test](a-verification-gate-is-code-and-needs-its-own-test.md)
for the gate that already names shape 3. Shape 5 is a duplicate-source-of-truth
bug: a test that checks one hand-kept list against another proves the copies
match each other, never that either is complete. Shape 6 is adjacent to shape
1's delegation trick but sharper: the test did not call the production
expression at all, it re-derived it independently, so production and test
could silently diverge without either side ever calling the other — inverting
production proved nothing, because the test's own mirror never moved.

## Solution

None of the fixes were "delete the test":

1. **Label it, don't pretend.** The delegation guard is worth keeping as a
   structural, advisory guard against re-divergence — but its doc comment
   should say that, distinct from the genuinely falsifiable precedence tests
   beside it.
2. **Cover the reachable shape, and keep the unreachable one as a named
   regression test.** `pressed_is_unproduced_for_a_button_role_at_any_toggle_state`
   stays, with a doc comment stating its input is unreachable and pointing at
   `toggle_state_on_a_switch_role_emits_checked_and_indeterminate`, which opens
   by asserting `is_toggleable_role("switch")` so the reachable case is pinned
   first.
3. **Add a lane that builds the target.** `.github/workflows/ci.yml` gained a
   `Windows example tests` step (`cargo test --locked -p agent-desktop-windows
   --examples`), pinned as a literal string in `src/cli/contract_tests.rs` so
   the workflow can't quietly narrow back to `--lib`.
4. **Extract a COM-free seam and test the seam.** `render_node()` moved into
   its own file taking a plain `NodeFields` struct instead of a live
   `UIAElement`. `render_node_tests.rs` plants a unique marker in every
   text-bearing field and asserts it appears nowhere in the rendered JSON
   while `present`/`chars` still report — it now drives the exact function
   deciding what reaches a committed capture, and fails against real
   call-site reverts.
5. **Replace the parallel list with a match the compiler enforces.**
   `TreeProperty::carries_target_text()` is an exhaustive match with no
   catch-all — adding a variant without extending it is a compile error, not a
   silent `false`. `property_ids_tests.rs` asserts it implies
   `is_value_bearing()` across the whole enum, not only `WALK_SET`.
6. **Extract one pure function and let both sides drive the same copy.**
   `pre_probe_decision` (`crates/windows/src/tree/hit_test_classify.rs:59-78`)
   is now the only place the guard ladder is expressed. Production calls it
   from `pre_probe_guard` (`hit_test.rs:195-207`); `hit_test_guard_tests.rs`
   calls it directly. Production and test read the identical logic instead of
   two independently maintained copies of it.

## Prevention

The one check that catches all six cheaply is **invert the thing under test
and confirm the test goes red**: revert the delegation to two independent
implementations, feed the producer an input the role mapper would never
produce, revert a `text_presence` call site to `slot()`, drop an enum variant
from the exhaustive match, trip one arm of the pre-probe guard ladder and
check that only that arm's own test fails. If nothing turns red, the test is
not coverage.

Shape 6 shows that inversion alone can still lie. The first extraction of the
guard ladder returned a bare `Option<HitTestResult>`, and every guard trip
produced the identical `Some(Unknown)`. Inverting the zero-area arm under that
shape still turned a test red — but for the wrong reason: the geometry fell
through to a *different* guard, which answered `Some(Unknown)` in its place,
so the arm supposedly under test was never the one actually exercised.
Falsifiability needed a result each arm could be blamed for individually:
`PreProbeGuard` (`hit_test_classify.rs:40-45`) is four named variants —
`ZeroArea`, `IconicRoot`, `OutsideVirtualScreen`, `OutsideTargetBounds` — and
`hit_test_guard_tests.rs` asserts each one by name, so an arm silently
absorbed by a sibling now produces the wrong variant instead of the same
`Unknown` its sibling would have produced too.

- "Invert it and confirm the test goes red" is necessary but not sufficient
  when several arms collapse to the same output value: check that it goes red
  because *that* arm's own assertion failed, not because a sibling arm caught
  the case in its place. Give each arm a distinguishable result, or the
  inversion is absorbed by a sibling.
- Before trusting a "these two agree" test, check whether one side is defined
  in terms of the other.
- Before trusting a states/role test, check the role mapper can actually
  produce the combination under test — see
  [Use explicit arms for string-keyed policy mirrors](exhaustiveness-guards-over-catch-alls-in-policy-mirrors.md)
  for the same discipline on the mapping side.
- A test that cannot reach the production call site is the same defect as one
  the runner never executes — check both the CI lane's flags and whether the
  test drives the function under review, not a neighbor of it.
- Prefer an exhaustive match with no catch-all over a second hand-maintained
  list whenever something must be classified; the compiler is a test that
  never goes stale.
