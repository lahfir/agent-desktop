---
title: "feat: implement --compact flag for tree chain collapsing"
type: feat
status: completed
date: 2026-03-01
origin: docs/brainstorms/2026-03-01-electron-tree-compaction-brainstorm.md
---

# feat: implement --compact flag for tree chain collapsing

## Overview

The `--compact` flag is fully wired through CLI → TreeOptions → snapshot pipeline but is a no-op. Implement it to collapse single-child non-interactive pass-through nodes, reducing structural noise in the JSON tree by ~15% tokens. Primary beneficiary: Electron/web apps (Slack, VS Code) where HTML `<div>` wrappers create deep group chains.

(see brainstorm: docs/brainstorms/2026-03-01-electron-tree-compaction-brainstorm.md)

## Collapse Rule

A node is collapsible when ALL of these are true:
- `ref_id` is `None` (not interactive)
- `name` is `None` (unnamed)
- `value` is `None` (no value)
- `description` is `None` (no description)
- `states` is empty (no semantic state like "disabled")
- Has exactly **1 child**
- Is **not the root node**

When a node is collapsible, it is removed and its single child is hoisted to take its place. Cascading is natural: bottom-up recursion means inner collapses happen first, potentially making outer nodes collapsible too.

**Deliberately excluded: name promotion.** The brainstorm considered promoting a wrapper's name to a nameless child, but this risks contaminating `RefEntry.name` and causing `STALE_REF` on re-identification. The 3% additional savings is not worth the correctness risk.

## Acceptance Criteria

- [x] `--compact` collapses single-child unnamed pass-through nodes in snapshot output
- [x] All interactive refs preserved (zero information loss)
- [x] Root node never collapsed
- [x] Nodes with `description` or non-empty `states` are never collapsed
- [x] Cascading collapse works (group > group > group > button → button)
- [x] Works correctly when combined with `--interactive-only`
- [x] Works correctly in batch mode (`{"command": "snapshot", "args": {"compact": true}}`)
- [x] `--compact` alone (without `-i`) also works
- [x] Clippy clean, fmt clean, all existing tests pass
- [x] New unit tests cover: cascading, description preservation, states preservation, compact+interactive_only

## Implementation

### `crates/core/src/snapshot.rs`

**1. Add helper predicate** (~10 LOC):

```rust
fn is_collapsible(node: &AccessibilityNode) -> bool {
    node.ref_id.is_none()
        && node.name.is_none()
        && node.value.is_none()
        && node.description.is_none()
        && node.states.is_empty()
        && node.children.len() == 1
}
```

**2. Add `compact: bool` param to `allocate_refs`** — extend signature alongside existing `interactive_only`:

```rust
fn allocate_refs(
    mut node: AccessibilityNode,
    refmap: &mut RefMap,
    include_bounds: bool,
    interactive_only: bool,
    compact: bool,              // new
    window_pid: i32,
    source_app: Option<&str>,
) -> AccessibilityNode
```

**3. Add compact logic in the existing `filter_map`** — after children are recursed, before the interactive_only check:

```rust
node.children = node
    .children
    .into_iter()
    .filter_map(|child| {
        let child = allocate_refs(child, refmap, include_bounds, interactive_only, compact, ...);

        // Compact: hoist single child of unnamed pass-through containers
        if compact && is_collapsible(&child) {
            return child.children.into_iter().next();
        }

        // Interactive-only: prune non-interactive leaves
        if interactive_only && child.ref_id.is_none() && child.children.is_empty() {
            return None;
        }

        Some(child)
    })
    .collect();
```

Order: compact fires first (hoists child), then interactive_only may prune the hoisted child if it's a non-interactive leaf. This is correct.

**4. Thread `compact` through the call site** in `build()`:

```rust
let mut tree = allocate_refs(raw_tree, &mut refmap, opts.include_bounds, opts.interactive_only, opts.compact, ...);
```

**5. Update tracing log** in `snapshot::execute()` to include `compact={}`.

### `src/cli_args.rs`

**6. Update help text** from "Omit empty structural nodes from output" to "Collapse single-child unnamed nodes to reduce tree depth":

```rust
#[arg(long, help = "Collapse single-child unnamed nodes to reduce tree depth")]
pub compact: bool,
```

### Tests — `crates/core/src/snapshot.rs` (unit tests)

**7. Add tests** (~60 LOC):

- `test_compact_collapses_single_child_chain` — group > group > group > button → button
- `test_compact_preserves_named_containers` — group("Sidebar") > button stays
- `test_compact_preserves_description` — group(desc="toolbar") > button stays
- `test_compact_preserves_states` — group(states=["disabled"]) > button stays
- `test_compact_preserves_multi_child` — group > (button + textfield) stays
- `test_compact_with_interactive_only` — both flags together work correctly

## Files Changed

| File | Change |
|------|--------|
| `crates/core/src/snapshot.rs` | Add `is_collapsible`, add `compact` param to `allocate_refs`, compact logic in filter_map, thread through call site, unit tests |
| `src/cli_args.rs` | Update `--compact` help text |

## Not Changing

- `append_surface_refs` — already uses `interactive_only: true`, compact adds negligible value for surface overlays
- `node.rs` — no struct changes needed
- `hints.rs` — runs after compact, indexes are correct post-collapse
- macOS adapter (`builder.rs`) — compact is a core-level tree transform, not platform-level

## Sources

- **Origin brainstorm:** [docs/brainstorms/2026-03-01-electron-tree-compaction-brainstorm.md](../brainstorms/2026-03-01-electron-tree-compaction-brainstorm.md) — key decisions: no separate bridge package, chain collapsing over flattening, universal optimization not Electron-specific
- Existing pattern: `interactive_only` filter in `snapshot.rs:191`
- Existing pattern: `add_structural_hints` in `hints.rs` (post-processing pass)
