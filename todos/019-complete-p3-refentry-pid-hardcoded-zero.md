---
status: pending
priority: p3
issue_id: "019"
tags: [code-review, correctness]
---

# RefEntry.pid Hardcoded to 0 in Snapshot

## Problem Statement

`RefEntry` in the snapshot output has `pid` hardcoded to `0`. When `resolve_element_impl` looks up an element by ref, it calls `AXUIElementCreateApplication(entry.pid)`, which with `pid=0` targets the kernel — a nonsensical lookup that will always fail or behave unexpectedly.

## Findings

**File:** `crates/core/src/snapshot.rs:95-102`

```rust
RefEntry {
    pid: 0,  // Should be the actual app PID
    role: node.role.clone(),
    name: node.name.clone().unwrap_or_default(),
    bounds_hash: compute_bounds_hash(node.bounds),
    available_actions: vec![],
}
```

The `pid` should be the PID of the application window from which this node was captured, available from `WindowInfo.pid`.

## Proposed Solutions

### Option A: Pass WindowInfo.pid through SnapshotEngine to RefEntry construction
Thread the `pid` from the snapshot call through to `RefEntry` creation. Minor refactor.
- **Effort:** Small
- **Risk:** Low

### Option B: Store pid in SnapshotResult and populate in allocate_refs
Have `SnapshotEngine` carry the `pid` from the `WindowInfo` used for the snapshot, then pass it to each `RefEntry`.
- **Effort:** Small
- **Risk:** Low

## Recommended Action

Option B: `SnapshotEngine` holds the source `pid`, populates all `RefEntry` instances with it.

## Technical Details

- **File:** `crates/core/src/snapshot.rs`
- **Lines:** 95–102
- **Component:** SnapshotEngine, RefEntry allocation

## Acceptance Criteria

- [ ] `RefEntry.pid` contains the actual process PID
- [ ] `pid=0` never appears in a saved RefMap
- [ ] Element resolution uses correct PID for `AXUIElementCreateApplication`

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
