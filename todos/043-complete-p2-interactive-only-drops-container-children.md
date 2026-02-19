---
status: pending
priority: p2
issue_id: "043"
tags: [code-review, correctness, snapshot]
---

# interactive_only Drops Non-Interactive Containers Before Recursing Into Children

## Problem Statement

When `--interactive-only` is true, the snapshot engine returns non-interactive nodes immediately without recursing into their children. Interactive elements nested inside non-interactive containers (e.g., buttons inside a toolbar group) never receive refs. The agent gets a tree with containers having children, but no actionable refs inside any container.

## Findings

**File:** `crates/core/src/snapshot.rs:92-106`

```rust
let is_interactive = INTERACTIVE_ROLES.contains(&node.role.as_str());

if is_interactive {
    node.ref_id = Some(refmap.allocate(entry));
} else if interactive_only {
    return node;  // Returns immediately — never recurses into node.children
}
```

A toolbar (`role: "group"`) containing 5 buttons: with `interactive_only=true`, the toolbar is returned with `return node` before the children are processed. All 5 buttons get no refs. An agent calling `snapshot --interactive-only` expecting to find refs for toolbar buttons receives empty results.

## Proposed Solutions

### Option A: Recurse into children before early return (Recommended)
Move the `interactive_only` check to filter the returned node from the tree, not skip recursion:
```rust
// Process children first regardless
let mut processed = allocate_refs_recursive(node, refmap, interactive_only);
// Then filter: if this node is non-interactive AND has no interactive descendants, skip it
if interactive_only && !is_interactive && !has_interactive_descendants(&processed) {
    return None;
}
```
- **Effort:** Medium
- **Risk:** Low

### Option B: Separate tree-building from ref-allocation
Build the full tree, allocate refs, then filter out non-interactive nodes in a post-pass. Children of filtered-out nodes bubble up to the parent.
- **Effort:** Medium
- **Risk:** Low — cleaner separation of concerns

### Option C: Don't filter nodes; only filter refs
Keep all nodes in the tree. The `interactive_only` flag only controls whether non-interactive nodes receive a ref_id. The tree structure is preserved but non-interactive nodes have no ref_id.
- **Effort:** Small
- **Risk:** Low — but tree output changes

## Recommended Action

Option C for a quick fix (minimal structural change). Option B for the correct long-term design.

## Technical Details

- **File:** `crates/core/src/snapshot.rs`
- **Lines:** 92–106
- **Component:** SnapshotEngine, `allocate_refs`

## Acceptance Criteria

- [ ] `snapshot --interactive-only` includes refs for buttons nested inside non-interactive groups
- [ ] The `interactive_only` flag does not prematurely skip tree traversal

## Work Log

- 2026-02-19: Finding identified by data-integrity-guardian review agent
