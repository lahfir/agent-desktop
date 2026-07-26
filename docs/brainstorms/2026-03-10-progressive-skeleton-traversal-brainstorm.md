# Progressive Skeleton Traversal

**Date:** 2026-03-10
**Status:** Brainstorm
**Author:** Lahfir

## What We're Building

A two-phase tree traversal system that lets AI agents explore dense accessibility trees incrementally instead of consuming the entire tree at once. The system introduces:

1. **Skeleton mode** — A shallow overview snapshot (2-3 levels deep) where truncated nodes show `children_count` instead of full subtrees. Named containers at the truncation boundary receive refs so agents can drill into them.

2. **Ref-rooted subtree** — A `--root @ref` flag on `snapshot` that starts traversal from a previously-discovered element instead of the window root. Combined with refmap merging, this lets agents accumulate knowledge across multiple targeted drill-downs.

3. **Epoch-tagged refmap merging** — Instead of replacing the entire refmap on each snapshot, drill-down snapshots merge new refs into the existing map. A monotonic epoch counter and root-ref tagging enable staleness detection and scoped invalidation.

### The Problem

Dense Electron apps (Slack, VS Code, Discord) produce massive accessibility trees — 500+ nodes, 12,000+ tokens per full snapshot. Even with `--interactive-only --compact`, Slack still produces ~200 nodes (~5,400 tokens). Agents pay per-token and struggle to navigate these trees efficiently.

Current optimization layers (depth-skip, compact, surfaces, interactive-only) reduce output size but still require full tree traversal. There's no way to say "I only care about the sidebar right now."

### Target Workflow

```bash
# Step 1: Cheap overview of the entire app (~500 tokens)
agent-desktop snapshot --app Slack --skeleton -i
# Returns 2-3 levels with children_count on truncated containers

# Step 2: Agent identifies sidebar (@e3) as interesting, drills in (~800 tokens)
agent-desktop snapshot --root @e3 -i --compact
# Returns @e3's full subtree, merges refs into existing refmap

# Step 3: Agent identifies a specific channel list (@e12), drills deeper (~400 tokens)
agent-desktop snapshot --root @e12 -i --compact
# Returns @e12's subtree, merges again

# Step 4: Agent acts on a discovered element
agent-desktop click @e45
# Uses ref from step 3 — still valid because refmap accumulated

# Total: ~1,700 tokens across 3 snapshots vs ~5,400 tokens for one full snapshot
# Plus: agent only processed relevant information at each step
```

## Why This Approach

### Why not Semantic Zone Auto-Detection?

Zone detection (auto-labeling "sidebar", "toolbar", "content" via role heuristics) is appealing but fragile:
- Not all apps use standard accessibility roles for layout
- Platform differences in role semantics make cross-platform heuristics unreliable
- Introduces a new concept (zones) alongside existing refs
- Can be layered on top of Progressive Skeleton later as sugar

### Why not Query-Driven Traversal?

Targeted search (`find --near "Channels"`) is token-efficient but requires agents to know what they're looking for. It fails at the "orient myself in an unfamiliar app" use case — which is the most common first step.

### Why Progressive Skeleton wins

1. **Composable** — Works with ALL existing flags (`--compact`, `--interactive-only`, `--max-depth`, `--surface`)
2. **Platform-agnostic** — Just new `TreeOptions` fields; each adapter implements the same interface
3. **Agent-controlled** — Agents decide exploration strategy based on their goals
4. **Familiar pattern** — Mirrors tree exploration in file systems (expand on demand)
5. **Minimal API surface** — Two new flags (`--skeleton`, `--root`), one new refmap behavior (merge)
6. **Incremental value** — Each feature is independently useful; skeleton without drill-down still saves tokens

## Key Decisions

### 1. Skeleton Output Format

Truncated nodes show `children_count` (direct child count) instead of full children arrays:

```json
{
  "role": "group",
  "name": "Channels",
  "ref": "@e3",
  "children_count": 47
}
```

**Rationale:** Child count is O(1) to fetch on all platforms (it's a property of the element, not requiring subtree traversal). Gives agents enough signal to prioritize drill-downs. More detailed stats (interactive count, role distribution) would require full traversal, defeating the purpose.

### 2. Skeleton Ref Assignment

At the skeleton boundary, **named containers with children** receive refs even if they're non-interactive. These are "drill-down targets" — the agent uses them with `--root @ref` to explore deeper.

Rules for skeleton ref assignment:
- Interactive elements: always get refs (existing behavior)
- Named containers at truncation boundary with `children_count > 0`: get refs (NEW)
- Anonymous wrappers (no name, no value): never get refs (existing behavior)

### 3. RefMap Merge Strategy

Each snapshot call increments a monotonic `epoch` counter stored in the refmap file. RefEntries are tagged with:

- `epoch: u32` — when this ref was created
- `root_ref: Option<String>` — which drill-down root created this ref (`None` for skeleton/full snapshots)

**Merge behavior:**
- **Full/skeleton snapshot** (`--skeleton` or no `--root`): replaces entire refmap (current behavior). Resets epoch.
- **Drill-down** (`--root @e3`): loads existing refmap, removes all refs where `root_ref == "@e3"` (clear stale subtree), increments epoch, adds new refs with `root_ref: "@e3"`, saves merged refmap.

**Staleness detection:**
- On `STALE_REF`, error includes epoch info for recovery hints
- Agent can compare epochs across refs to detect relative freshness
- Re-drilling a region automatically invalidates only that region's refs

### 4. `--root` Implies Merge

When `--root @ref` is used, merge mode is automatic — there's no separate `--merge` flag. Rationale: drilling into a subtree without merging would discard all other refs, which is never what an agent wants.

### 5. Platform-Agnostic Design

New `PlatformAdapter` trait method:

```rust
fn get_subtree(&self, handle: &NativeHandle, opts: &TreeOptions) -> Result<AccessibilityNode, AdapterError> {
    Err(AdapterError::not_supported("get_subtree"))
}
```

Each platform adapter implements this by:
- macOS: extract `AXUIElementRef` from handle, call `build_subtree()` from that element
- Windows (Phase 2): extract `IUIAutomationElement` from handle, walk with TreeWalker from that element
- Linux (Phase 3): extract AT-SPI accessible from handle, walk from that element

The `TreeOptions` struct gains:
```rust
pub struct TreeOptions {
    pub max_depth: u8,
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
    pub surface: SnapshotSurface,
    pub skeleton: bool,           // NEW: shallow overview with children_count
    pub root_ref: Option<String>, // NEW: start from this ref instead of window root
}
```

### 6. Skeleton Depth

Skeleton mode sets an internal depth limit independent of `--max-depth`. Default skeleton depth: **3 levels** from the root (or from `--root` element).

- Level 0: Root element (window or ref target)
- Level 1: Major sections (toolbar, sidebar, content)
- Level 2: Sub-sections (channel list, message list)
- Level 3: Individual groups/items (truncated here with `children_count`)

Agent can override: `--skeleton --max-depth 2` for even shallower overview.

## Token Budget Analysis

Based on real Slack data (528 nodes full tree):

| Mode | Nodes | Est. Tokens | Savings vs Full |
|------|-------|-------------|-----------------|
| Full snapshot | 528 | ~12,000 | baseline |
| + interactive-only | 268 | ~6,700 | 44% |
| + compact | 204 | ~5,400 | 55% |
| **Skeleton (depth 3)** | ~25-40 | ~500-800 | **93-96%** |
| Drill-down (sidebar) | ~30-60 | ~600-1,200 | scoped |
| Drill-down (content) | ~50-100 | ~1,000-2,000 | scoped |
| **Skeleton + 1 drill-down** | ~55-100 | ~1,100-2,000 | **83-91%** |
| **Skeleton + 2 drill-downs** | ~85-160 | ~1,700-3,200 | **73-86%** |

Even with 2 targeted drill-downs, agents use 1,700-3,200 tokens instead of 5,400. And they only process relevant information at each step, improving reasoning quality.

## Implementation Scope

### Core Changes
- `TreeOptions`: add `skeleton: bool`, `root_ref: Option<String>`
- `RefMap` / `RefEntry`: add `epoch: u32`, `root_ref: Option<String>` fields
- `snapshot.rs`: skeleton mode in `allocate_refs()` (assign refs to named containers, replace children with `children_count`)
- `snapshot.rs`: merge logic in `run()` (load existing refmap, scoped invalidation, merge)
- `AccessibilityNode`: add `children_count: Option<u32>` field with serde skip-if-none
- `PlatformAdapter`: add `get_subtree()` method with default `not_supported`

### macOS Changes
- `adapter.rs`: implement `get_subtree()` — resolve handle to AXElement, call `build_subtree()`
- `builder.rs`: support `skeleton` flag in `build_subtree()` — at depth limit, count children instead of recursing

### CLI Changes
- `cli_args.rs`: add `--skeleton` and `--root` flags to `SnapshotArgs`
- `dispatch.rs`: pass new args through to `TreeOptions`

### find Command Changes
- `find.rs`: add `--root @ref` support — resolve ref, build subtree from it, then search in-memory
- `cli_args.rs`: add `--root` flag to `FindArgs`

### No Changes Needed
- `resolve.rs` — existing resolution logic works as-is
- `get.rs` — works with merged refmaps transparently
- All action commands — work with any valid ref from the merged map
- `compact`, `interactive-only` — compose naturally with skeleton/drill-down

## Resolved Questions

1. **Should `find` support `--root @ref` too?** **Yes.** Agents can search within a drill-down region: `find --root @e3 --role button`. Low effort since find already operates on in-memory trees — just change where the tree comes from.

2. **What happens when the skeleton root ref itself is stale?** **Return STALE_REF, no auto-recovery.** The suggestion tells the agent to re-run skeleton. Auto-fallback hides UI state changes the agent should know about. Agents already handle STALE_REF.

3. **Should `children_count` count ALL descendants or just direct children?** **Direct children only.** O(1) on all platforms — it's an attribute of the element. Agent sees "this group has 47 direct children" which is sufficient to judge density. Total descendant count would require subtree traversal, defeating the purpose.

4. **Naming: `--skeleton` vs `--overview` vs `--shallow`?** **`--skeleton`.** Most descriptive — "shows structure without flesh." Technical but precise. Matches the pattern of other flags.

5. **Should skeleton be the default when `--compact --interactive-only` are both set?** **No, always opt-in.** Agents explicitly choose skeleton mode. Existing behavior unchanged. No surprise breakage for agents already using snapshot.
