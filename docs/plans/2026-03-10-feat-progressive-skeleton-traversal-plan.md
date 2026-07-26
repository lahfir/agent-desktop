---
title: "feat: progressive skeleton traversal with ref-rooted drill-down"
type: feat
status: active
date: 2026-03-10
deepened: 2026-03-10
origin: docs/brainstorms/2026-03-10-progressive-skeleton-traversal-brainstorm.md
---

# Progressive Skeleton Traversal

## Enhancement Summary

**Deepened on:** 2026-03-10
**Agents used:** architecture-strategist, performance-oracle, security-sentinel, pattern-recognition-specialist, code-simplicity-reviewer, agent-native-reviewer, data-integrity-guardian, best-practices-researcher

### Key Improvements from Review

1. **Scope reduction** — Cut `epoch` (redundant with per-element staleness), `new_ref_count` (agent computes delta), `mode` response field (agent knows what it asked for). Defer `find --root` and `--skeleton --root` combo to follow-up.
2. **Naming fix** — Rename `drill_down()` to `run_from_ref()` for consistency with existing `build()`/`run()` pattern.
3. **Performance** — Add `count_children()` using raw `CFArrayGetCount` to avoid N `CFRetain`/`CFRelease` pairs at skeleton boundary.
4. **Data integrity** — Add write-side size check to `RefMap::save()` to prevent >1MB files that would fail on next load.
5. **LOC management** — `snapshot.rs` is at 354 LOC. Extract ref-rooted logic to `snapshot_ref.rs` to stay under 400.
6. **Security** — Document TOCTOU race on refmap as known limitation, defer file locking to Phase 4.

### Simplifications Applied

| Original | Decision | Rationale |
|----------|----------|-----------|
| `epoch: u32` on RefEntry/RefMap | **Cut** | Existing per-element re-identification via `(pid, role, name, bounds_hash)` already handles staleness. Epoch adds a second, redundant mechanism. |
| `new_ref_count` in response | **Cut** | Agent can diff `ref_count` across calls if needed. One number is sufficient. |
| `mode` field in response | **Cut** | Agent knows what it passed (`--skeleton` or `--root`). Presence of `children_count` is the structural signal. |
| `find --root` | **Deferred** | Scope creep. Agent can do `snapshot --root @e3` then `find` separately. Follow-up PR. |
| `--skeleton --root` combo | **Deferred** | Edge case for very deep subtrees. Add in follow-up if agents need it. Reduces testing matrix. |
| `increment_epoch()` | **Cut** | No epoch to increment. |

---

## Overview

Add a two-phase incremental tree traversal system that lets AI agents explore dense accessibility trees progressively — skeleton overview first, then targeted drill-downs — instead of consuming the full tree every time. This introduces two new `snapshot` flags (`--skeleton`, `--root @ref`), refmap merging with `root_ref`-tagged scoped invalidation, and a new `PlatformAdapter::get_subtree()` trait method.

Token savings for Slack: **73-96%** vs current full snapshot (see brainstorm: `docs/brainstorms/2026-03-10-progressive-skeleton-traversal-brainstorm.md`).

## Problem Statement

Dense Electron apps (Slack, VS Code, Discord) produce massive accessibility trees — 500+ nodes, ~12,000 tokens per full snapshot. Even with `--interactive-only --compact`, Slack still costs ~5,400 tokens (~204 nodes). Agents pay per-token and struggle to navigate large trees efficiently.

Current optimizations (depth-skip, compact, surfaces, interactive-only) reduce output size but still require full tree traversal from the window root. There is no way to:
- Get a cheap structural overview of an app
- Drill into a specific region without re-traversing the entire tree
- Accumulate knowledge across multiple targeted observations

## Proposed Solution

Two new capabilities on the `snapshot` command:

1. **`--skeleton`** — Shallow overview (default depth 3) where truncated nodes show `children_count` instead of full subtrees. Named containers at the truncation boundary receive refs as drill-down targets.

2. **`--root @ref`** — Start traversal from a previously-discovered ref element instead of the window root. Merges new refs into the existing refmap with `root_ref`-tagged scoped invalidation.

These compose with all existing flags (`--compact`, `--interactive-only`, `--max-depth`, `--surface`).

## Technical Approach

### Architecture

The feature touches three layers:

```
CLI (cli_args.rs, dispatch.rs, typed batch path)
  ↓ --skeleton, --root flags
Core (adapter.rs, snapshot.rs, snapshot_ref.rs [NEW], refs.rs, node.rs)
  ↓ TreeOptions, skeleton logic, merge logic, children_count
Platform (macos/adapter.rs, macos/tree/builder.rs, macos/tree/element.rs)
  ↓ get_subtree(), skeleton-aware build_subtree(), count_children()
```

One new file: `crates/core/src/snapshot_ref.rs` — extracted drill-down logic to keep `snapshot.rs` under 400 LOC. The dependency inversion principle is preserved — core defines the interface, platform implements it.

### Research Insights: Architecture

**From architecture-strategist:**
- The plan initially proposed `get_subtree()` as a separate trait method. An alternative is to extend `get_tree()` with an optional `&NativeHandle` parameter. However, `NativeHandle` contains a raw pointer and would break `TreeOptions`'s `Clone`/`Copy` derives. **Decision: keep `get_subtree()` as additive trait method** — both `get_tree()` and `get_subtree()` delegate to the same internal `build_subtree()` in each platform adapter, preventing behavior divergence.
- `snapshot.rs` is at 354 LOC. Adding `run_from_ref()` and merge logic would push it over 400. **Decision: extract to `snapshot_ref.rs`** with `pub(crate)` visibility, re-exported from `snapshot.rs`.

**From pattern-recognition-specialist:**
- `drill_down()` does not follow the verb-first naming convention (existing: `build()`, `run()`, `allocate_refs()`). **Renamed to `run_from_ref()`** to match the existing `run()` pattern.
- TreeOptions grows from 5 to 7 fields (at the CLAUDE.md boundary of 7). Acceptable since both new fields are simple primitives. If more fields are needed later, extract a `SnapshotMode` enum.

### Data Flow: Skeleton Mode

```
snapshot --app Slack --skeleton -i
  → SnapshotArgs { skeleton: true, interactive_only: true }
  → TreeOptions { skeleton: true, interactive_only: true, ... }
  → snapshot::build() [existing path]
    → adapter.get_tree(window, opts)  [skeleton flag in opts]
      → build_subtree(el, 0, 3, true, ...) [skeleton_depth=3]
        → At depth limit: count_children() [O(1), no per-element CFRetain]
        → Set children_count on node, return with empty children vec
    → allocate_refs(tree, &mut fresh_refmap, opts)
      → Interactive elements: assign refs (existing)
      → Named containers at boundary with children_count > 0: assign refs (NEW)
      → Pruning: skip if non-interactive AND children empty AND children_count is None
  → refmap.save() [full replace]
  → JSON response
```

### Data Flow: Drill-Down Mode

```
snapshot --root @e3 -i --compact
  → SnapshotArgs { root_ref: Some("@e3"), interactive_only: true, compact: true }
  → snapshot_ref::run_from_ref(adapter, opts, "@e3") [NEW function, NEW file]
    → resolve_ref("@e3") → (RefEntry, NativeHandle)
    → Derive app/window context from RefEntry.pid
    → adapter.get_subtree(handle, opts)  [NEW trait method]
      → macOS: ManuallyDrop AXElement from handle, build_subtree()
    → existing_refmap = RefMap::load()
    → existing_refmap.remove_by_root_ref("@e3") [scoped invalidation]
    → allocate_refs(subtree, &mut existing_refmap, opts)  [counter continues]
      → New refs tagged with root_ref: "@e3"
    → existing_refmap.save() [merged, with write-side size check]
  → JSON response with ref_count (total)
```

### Research Insights: Performance

**From performance-oracle:**
- **Children count at skeleton boundary**: `copy_children()` triggers a Mach IPC call that materializes the entire CFArray of child AXUIElementRefs. Even calling `.len()` pays full IPC + CFArray allocation. **Recommendation: Add a `count_children()` function** that calls `AXUIElementCopyAttributeValue` then `CFArrayGetCount` on the raw `CFArrayRef`, then releases — without materializing per-element `AXElement` wrappers. Saves N `CFRetain`/`CFRelease` pairs per truncation boundary node.
- **RefMap file I/O**: ~3-4ms for load+save of a 500-entry refmap (50-80KB). Negligible against 50-200ms total CLI invocation time. No optimization needed.
- **Counter collision**: Safe — counter continues from loaded refmap. `allocate()` increments before formatting, so first new ref is `@e{loaded_counter+1}`.
- **Branch cost of `skeleton: bool` in recursive `build_subtree()`**: Near-zero. Branch prediction handles it (the flag is constant across the entire recursive call tree).
- **Net savings**: CLI startup is ~10ms. Even with 3 invocations (skeleton + 2 drill-downs), total overhead is ~30ms vs ~5,400 tokens saved. Token cost dominates by orders of magnitude.

### Implementation Phases

#### Phase 1: Data Model Foundation

Add new fields to core types. No behavioral changes yet — all new fields are optional with serde defaults for backward compatibility.

**`crates/core/src/node.rs`** — AccessibilityNode:
- [ ] Add `children_count: Option<u32>` with `#[serde(skip_serializing_if = "Option::is_none", default)]`
- [ ] Place before `children` field (line ~29)
- [ ] Update test helper `make_node()` in `snapshot.rs` to include field

**`crates/core/src/refs.rs`** — RefEntry:
- [ ] Add `root_ref: Option<String>` with `#[serde(skip_serializing_if = "Option::is_none", default)]`

**`crates/core/src/refs.rs`** — RefMap:
- [ ] Add `fn remove_by_root_ref(&mut self, root: &str)` — `self.inner.retain(|_, v| v.root_ref.as_deref() != Some(root))`
- [ ] Update `allocate()` to accept `root_ref: Option<&str>` parameter, store on each new `RefEntry`
- [ ] Add write-side size check in `save()`: if serialized JSON exceeds `MAX_REFMAP_BYTES`, return error instead of writing (preserves previous valid refmap on disk)

**`crates/core/src/adapter.rs`** — TreeOptions:
- [ ] Add `skeleton: bool` (default `false`)
- [ ] Add `root_ref: Option<String>` (default `None`)
- [ ] Update `Default` impl

**`crates/core/src/adapter.rs`** — PlatformAdapter trait:
- [ ] Add `fn get_subtree(&self, handle: &NativeHandle, opts: &TreeOptions) -> Result<AccessibilityNode, AdapterError>` with default `not_supported("get_subtree")`

### Research Insights: Data Model

**From data-integrity-guardian:**
- **Write-side size check is critical.** Currently `MAX_REFMAP_BYTES` (1MB) is checked only on `load()`. A series of drill-downs could produce a >1MB file that then fails on every subsequent `load()`, making all refs inaccessible. The fix: check in `save()` and fail early, preserving the previous valid file (temp+rename never completes).
- **Orphaned refs from dead PIDs** accumulate but are harmless — `resolve_element()` returns `STALE_REF` naturally. No cleanup needed.

**From best-practices-researcher (serde):**
- `#[serde(default)]` on `Option<String>` fields defaults to `None`. Correct for backward compat.
- `#[serde(skip_serializing_if = "Option::is_none")]` prevents writing `null` fields. Correct.
- Do NOT use `#[serde(deny_unknown_fields)]` — allow unknown fields for forward compatibility (newer binary writes fields that older binary doesn't know about).

**Tests (Phase 1):**
- [ ] Backward compat: deserialize old RefEntry JSON (missing root_ref) → defaults to `root_ref: None`
- [ ] `remove_by_root_ref()`: removes only matching entries, preserves others
- [ ] `AccessibilityNode` with `children_count: Some(5)` and empty `children` serializes correctly (no `children` key, has `children_count`)
- [ ] `RefMap::save()` rejects oversized refmaps (>1MB) with actionable error
- [ ] `RefMap::save()` on oversized rejection: previous file on disk is preserved

#### Phase 2: Skeleton Mode

Implement the `--skeleton` flag end-to-end.

**`crates/macos/src/tree/element.rs`** — New `count_children()` function:
- [ ] Add `pub fn count_children(el: &AXElement) -> usize` that calls `AXUIElementCopyAttributeValue` for `kAXChildrenAttribute`, then `CFArrayGetCount` on raw `CFArrayRef`, then `CFRelease` — without materializing per-element `AXElement` wrappers
- [ ] Returns 0 if attribute fetch fails

**`crates/macos/src/tree/builder.rs`** — build_subtree():
- [ ] Accept `skeleton: bool` parameter (add to function signature)
- [ ] When `skeleton=true` and `depth >= skeleton_depth`: call `count_children(el)` instead of `copy_children()`, set `children_count` on the node, return the node with empty children vec
- [ ] Web-wrapper depth-skip still applies during skeleton traversal (existing behavior preserved)
- [ ] Skeleton depth constant: `SKELETON_DEFAULT_DEPTH: u8 = 3`
- [ ] Effective skeleton depth: `if skeleton { min(max_depth, SKELETON_DEFAULT_DEPTH) } else { max_depth }` — `--max-depth` can make it shallower but not deeper than 3 in skeleton mode

**`crates/core/src/snapshot.rs`** — allocate_refs():
- [ ] Accept `skeleton: bool` parameter
- [ ] New ref assignment rule for skeleton mode: if node has `children_count.is_some()` AND node has a non-empty name → assign a ref (drill-down target), even if role is not interactive
- [ ] `available_actions` for skeleton boundary refs: empty `vec![]`
- [ ] Fix `interactive_only` pruning: change guard to `child.ref_id.is_none() && child.children.is_empty() && child.children_count.is_none()` — do NOT prune truncated nodes that have `children_count` set

**`crates/core/src/commands/snapshot.rs`** — SnapshotArgs:
- [ ] Add `skeleton: bool` field
- [ ] Pass to TreeOptions construction

**`src/cli_args.rs`** — SnapshotArgs:
- [ ] Add `#[arg(long, help = "Shallow overview with children_count on truncated nodes")]` `skeleton: bool`

**`src/dispatch.rs`**:
- [ ] Pass `skeleton` through to core SnapshotArgs

**`src/typed batch path`**:
- [ ] Parse `skeleton` from JSON in batch snapshot dispatch

### Research Insights: Skeleton Mode

**From performance-oracle:**
- At the skeleton truncation boundary, `count_children()` still triggers one Mach IPC call per boundary node. For a depth-3 skeleton with ~50 boundary nodes, that is 5-25ms overhead. Acceptable — the savings (avoiding 450+ deeper IPC calls) vastly outweigh it.

**From code-simplicity-reviewer:**
- `allocate_refs()` currently has 7 parameters (already exceeds the 5-param max from CLAUDE.md). Adding `skeleton: bool` makes 8. **Recommendation: pass `skeleton` via `TreeOptions`** (already done — `opts.skeleton` is available). Do NOT add a separate parameter. Access it from the `opts` struct that's already threaded through.

**Tests (Phase 2):**
- [ ] Skeleton of mock tree with depth 5: output has max depth 3, truncated nodes have `children_count`, no `children` array
- [ ] Named container at boundary gets a ref; anonymous wrapper at boundary does not
- [ ] `interactive_only` does NOT prune truncated nodes with `children_count`
- [ ] `--skeleton --max-depth 2` produces depth-2 output
- [ ] `--skeleton --compact` composes: compact collapses within skeleton depth
- [ ] Skeleton ref_count includes both interactive refs and drill-down target refs
- [ ] Golden fixture: skeleton of standard test tree
- [ ] `count_children()` does not materialize AXElement wrappers (unit test in macos crate)

#### Phase 3: Drill-Down Mode

Implement `--root @ref` with refmap merging.

**`crates/macos/src/adapter.rs`** — MacOSAdapter:
- [ ] Implement `get_subtree(&self, handle: &NativeHandle, opts: &TreeOptions) -> Result<AccessibilityNode, AdapterError>`
- [ ] Extract AXElement from handle: `ManuallyDrop::new(AXElement(handle.as_raw() as AXUIElementRef))` (existing pattern from `get_live_value`)
- [ ] Call `build_subtree(&el, 0, opts.max_depth, opts.include_bounds, &mut FxHashSet::default())` — fresh ancestor set
- [ ] Delegate to same `build_subtree()` as `get_tree()` — prevents behavior divergence

**`crates/core/src/snapshot_ref.rs`** — New file (~80-100 LOC):
- [ ] `pub(crate) fn run_from_ref(adapter: &dyn PlatformAdapter, opts: &TreeOptions, root_ref: &str) -> Result<RefRootResult, AppError>`
- [ ] Step 1: `resolve_ref(root_ref, adapter)` → `(entry, handle)`. On failure → `STALE_REF` with suggestion "Run 'snapshot --skeleton' to refresh"
- [ ] Step 2: Derive app context from `entry.pid` and `entry.source_app`
- [ ] Step 3: `adapter.get_subtree(&handle, opts)` → raw subtree
- [ ] Step 4: `RefMap::load()` → existing refmap
- [ ] Step 5: `existing_refmap.remove_by_root_ref(root_ref)` — scoped invalidation
- [ ] Step 6: `allocate_refs(subtree, &mut existing_refmap, opts)` with `root_ref_tag: Some(root_ref)` — counter continues from loaded refmap
- [ ] Step 7: `existing_refmap.save()` (includes write-side size check)
- [ ] Step 8: Return `RefRootResult { tree, refmap, app_name, window }`
- [ ] **Skeleton boundary ref preservation**: @e3 itself (from skeleton, with `root_ref: None`) is NOT removed by scoped invalidation. Only refs where `root_ref == "@e3"` are removed. The agent keeps using @e3 for re-drilling.

**`crates/core/src/snapshot.rs`**:
- [ ] Add `pub mod snapshot_ref;` or re-export from the module
- [ ] In existing code, no changes needed — `build()` and `run()` are untouched

**`crates/core/src/commands/snapshot.rs`** — execute():
- [ ] Branch: if `args.root_ref.is_some()` → call `snapshot_ref::run_from_ref()` instead of `run()`
- [ ] Format response with `ref_count` (total)
- [ ] Derive `app` and `window` from the resolved `RefEntry.source_app` and PID window lookup

**`src/cli_args.rs`**:
- [ ] Add `#[arg(long, value_name = "REF", help = "Start from a previously-discovered ref")]` `root: Option<String>`

**`src/dispatch.rs`**:
- [ ] Pass `root` as `root_ref` through to core SnapshotArgs

**`src/typed batch path`**:
- [ ] Parse `root` from JSON in batch snapshot dispatch: `root_ref: str_field(&args, "root")`

**Argument validation:**
- [ ] `--root` + `--surface` → return `INVALID_ARGS` with message "Cannot use --root and --surface together"
- [ ] Validate ref format (`@e{N}`) before attempting resolution

### Research Insights: Drill-Down Mode

**From security-sentinel:**
- **TOCTOU race (HIGH):** The load → modify → save cycle on the refmap has no file locking. Concurrent `agent-desktop` invocations can silently overwrite each other's refs. The atomic rename prevents corruption but not lost updates. **Mitigation for now:** Document as known limitation. **For Phase 4:** Add advisory `flock()`/`fcntl()` around the load-modify-save cycle.
- **PID reuse (MEDIUM):** If an app restarts with a different PID, the old PID could theoretically belong to a different process. The `resolve_element()` function already matches on `(pid, role, name, bounds_hash)` which makes accidental cross-process resolution extremely unlikely. Acceptable risk.
- **Write-side size check:** Implemented in Phase 1 (RefMap::save). Prevents the scenario where drill-downs produce a >1MB file that blocks all future operations.

**From agent-native-reviewer:**
- **Batch dispatch parity is critical.** Every new CLI field must have an explicit batch JSON parsing line in `typed batch path`. The plan now includes explicit checklist items for both `skeleton` and `root` parsing.
- **Batch sequencing works:** A batch like `[{"command": "snapshot", "skeleton": true}, {"command": "snapshot", "root": "@e3"}]` executes sequentially, and the second command reads the refmap written by the first. This is the existing batch execution model.

**Tests (Phase 3):**
- [ ] Drill-down from a ref: subtree returned with correct depth, refs merge into existing map
- [ ] Scoped invalidation: drill into @e3, refs with `root_ref: "@e3"` removed, others preserved
- [ ] Skeleton boundary ref (@e3 with `root_ref: None`) preserved after drill-down
- [ ] Re-drill same region: old subtree refs replaced, new ones added, counter continues
- [ ] Multiple drill-downs accumulate: refs from @e3 and @e7 coexist
- [ ] Stale root ref: returns STALE_REF with skeleton suggestion
- [ ] `--root @e3 --surface menu` → INVALID_ARGS error
- [ ] Empty subtree (root has no children): valid response with ref_count of 0 or 1
- [ ] Counter continuity: after skeleton (refs @e1-@e10), drill-down creates @e11+
- [ ] Golden fixture: drill-down result merged refmap
- [ ] Write-side size check: rapid drill-downs approaching 1MB → error, previous refmap preserved

#### Phase 4: Polish and Documentation

- [ ] Update STALE_REF suggestion in `crates/core/src/commands/helpers.rs`: when `STALE_REF` occurs and the ref was from a drill-down (has `root_ref`), suggest "Run 'snapshot --skeleton' to refresh, then re-drill"
- [ ] Update skill files: `skills/agent-desktop/references/commands-observation.md` — document `--skeleton`, `--root` flags
- [ ] Add golden fixtures for skeleton and drill-down JSON output
- [ ] Run full clippy + fmt + test suite

### Deferred to Follow-Up PRs

These were cut from scope based on simplicity review:

- [ ] **`find --root @ref`** — Scoped search within a ref's subtree. Agent can achieve this today with `snapshot --root @e3` then `find`. Follow-up PR.
- [ ] **`--skeleton --root @ref` combo** — Skeleton view of a subtree. Useful for very deep regions. Follow-up PR.
- [ ] **`epoch` counter on RefMap/RefEntry** — Was deemed redundant with existing per-element staleness detection. Revisit if agents report confusion about ref freshness.
- [ ] **Advisory file locking (`flock`)** — Prevents TOCTOU race on refmap during concurrent access. Deferred to Phase 4 daemon.
- [ ] **RefMap size eviction policy** — Auto-purge oldest refs when approaching 1MB. For now, agents should periodically run `--skeleton` to reset.

## Alternative Approaches Considered

(see brainstorm: `docs/brainstorms/2026-03-10-progressive-skeleton-traversal-brainstorm.md`)

1. **Semantic Zone Auto-Detection** — Auto-label regions ("sidebar", "toolbar") via role heuristics. Rejected: fragile across apps, platform-dependent role semantics, introduces new concept (zones) alongside refs.

2. **Query-Driven Traversal** — Skip trees, agents search directly with `find --near "Channels"`. Rejected: requires agents to know what they're looking for, fails for the "orient myself in an unfamiliar app" use case.

3. **Auto-skeleton default** — Make `--skeleton` the default when `--compact --interactive-only` are both set. Rejected: changes existing behavior, could break agents already using those flags.

4. **Extend `get_tree()` signature instead of adding `get_subtree()`** — Architecture reviewer suggested adding `Option<&NativeHandle>` to `get_tree()`. Rejected: `NativeHandle` contains raw pointers, would break `TreeOptions`'s `Clone`/`Copy` derives. The additive `get_subtree()` method is cleaner and follows the project's pattern of default `not_supported` implementations.

## System-Wide Impact

### Interaction Graph

```
snapshot --skeleton → build() → adapter.get_tree() → build_subtree() [skeleton-aware]
                    → allocate_refs() [skeleton-aware ref assignment]
                    → refmap.save() [full replace]

snapshot --root → run_from_ref() → resolve_ref() → adapter.get_subtree()
                                 → RefMap::load() → remove_by_root_ref()
                                 → allocate_refs() [merge into loaded refmap]
                                 → refmap.save() [merged, size-checked]

get @ref → RefMap::load() → resolve → [transparent, no changes needed]
click @ref → RefMap::load() → resolve → [transparent, no changes needed]
```

### Error Propagation

- `--root @e3` with stale ref → `resolve_ref()` fails → `STALE_REF` (exit 1) with skeleton suggestion
- `--root @e3 --surface menu` → validation in `execute()` → `INVALID_ARGS` (exit 2)
- `adapter.get_subtree()` fails (e.g., app crashed mid-traversal) → `ACTION_FAILED` propagated
- RefMap load failure during merge → `INTERNAL` error (file permissions, corrupt JSON)
- RefMap save exceeds 1MB → `INTERNAL` error with suggestion to run full snapshot to reset

### State Lifecycle Risks

- **RefMap is the critical state artifact.** Skeleton replaces entirely (safe). Drill-down merges (partial update). Concurrent access (two agents) can cause lost updates — documented, deferred to Phase 4.
- **Unbounded refmap growth:** Multiple drill-downs without a skeleton reset accumulate refs. Write-side size check (1MB) prevents catastrophic failure. Agent best practice: periodically run `--skeleton` to reset.
- **TOCTOU race (documented):** The load → modify → save cycle has no file locking. Atomic rename prevents corruption but not lost updates. Phase 4 will add advisory `flock()`.

### API Surface Parity

| Interface | Needs update |
|-----------|-------------|
| `snapshot` CLI command | Yes — `--skeleton`, `--root` flags |
| `batch` JSON API | Yes — `skeleton`, `root` fields |
| MCP server (Phase 3) | Will inherit via dispatch |
| All action commands | No — transparent via merged refmap |
| `get`, `is` commands | No — transparent via merged refmap |
| `find` CLI command | Deferred — follow-up PR |

### Integration Test Scenarios

1. **End-to-end progressive workflow**: `snapshot --skeleton` → `snapshot --root @e3` → `click @e45` → verify action succeeds on element from drill-down
2. **Re-drill after UI change**: `snapshot --skeleton` → `snapshot --root @e3` → [UI changes] → `snapshot --root @e3` again → verify stale refs replaced, fresh refs work
3. **Cross-region interaction**: `snapshot --skeleton` → `snapshot --root @e3` → `snapshot --root @e7` → `click @e25` (from @e3 drill-down) → verify still works
4. **Full snapshot resets**: `snapshot --skeleton` → `snapshot --root @e3` → `snapshot --app Slack -i` (no flags) → verify refmap fully replaced, drill-down refs gone
5. **Batch with skeleton + drill-down**: batch JSON with skeleton first, drill-down second → verify sequential execution and refmap merge

## Acceptance Criteria

### Functional Requirements

- [ ] `snapshot --skeleton -i` returns max depth 3 tree with `children_count` on truncated nodes
- [ ] Named containers at skeleton boundary receive refs as drill-down targets
- [ ] `snapshot --root @e3` returns subtree rooted at @e3, merges refs into existing refmap
- [ ] Scoped invalidation: re-drilling @e3 removes only @e3-rooted refs, preserves others
- [ ] `--root @ref --surface menu` returns `INVALID_ARGS`
- [ ] Stale root ref returns `STALE_REF` with skeleton suggestion
- [ ] All existing flags compose: `--skeleton --compact --interactive-only`, `--root @e3 --compact --interactive-only --max-depth 5`
- [ ] Write-side refmap size check prevents >1MB files

### Non-Functional Requirements

- [ ] Skeleton snapshot of Slack: < 50 nodes, < 1,000 tokens
- [ ] Drill-down of Slack sidebar: < 100 nodes, < 2,000 tokens
- [ ] No regression in full snapshot performance (same path when no new flags used)
- [ ] Backward compatibility: old refmap files (no root_ref) deserialize with defaults

### Quality Gates

- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo test --lib --workspace` passes (all new + existing tests)
- [ ] No file exceeds 400 LOC (snapshot_ref.rs extraction keeps snapshot.rs under limit)
- [ ] Zero `unwrap()` in non-test code
- [ ] Golden fixtures for skeleton output and merged refmap

## Dependencies & Prerequisites

- No new crate dependencies
- No platform-specific dependencies beyond what exists
- Windows/Linux stubs inherit `get_subtree()` default (`not_supported`) — no stub changes needed

## Risk Analysis & Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| `interactive_only` pruning removes truncated skeleton nodes | High — drill-down targets lost | Guard: only prune if `children_count.is_none()` |
| Ref ID collision during merge | High — corrupt refmap | Counter continues from loaded refmap, never resets during merge |
| Concurrent drill-down race condition | Medium — lost updates | Documented. Atomic write prevents corruption. flock in Phase 4. |
| RefMap exceeds 1MB after many drill-downs | Medium — all refs inaccessible | Write-side size check in save(). Fail early, preserve previous file. |
| `children_count` not O(1) on Linux AT-SPI | Low (Phase 3) | Verify during Linux adapter. Fallback: quick recursive count. |
| `--skeleton --max-depth 100` defeats purpose | Low — foot-gun | Cap: `min(max_depth, SKELETON_DEFAULT_DEPTH)` |
| `snapshot.rs` exceeds 400 LOC | Low — coding standard violation | Extract to `snapshot_ref.rs` |

## File Change Summary

| File | Change | LOC Impact |
|------|--------|------------|
| `crates/core/src/node.rs` | Add `children_count` field | +2 |
| `crates/core/src/refs.rs` | Add `root_ref` to RefEntry, `remove_by_root_ref()` + save size check to RefMap | +25 |
| `crates/core/src/adapter.rs` | Add fields to TreeOptions, `get_subtree()` to trait | +15 |
| `crates/core/src/snapshot.rs` | Skeleton logic in `allocate_refs()`, pruning fix | +20 |
| `crates/core/src/snapshot_ref.rs` | **NEW** — `run_from_ref()` drill-down logic | +80-100 |
| `crates/core/src/commands/snapshot.rs` | Branch on `root_ref`, skeleton args | +15 |
| `crates/macos/src/adapter.rs` | Implement `get_subtree()` | +15 |
| `crates/macos/src/tree/builder.rs` | Skeleton-aware truncation | +15 |
| `crates/macos/src/tree/element.rs` | `count_children()` function | +15 |
| `src/cli_args.rs` | `--skeleton`, `--root` flags | +5 |
| `src/dispatch.rs` | Pass new args | +3 |
| `src/typed batch path` | Parse new JSON fields | +5 |
| **Total** | | **~215-235 new LOC** |

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-03-10-progressive-skeleton-traversal-brainstorm.md](docs/brainstorms/2026-03-10-progressive-skeleton-traversal-brainstorm.md) — Key decisions: skeleton with children_count hint, ref-rooted drill-down with scoped merge, named containers get refs at skeleton boundary, `--skeleton` flag naming, opt-in only.

### Internal References

- `crates/core/src/snapshot.rs:127-149` — `append_surface_refs()` precedent for refmap merge pattern
- `crates/core/src/snapshot.rs:160-215` — `allocate_refs()` where skeleton logic and pruning fix go
- `crates/core/src/refs.rs:8-28` — RefEntry and RefMap structs to extend
- `crates/core/src/adapter.rs:27-45` — TreeOptions to extend
- `crates/core/src/adapter.rs:115-247` — PlatformAdapter trait for new method
- `crates/macos/src/adapter.rs:33-56` — `get_tree()` pattern to follow for `get_subtree()`
- `crates/macos/src/tree/builder.rs:44-119` — `build_subtree()` to make skeleton-aware
- `crates/macos/src/tree/element.rs` — Where `count_children()` goes
- `crates/core/src/commands/helpers.rs:11-34` — `resolve_ref()` used by drill-down path

### Review Agents Applied

| Agent | Key Finding |
|-------|-------------|
| architecture-strategist | Keep `get_subtree()` additive; extract `snapshot_ref.rs` for LOC management |
| performance-oracle | Add `count_children()` using raw CFArrayGetCount; RefMap I/O negligible |
| security-sentinel | TOCTOU race documented, defer flock to Phase 4; write-side size check critical |
| pattern-recognition | Rename `drill_down` → `run_from_ref`; TreeOptions at 7-field boundary |
| code-simplicity-reviewer | Cut epoch, new_ref_count, mode; defer find --root and skeleton+root combo |
| agent-native-reviewer | Explicit batch dispatch parsing for all new fields |
| data-integrity-guardian | Write-side size check prevents catastrophic refmap failure |

### Related Work

- `docs/brainstorms/2026-03-01-electron-tree-compaction-brainstorm.md` — compact mode (already implemented), composes with skeleton
- `docs/brainstorms/2026-02-19-ax-tree-accuracy-speed-brainstorm.md` — tree accuracy insights
