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
  - "Changing the shape of the FFI raw tree"
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
- bounds omission, compact collapse, and interactive-only pruning for every
  observation that allocates refs;
- named skeleton anchors that are legitimate drill-down targets.

Full snapshots use an empty root scope. Drill-down passes the root ref and its
path prefix, then updates only that root's descendants in the stored map.

### The one sanctioned second recursion

`transform_tree`, in the same file, is a second recursive walk, and it is the
one case the Prevention rule below admits: its output contract genuinely
differs, and its rustdoc says so. It applies bounds omission, compact collapse,
and interactive-only pruning to a raw adapter tree and never allocates a ref,
because the FFI `ad_get_tree` path (`crates/ffi/src/tree/get.rs`) exposes raw
trees with no ref pipeline — there is no `ref_id` for it to consult and no map
for it to write. Its pruning decision is therefore role-based where the
allocator's is ref-based.

That splits maintenance in two, and which half a change falls in is the thing to
settle first:

- **Eligibility changes in one place.** Both walks decide through the same
  predicates — `is_ref_able` / `is_ref_able_role_actions` for what is
  addressable, `is_collapsible` for what is a semantically empty wrapper.
  Change those and both paths follow.
- **Presentation semantics change in two.** The filtering walks themselves are
  separate. A new allocation dimension, or a change to when a filtered child is
  kept, has to land in `allocate_refs_at_path` *and* in `transform_tree`, or the
  raw FFI tree and the reffed tree stop agreeing on the shape of the same UI.

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
