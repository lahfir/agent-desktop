---
status: pending
priority: p2
issue_id: "017"
tags: [performance, code-review, serialization]
---

# Double-Serialization in snapshot.rs

## Problem Statement

`snapshot.rs` serializes the tree to a `serde_json::Value` first, then wraps it in another `json!` macro call, resulting in double serialization. The tree gets serialized twice — once to Value, once to the final response string. For a large tree (2000 nodes), this doubles the serialization CPU time and transiently allocates a full copy of the JSON tree.

## Findings

**File:** `crates/core/src/commands/snapshot.rs`

```rust
let tree_value = serde_json::to_value(&result.tree)?;  // First serialization
Ok(json!({                                               // Second serialization
    "app": result.app,
    "tree": tree_value,                                  // Re-serialized
    ...
}))
```

This pattern is unnecessary. The `json!` macro accepts any `Serialize` type directly via the `serde_json::to_value` path internally.

## Proposed Solutions

### Option A: Pass tree reference directly to json! macro (Recommended)
```rust
Ok(json!({
    "app": result.app,
    "tree": &result.tree,  // json! calls Serialize directly
    ...
}))
```
- **Effort:** Tiny
- **Risk:** Low

### Option B: Build a typed response struct
Define a `SnapshotResponse { app, tree, ref_count, window }` and derive `Serialize`. Return it directly.
- **Effort:** Small
- **Risk:** Low — cleaner, avoids json! macro entirely

## Recommended Action

Option A for immediate fix. Option B as a follow-up cleanup.

## Technical Details

- **File:** `crates/core/src/commands/snapshot.rs`
- **Component:** snapshot command serialization

## Acceptance Criteria

- [ ] Tree is serialized exactly once
- [ ] Snapshot response JSON is identical to current output
- [ ] No transient `serde_json::Value` clone of entire tree

## Work Log

- 2026-02-19: Finding identified by performance-oracle review agent
