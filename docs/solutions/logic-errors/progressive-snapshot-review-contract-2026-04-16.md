---
title: Keep progressive snapshots namespace-scoped and ref-safe
date: 2026-07-11
category: logic-errors
module: crates/core snapshot and refs
problem_type: logic_error
component: tooling
symptoms:
  - "A drill-down mutates refs outside its requested subtree or reports a window unrelated to its root ref."
  - "Malformed or cross-namespace refs are classified as stale instead of invalid input."
  - "Skeleton output hides truncation or creates refs that cannot be resolved safely."
root_cause: logic_error
resolution_type: code_fix
severity: high
tags: [snapshot, drill-down, refs, sessions, skeleton, contracts]
---

# Keep progressive snapshots namespace-scoped and ref-safe

## Problem

Progressive observation has two different operations: a full window snapshot
creates a new ref map, while `snapshot --root` replaces only the selected
subtree in an existing snapshot. Treating either as an unconstrained merge
makes old refs appear valid after their evidence has changed.

## Solution

- Resolve `--root` with `ref_token::resolve_ref_target` before loading a map.
  A qualified ref supplies its snapshot; a bare ref requires the explicit
  snapshot argument. Both are looked up only through
  `RefStore::for_session(context.session_id())`.
- Re-resolve the saved root entry strictly before observing its subtree. A
  missing, stale, or ambiguous live root fails closed; it is never replaced by
  a positional guess.
- Use `RefStore::update_existing_snapshot` to remove only descendants owned by
  that root, then allocate replacements through the shared
  `ref_alloc::allocate_refs` owner. `RefAllocScope` preserves the root and
  absolute path evidence for the replacement entries.
- A normal snapshot writes a fresh map. Skeleton mode is only a shallow
  observation policy: it clamps depth to three and leaves `children_count` on
  truncated nodes. Named structural anchors can receive drill-down refs, but
  inert containers do not become actionable merely because they were visible.
- Truncation has two paths and they carry different markers. A depth clamp
  knows how many children it skipped, so it leaves `children_count`. Budget or
  deadline exhaustion cannot afford the child-count read that would produce
  one, so every node whose descendants were cut carries `subtree_truncated`,
  which propagates to its ancestors and lets a reader walk from the root to
  the cut. A full snapshot that exhausts its budget returns the subtree it did
  observe with `complete: false` and `truncated: true` in `data`, rather than
  discarding the walk. A drill-down never does this: `--root` replaces refs
  inside an existing map, so it still requires a complete observation and
  errors instead of destructively merging a partial one.
- Derive the response window from the root entry's process identity with
  `window_lookup::find_window_for_process`; never synthesize a plausible
  window response.

## Why This Works

The selected session is a hard namespace boundary, qualification removes
snapshot ambiguity, and strict resolution protects the gap between observation
and a later read. Replacing one rooted subtree preserves unrelated refs while
ensuring no stale descendants survive a re-drill.

## Prevention

- Test full-snapshot replacement and rooted-subtree replacement separately.
- Test qualified and bare ref parsing, including mismatched snapshot IDs.
- Assert every truncation path exposes a boundary marker instead of silently
  dropping descendants. Depth clamping and budget exhaustion are separate
  paths; a test that only covers the clamp will not notice the other losing
  its marker.
- Assert a drill-down refuses to replace refs from an incomplete observation.
  A full snapshot may return a partial tree because it writes a fresh map and
  destroys nothing; a rooted replacement deletes descendants it may then be
  unable to re-allocate.
- Keep ref allocation in `crates/core/src/ref_alloc.rs`; full snapshots and
  drill-downs must not grow separate allocators.

## Related

- [Single-owner ref allocation](../best-practices/deduplicate-ref-allocator-via-config-struct-2026-04-14.md)
- [Playwright-grade desktop reliability contract](../best-practices/playwright-grade-desktop-reliability-2026-06-02.md)
