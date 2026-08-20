---
title: Fix the class, not the reported instance
date: 2026-08-01
category: best-practices
module: crates/windows/examples/uia_tree_dump, scripts/check-no-phase-references.sh
problem_type: process_gap
component: tooling
symptoms:
  - "A review finding lists two sites; the same defect exists at two more the reviewer never reached."
  - "A widened lint catches an order of magnitude more violations than the report that motivated widening it named."
  - "Two files cross the same hard line-count cap in the same change, one of them only by trimming doc comments to fit under it."
root_cause: process_gap
resolution_type: fix_by_predicate_not_by_list
severity: medium
tags: [code-review, ci, exhaustiveness, tooling, delegation]
---

# Fix the class, not the reported instance

## Problem

Four times now — three on `feat/windows-2.3-vocabulary`, a fourth two
sub-phases later — a review named one occurrence of a defect, and fixing only
the named occurrence would have left the same defect live elsewhere in the
same tree.

**Enumeration fault-vs-exhaustion.** UI Automation signals both end-of-list
and a real cross-process fault as `Err`, distinguished only by
`is_exhaustion()`. A review of `crates/windows/examples/uia_tree_dump/render.rs`
reported the bug at two sites in `count_view()`. Commit `b708810`'s own
message states it plainly: *"Fixed at all FOUR enumeration arms, not the two
that were reported: both first-child and next-sibling in `collect()`, and
both in `count_view()`. The reviewer reached `count_view()`; `collect()` had
it too."* All four arms — `collect()`'s `first_child` match and its
`next_sibling` match, `count_view()`'s `first_child` match and its
`next_sibling` match — needed the identical `if !failure.is_exhaustion()`
guard.

**The bare plan-reference gate.** `scripts/check-no-phase-references.sh`
banned the words `phase`, `sub-phase`, `KTD<n>`, `unit U<n>` in `crates/` and
`src/`. A review reported that a bare `2.4` — the same reference with the
word filed off — escaped the gate, and named two instances. Commit `7823f2f`
widened the detection to catch bare `N.N` in doc comments (while still
passing `v0.5.0`, `pre-1.0`, `"2.1"` on the wire, and measured ratios like
`1.35x`): *"It found 14, not the 2 the review reported."* The review had
sampled the class; the class itself was seven times larger.

**The 400-line file cap.** `render.rs` and `states_tests.rs` both grew during
the same round of fixes — `render.rs` by the redaction-seam extraction work,
`states_tests.rs` by replacing the unreachable-input test with a properly
documented pair. Commit `635a537`: *"Two files reached 400 lines exactly -
`states_tests.rs` by trimming doc comments to fit, `render.rs` by growing."*
Both hit the cap in the same commit; one had already been kept under it only
by cutting documentation, which is the wrong trade the cap exists to prevent.
Both were split by responsibility instead — `render_node.rs` /
`render_node_tests.rs` out of `render.rs`, `states_walk_tests.rs` out of
`states_tests.rs`.

**Duplicated geometry and walker helpers.** A 2.6 code-review finding named
two duplicated helpers: `rect_has_area` and `nearest_scroll_viewport_bounds`.
`rect_has_area`'s copies had already drifted apart by the time anyone was
fixing them — one checked that a rectangle's dimensions were finite before
trusting them, its twin did not, so a provider answering `NaN` or an infinity
satisfied the twin's bare positive-dimension comparison — this doc's thesis
demonstrated rather than argued, not merely predicted. Enumerating the
predicate behind the second finding — an ancestor-walk primitive
reimplemented at each call site instead of owned once by the tree source —
found the class was six, not one: `same_element`, `identity`, `parent_step`,
`nearest_scroll_viewport`, `viewport_bounds`, and
`nearest_scroll_viewport_bounds`. Commit `4918c25` gave the six one home in
`crates/windows/src/tree/walker_source.rs` and gave `rect_has_area` its own
single definition in `properties.rs`, touching four files for both fixes
together.

## Root cause

A review finding is a *sample*, not a specification. "The bug is at these two
lines" is evidence of a predicate — here, "an enumeration arm treats
exhaustion and fault alike," or "a doc comment carries a bare sub-phase
number," or "this file is at the size cap" — and a predicate has however many
matches it has in the tree, independent of how many the reviewer happened to
look at. Fixing exactly the named lines closes the report, not the defect
class the report is a sample of.

## Prevention

**Treat every review finding as a sample from a class, not as the defect
itself.** Before fixing, name the predicate the finding demonstrates — not
"line 129 is wrong" but "any enumeration arm that maps `Err` to truncation
without checking `is_exhaustion()` is wrong" — then grep or otherwise
enumerate every site that matches the predicate, and fix them together in the
same change. A fix that only touches the reported lines is unverified outside
those lines.

**The corollary for delegated work:** an agent (or a person) briefed with a
list of sites fixes exactly that list. Brief it with the predicate and ask it
to find the sites itself, or the sweep silently inherits whatever the
original list missed — which is exactly how the two-vs-fourteen gap above
happened at the tooling layer instead of the review layer.

Related: [A test that cannot fail is not coverage](a-test-that-cannot-fail-is-not-coverage.md)
covers the sibling failure — a test that passed for the wrong reason rather
than a fix that was too narrow — and the same "invert and check every site"
discipline closes both.
