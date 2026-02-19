---
status: pending
priority: p1
issue_id: "009"
tags: [performance, code-review, macos, infinite-loop]
---

# resolve_element Lacks Cycle Detection (Potential Infinite Loop)

## Problem Statement

`resolve_element_impl` performs a full O(n) tree traversal on every action invocation, without any visited-set or cycle detection. If the AX tree has a cycle (possible in buggy or adversarial apps), the traversal will loop infinitely, hanging the agent-desktop process forever. Even without cycles, traversing a 10,000-node tree for every click is a severe performance bottleneck.

## Findings

**File:** `crates/macos/src/adapter.rs:131-194`

`find_element_recursive` walks the entire AX tree depth-first. Issues:

1. **No visited-set:** A cycle in the AX tree causes infinite recursion / stack overflow
2. **Full re-traversal per action:** Every `click`, `type`, etc. re-traverses the full tree
3. **O(n) per action:** For complex apps (n=10,000), this adds 2–5 seconds to every action

The design calls for a RefMap that stores `(pid, role, name, bounds_hash)` for fast re-identification, but the current implementation ignores the RefMap and does full traversal.

## Proposed Solutions

### Option A: Use RefMap for O(1) element lookup (Recommended)
On action, load the RefMap, look up the ref ID, then call `AXUIElementCreateApplication(pid)` + walk only to the known element using stored path/hash. Use `kAXRoleAttribute` + `kAXTitleAttribute` for liveness confirmation.
- **Effort:** Medium
- **Risk:** Low — RefMap already exists, just needs to be used for resolution

### Option B: Add visited-set to current traversal
Maintain a `HashSet<AXUIElementRef_ptr>` during traversal. Skip already-visited nodes.
- **Effort:** Small
- **Risk:** Low — prevents infinite loop; doesn't fix O(n) performance

### Option C: Index by (role, name, bounds_hash) at snapshot time
Build an in-memory HashMap at snapshot time. Resolution is O(1) hash lookup.
- **Effort:** Medium
- **Risk:** Medium — HashMap must be invalidated per-snapshot

## Recommended Action

Option B immediately (safety), Option A in same PR (performance). Both changes are in the same function.

## Technical Details

- **File:** `crates/macos/src/adapter.rs`
- **Lines:** 131–194
- **Component:** macOS adapter, `resolve_element_impl`, `find_element_recursive`

## Acceptance Criteria

- [ ] Traversal has visited-set preventing infinite loop on cyclic AX trees
- [ ] Element resolution does not re-traverse the full tree on every action call
- [ ] Action latency for click on any element < 100ms (excluding AX IPC)

## Work Log

- 2026-02-19: Finding identified by performance-oracle review agent
