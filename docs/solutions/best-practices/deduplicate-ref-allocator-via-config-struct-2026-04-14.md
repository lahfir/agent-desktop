---
title: Keep ref allocation in one recursive owner
date: 2026-07-11
category: best-practices
module: crates/core ref allocation
problem_type: best_practice
component: tooling
severity: high
applies_when:
  - "Changing which nodes receive refs"
  - "Changing compact, bounds, skeleton, or drill-down allocation behavior"
  - "Adding source or scope evidence to RefEntry"
tags: [refs, snapshot, drill-down, recursion, config-struct]
---

# Keep ref allocation in one recursive owner

## Context

Full snapshots and rooted drill-downs both turn an observed tree into refs.
They differ only in their allocation options, source evidence, and scope. Two
recursive implementations would inevitably make the same UI allocate different
refs depending on how it was observed.

## Guidance

`allocate_refs` in `crates/core/src/ref_alloc.rs` is the only recursive allocator.
Callers supply a `RefAllocConfig` composed of `RefAllocOptions`,
`RefAllocSource`, and `RefAllocScope` rather than adding another allocator or a
positional argument list.

The allocator owns these semantics together:

- ref eligibility: interactive roles or primary advertised actions, never a
  bare focus affordance;
- identity, geometry, capability, source, and scope evidence in `RefEntry`;
- bounds omission, compact collapse, and interactive-only pruning;
- named skeleton anchors that are legitimate drill-down targets.

Full snapshots use an empty root scope. Drill-down passes the root ref and its
path prefix, then updates only that root's descendants in the stored map.

## Why This Matters

Ref allocation is an identity boundary, not presentation formatting. A change
to one path but not the other can create stale refs, lose scope evidence, or
make an action available in one observation mode but impossible in another.

## Prevention

- Add behavior once in `ref_alloc.rs` and cover both full and rooted snapshots.
- Prefer a nested config type when a new allocation dimension appears; do not
  add `_with_root` or `_for_skeleton` copies.
- Treat a new recursive allocator as an architecture violation unless its
  output contract is genuinely different and documented.

## Related

- [Keep progressive snapshots namespace-scoped and ref-safe](../logic-errors/progressive-snapshot-review-contract-2026-04-16.md)
- [Playwright-grade desktop reliability contract](playwright-grade-desktop-reliability-2026-06-02.md)
