---

## title: Harden progressive snapshot after review (skeleton reset, drill window, validation, depth)
date: 2026-04-16
category: logic-errors
module: agent-desktop-core
problem_type: logic_error
component: tooling
symptoms:
  - Repeated `snapshot --skeleton` runs grew `ref_count` and retained stale drill-down refs because skeleton reused a merged on-disk `RefMap` instead of replacing it.
  - `snapshot --root @eN` returned `window.id` empty and a synthetic title, breaking the same JSON contract as window-rooted snapshots.
  - Malformed `--root` values surfaced as `STALE_REF` after a failed map lookup instead of `INVALID_ARGS` from format validation.
  - Deep non-skeleton subtree builds could return `None` at `ABSOLUTE_MAX_DEPTH` with no truncation marker, so agents saw missing branches without explanation.
root_cause: logic_error
resolution_type: code_fix
severity: high
tags:
  - progressive-snapshot
  - skeleton
  - drill-down
  - refmap
  - snapshot
  - macos
  - agent-contract

# Harden progressive snapshot after review (skeleton reset, drill window, validation, depth)

## Problem

A focused review of the `feat/progressive-skeleton-traversal` work found several **contract and state** issues: skeleton mode did not behave like a clean refresh of overview refs, drill-down responses did not carry real window identity, invalid `--root` strings were classified like missing refs, and the macOS tree builder could silently stop at the absolute depth cap. Follow-up work aligned behavior with agent expectations and locked the behavior with tests and docs.

## Symptoms

- Refmap growth and stale drill refs after multiple skeleton snapshots (merged load + selective removal).
- Drill-down JSON with empty `window.id` and generic title.
- `bad-ref` style `--root` values reported as stale rather than invalid input.
- Possible empty/missing subtrees at extreme depth without `children_count` or a boundary node.

## What Didn't Work

- **Treating skeleton as incremental merge** — convenient for caching, but agents and token budgets assume a fresh overview map unless documented otherwise.
- **Synthetic `WindowInfo` only** — fast to ship, but breaks any workflow that keys on `window.id` or correlates with `list-windows`.
- **Lookup-before-validate for `--root`** — reuses `stale_ref` for every missing key, including syntactically invalid IDs.

## Solution

1. **Full refmap replace on window snapshot** — `snapshot::build` always starts from `RefMap::new()` before `allocate_refs`, then `run` persists the result. Skeleton mode still passes `TreeOptions.skeleton` into `get_tree` for shallow traversal and boundary labeling; it no longer rehydrates prior refs for the overview path.
2. **Resolve real window for drill-down** — `snapshot_ref::run_from_ref` calls `adapter.list_windows` and picks the window matching `entry.pid`, with a fallback to the previous synthetic `WindowInfo` if listing fails.
3. **Validate `--root` early** — `commands/snapshot::execute` calls `validate_ref_id(root)` before `run_from_ref`, so malformed IDs return `invalid_input` / `INVALID_ARGS` instead of `STALE_REF`.
4. **Observable cap at `ABSOLUTE_MAX_DEPTH`** — `build_subtree` returns a **leaf boundary node** with `children_count` when `raw_depth >= ABSOLUTE_MAX_DEPTH` instead of `None`, so deep drill-downs are not silently dropped.
5. **Docs and tests** — `docs/phases.md` clarifies that `root` is CLI-only (`SnapshotArgs`), not `TreeOptions`; skills document `ref_id` in examples and batch `snapshot` args for `skeleton` / `root`. Integration tests cover skeleton depth, ref-count stability across refresh, invalid `--root`, and skeleton→drill flow.

## Why This Works

- **Reset semantics** match the mental model “snapshot the window again” — one `RefMap` per successful window snapshot save, no hidden accumulation from prior sessions on the same tree.
- **Window resolution** restores a stable JSON shape for automation that round-trips with window listing APIs.
- **Validation order** separates **bad syntax** from **stale or missing** refs, which is what agents need for recovery.
- **Boundary nodes** make the absolute depth cap **visible** in the tree instead of a silent prune.

## Prevention

- When changing snapshot or ref persistence, add or extend an integration test that asserts `**ref_count` is stable** across two identical `snapshot --skeleton` runs for the same app/window.
- Any new CLI flag that accepts a ref id should call `**validate_ref_id`** before map or adapter lookups.
- For subtree-only APIs, if a hard depth cap exists, return a **node with `children_count`** (or an explicit hint field) rather than `None`.
- Keep `**TreeOptions` vs CLI args** documented in `docs/phases.md` when adding root-like concepts so public architecture docs do not drift.

## Related Issues

- [Known pattern] DRY ref allocation — `[docs/solutions/best-practices/deduplicate-ref-allocator-via-config-struct-2026-04-14.md](../best-practices/deduplicate-ref-allocator-via-config-struct-2026-04-14.md)`
- Plan: `docs/plans/2026-03-10-feat-progressive-skeleton-traversal-plan.md`