---
title: Deduplicate ref allocator via RefAllocConfig instead of a _with_X copy
date: 2026-04-14
category: best-practices
module: crates/core
problem_type: best_practice
component: tooling
severity: medium
applies_when:
  - Two functions share identical bodies differing only in one Optional/nullable field (classic _with_X naming)
  - A positional parameter list carries five or more args, including bool flags
  - A patch commit touches only one copy of duplicated traversal logic, causing silent drift in the other
  - A code review flags a DRY violation on ref-allocation or tree-traversal paths
  - Adding a new per-call config axis (e.g. root_ref_id) would require another _with_X variant
tags:
  - dry
  - ref-allocation
  - config-struct
  - skeleton-traversal
  - drill-down
  - deduplication
  - rust-patterns
  - code-review
---

# Deduplicate ref allocator via RefAllocConfig instead of a _with_X copy

## Context

The progressive skeleton traversal feature shipped two near-identical ref allocators into `crates/core/`:

- `allocate_refs` in `crates/core/src/snapshot.rs` — 7 positional parameters, called from the full-snapshot path
- `allocate_refs_with_root` in `crates/core/src/snapshot_ref.rs` — took a local `DrillDownConfig<'a>` struct, called from the drill-down path

Both bodies were ~95% identical: same `INTERACTIVE_ROLES` check, same `ref_entry_from_node` call, same skeleton-anchor detection (`!is_interactive && node.children_count.is_some() && has_label`), same bounds filtering, same compact-collapse, same `interactive_only` pruning. They differed in exactly one thing: whether each allocated `RefEntry` carried a `root_ref: None` or `root_ref: Some(root_ref_id.to_string())` tag.

The duplication did not land because an author was lazy. (session history) It landed because the March 10 implementation session used two parallel worktree agents — one owning `snapshot.rs` + `builder.rs`, the other owning `snapshot_ref.rs` + the macOS adapter. Each agent wrote its own allocator in isolation. When the merge agent integrated both worktrees, it hit the project's 400 LOC file limit on `snapshot.rs` and extracted `ref_alloc.rs` purely as a LOC-budget fix — pulling out the *small* helpers (`INTERACTIVE_ROLES`, `actions_for_role`, `ref_entry_from_node`, `is_collapsible`) while leaving the two full allocator bodies in their original files. The merge agent's summary explicitly listed `allocate_refs_with_root` and `DrillDownConfig` as distinct items belonging to `snapshot_ref.rs`. No decision was ever made to leave them separate for design reasons; it was the path of least resistance under a LOC deadline.

The brainstorm at `docs/brainstorms/2026-03-10-progressive-skeleton-traversal-brainstorm.md` described `ref_alloc.rs` as the home for "shared ref-allocation logic" but only named `INTERACTIVE_ROLES` and `is_collapsible`. It did not specify that the full allocator body should live there, leaving a gap the implementation filled with a copy.

## Guidance

**Any two function bodies that are structurally identical except for a small number of parameter values or literals are a single function waiting to be written.** The correct fix is a shared config struct whose fields default to — or accept an `Option<T>` for — the differentiating value. Copy-and-modify with a name suffix (`foo_with_X`, `foo_v2`, `foo_alternate`, `do_thing_for_Y`) is never the right tool; it is technical debt that generates silent divergence under the first patch.

**Concrete rule for this codebase:** `crates/core/src/ref_alloc.rs` is the single source of truth for the ref-allocation pass. `RefAllocConfig::root_ref_id` is `Option<&'a str>`:

- `snapshot.rs::build` (and `append_surface_refs`) passes `root_ref_id: None`
- `snapshot_ref.rs::run_from_ref` passes `root_ref_id: Some(root_ref_id)`

The function body is identical for both callers. Any future change to allocation logic — adding a new role to `INTERACTIVE_ROLES`, changing skeleton-anchor detection, altering `is_collapsible`, tuning the `interactive_only` pruning rule — is made once in `ref_alloc::allocate_refs` and immediately applies to both the full-snapshot and the drill-down path.

**The threshold to extract a config struct:** (auto memory [claude]) when a function already takes four or more positional parameters and the new variant adds one more distinguishing value. Do not add an eighth positional parameter. Do not copy the body. Extract a config struct with all fields (including the new one as `Option<T>`) and unify.

**Applies to tests too.** Two test helper closures that differ only in a flag value are a single parametrised helper. The drill-down tests in `snapshot_ref.rs` already follow this via `drill_config(source_app, pid, root_ref_id, interactive_only, compact)` returning a `RefAllocConfig`.

## Why This Matters

The divergence was not theoretical. (session history) Two separate commits drifted the copies in opposite directions before the duplication was even two weeks old:

**Incident 1 — `cec176d fix: preserve skeleton drill-down anchors`**: repaired logic that ensured skeleton anchors retained their `root_ref` tag correctly. The fix was applied only to `allocate_refs_with_root` in `snapshot_ref.rs`. The matching logic in `allocate_refs` in `snapshot.rs` was not touched. Snapshot and drill-down paths became silently inconsistent on anchor tagging.

**Incident 2 — `0fcf4e8 fix: bypass AXConfirm on web elements and add skeleton anchors to drill-down`**: added the `!is_interactive && node.children_count.is_some() && has_label` skeleton-anchor detection to `allocate_refs_with_root`. The same logic already existed in `allocate_refs` — it had been added there in the original `b13dc69 feat: implement progressive skeleton traversal` commit. The author of `0fcf4e8` had to check both copies, noticed the gap in the drill-down copy, and patched it. But the patch direction was reversed from `cec176d`: this time the snapshot copy was ahead and the drill-down copy was behind.

Two patches, two weeks, opposite directions of drift, within the same feature branch. That is the expected rate of rot for any copied core algorithm in this codebase.

**Review cost**: every reviewer on PR #20 had to understand both bodies to confirm they were equivalent. Because they lived in different files with different struct names (`window_pid` vs `pid`, seven positional args vs `&DrillDownConfig`), confirming equivalence required careful side-by-side reading rather than a single glance. The Cursor Bugbot eventually flagged it as "Low Severity". **Low was wrong.** (auto memory [claude]) Per `feedback_dry_is_core.md`, any duplicated core algorithm is P1 minimum, because the next fix will always land in exactly one copy.

## When to Apply

**Trigger 1 — identical recursive body.** If you are writing a recursive tree-walk and the only difference from an existing recursive tree-walk is one field on the entries it produces, unify. Recursive duplicates are particularly high-risk because the recursive self-call must also be updated in both copies on every future change.

**Trigger 2 — name suffix pattern.** Function names ending in `_with_X`, `_v2`, `_alternate`, `_for_Y` are red flags. Before adding such a function, ask: can the original accept an `Option<X>` parameter or a config struct with `X` as an optional field?

**Trigger 3 — config struct threshold.** Four or more positional parameters + one more distinguishing value = extract a config struct. This rule applies even if the existing function "looks fine" with seven args today; adding an eighth makes the call sites unreadable and invites the copy-and-modify shortcut at the next variant.

**Trigger 4 — parallel worktree seams.** (session history) When an implementation plan fans out into parallel worktree agents by file, be explicit in the plan about which functions are shared and name the exact helpers that will live in the shared module *before* the agents diverge. The March 10 plan named `INTERACTIVE_ROLES` and `is_collapsible` as shared but omitted `allocate_refs` — and the duplication followed from that omission.

**Trigger 5 — LOC-budget extraction.** When extracting code into a new module purely to fit a file-size limit, resist the urge to stop at the smallest extraction that clears the limit. Check whether the code that *stays behind* on one side now mirrors code on the other side. If it does, the LOC fix is also a DRY fix and both extractions belong together.

**Applies to review too.** (auto memory [claude]) Any two function bodies flagged by review tooling as "duplicated" get P1 severity minimum in this project. Low or P3 severity labels on duplication findings are wrong and should be escalated.

## Examples

### Before — two independent bodies, two files

**`crates/core/src/snapshot.rs`** (pre-`d06a6c2`, 7 positional params):

```rust
fn allocate_refs(
    mut node: AccessibilityNode,
    refmap: &mut RefMap,
    include_bounds: bool,
    interactive_only: bool,
    compact: bool,
    window_pid: i32,
    source_app: Option<&str>,
) -> AccessibilityNode {
    let is_interactive = INTERACTIVE_ROLES.contains(&node.role.as_str());

    if is_interactive {
        let entry = ref_entry_from_node(&node, window_pid, source_app, None);
        node.ref_id = Some(refmap.allocate(entry));
    }

    let has_label = node.name.as_deref().is_some_and(|n| !n.is_empty())
        || node.description.as_deref().is_some_and(|d| !d.is_empty());
    let is_skeleton_anchor = !is_interactive && node.children_count.is_some() && has_label;

    if is_skeleton_anchor {
        let mut entry = ref_entry_from_node(&node, window_pid, source_app, None);
        entry.available_actions = vec![];
        node.ref_id = Some(refmap.allocate(entry));
    }

    if !include_bounds { node.bounds = None; }

    node.children = node.children.into_iter()
        .filter_map(|child| {
            let child = allocate_refs(child, refmap, include_bounds,
                                      interactive_only, compact, window_pid, source_app);
            if compact && is_collapsible(&child) {
                return child.children.into_iter().next();
            }
            if interactive_only && child.ref_id.is_none()
                && child.children.is_empty() && child.children_count.is_none() {
                None
            } else { Some(child) }
        })
        .collect();
    node
}
```

**`crates/core/src/snapshot_ref.rs`** (pre-`d06a6c2`, local `DrillDownConfig`):

```rust
struct DrillDownConfig<'a> {
    include_bounds: bool,
    interactive_only: bool,
    compact: bool,
    pid: i32,
    source_app: Option<&'a str>,
    root_ref_id: &'a str,
}

fn allocate_refs_with_root(
    mut node: AccessibilityNode,
    refmap: &mut RefMap,
    config: &DrillDownConfig,
) -> AccessibilityNode {
    let is_interactive = INTERACTIVE_ROLES.contains(&node.role.as_str());

    if is_interactive {
        let entry = ref_entry_from_node(
            &node, config.pid, config.source_app,
            Some(config.root_ref_id.to_string()),   // <-- the only substantive difference
        );
        node.ref_id = Some(refmap.allocate(entry));
    }

    // ... 40+ more lines structurally identical to allocate_refs
}
```

The only substantive difference between the two bodies was `None` vs `Some(config.root_ref_id.to_string())` in exactly two call sites.

### After — one body, one file, two callers

**`crates/core/src/ref_alloc.rs`** (post-`d06a6c2`):

```rust
pub(crate) struct RefAllocConfig<'a> {
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
    pub pid: i32,
    pub source_app: Option<&'a str>,
    pub root_ref_id: Option<&'a str>, // None = full snapshot, Some(_) = drill-down
}

pub(crate) fn allocate_refs(
    mut node: AccessibilityNode,
    refmap: &mut RefMap,
    config: &RefAllocConfig,
) -> AccessibilityNode {
    let root_ref_owned = config.root_ref_id.map(str::to_string);
    let is_interactive = INTERACTIVE_ROLES.contains(&node.role.as_str());

    if is_interactive {
        let entry = ref_entry_from_node(&node, config.pid, config.source_app, root_ref_owned.clone());
        node.ref_id = Some(refmap.allocate(entry));
    }

    let has_label = node.name.as_deref().is_some_and(|n| !n.is_empty())
        || node.description.as_deref().is_some_and(|d| !d.is_empty());
    let is_skeleton_anchor = !is_interactive && node.children_count.is_some() && has_label;

    if is_skeleton_anchor {
        let mut entry = ref_entry_from_node(&node, config.pid, config.source_app, root_ref_owned);
        entry.available_actions = vec![];
        node.ref_id = Some(refmap.allocate(entry));
    }

    if !config.include_bounds { node.bounds = None; }

    node.children = node.children.into_iter()
        .filter_map(|child| {
            let child = allocate_refs(child, refmap, config);
            if config.compact && is_collapsible(&child) {
                return child.children.into_iter().next();
            }
            if config.interactive_only
                && child.ref_id.is_none()
                && child.children.is_empty()
                && child.children_count.is_none()
            {
                None
            } else {
                Some(child)
            }
        })
        .collect();

    node
}
```

**`crates/core/src/snapshot.rs`** caller:

```rust
let config = RefAllocConfig {
    include_bounds: opts.include_bounds,
    interactive_only: opts.interactive_only,
    compact: opts.compact,
    pid: window.pid,
    source_app: Some(window.app.as_str()),
    root_ref_id: None,
};
let mut tree = ref_alloc::allocate_refs(raw_tree, &mut refmap, &config);
```

**`crates/core/src/snapshot_ref.rs`** caller:

```rust
let config = RefAllocConfig {
    include_bounds: opts.include_bounds,
    interactive_only: opts.interactive_only,
    compact: opts.compact,
    pid: entry.pid,
    source_app,
    root_ref_id: Some(root_ref_id),
};
let mut tree = ref_alloc::allocate_refs(raw_tree, &mut refmap, &config);
```

`DrillDownConfig` is deleted. `allocate_refs_with_root` is deleted. A future change to `INTERACTIVE_ROLES`, the skeleton-anchor condition, the `is_collapsible` rule, or the `interactive_only` pruning touches exactly one place.

**Net diff** (`d06a6c2 refactor: unify allocate_refs across snapshot and drill-down paths`): +139 / −190 across `ref_alloc.rs`, `snapshot.rs`, `snapshot_ref.rs`. 51 net LOC removed. `grep allocate_refs_with_root` returns zero matches. The follow-up `7c7837a chore: drop with_root from drill test names after allocator unification` renamed the leftover `test_allocate_refs_with_root_*` helpers to `test_drill_alloc_*` so the name is gone from the codebase entirely.

## Related

- `crates/core/src/ref_alloc.rs` — single source of truth for ref allocation
- `crates/core/src/snapshot.rs` — full-snapshot caller
- `crates/core/src/snapshot_ref.rs` — drill-down caller
- Commit `d06a6c2` — the unifying refactor
- Commit `7c7837a` — test name cleanup
- `docs/plans/2026-03-10-feat-progressive-skeleton-traversal-plan.md` — original plan that split into parallel worktrees but did not name `allocate_refs` as a shared helper (moderate overlap but predates the duplication problem)
- `docs/brainstorms/2026-03-10-progressive-skeleton-traversal-brainstorm.md` — feature brainstorm; named `INTERACTIVE_ROLES` and `is_collapsible` as shared helpers but omitted the allocator body
