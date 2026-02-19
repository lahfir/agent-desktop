---
status: pending
priority: p3
issue_id: "020"
tags: [code-review, correctness, stability]
---

# bounds_hash Uses DefaultHasher (Unstable Across Rust Versions)

## Problem Statement

`bounds_hash()` in `node.rs` uses `std::collections::hash_map::DefaultHasher`. The DefaultHasher algorithm is not guaranteed to be stable across Rust versions — it can change between releases. If the hash changes after a Rust toolchain upgrade, every ref from old snapshots will appear stale, causing spurious `STALE_REF` errors for agents that persist refs across sessions.

## Findings

**File:** `crates/core/src/node.rs`

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn bounds_hash(bounds: Option<Bounds>) -> u64 {
    let mut hasher = DefaultHasher::new();
    bounds.hash(&mut hasher);
    hasher.finish()
}
```

The Rust documentation explicitly states: "The default hashing algorithm is currently SipHash 1-3, though this is subject to change at any point in the future."

## Proposed Solutions

### Option A: Use a stable, deterministic hasher (Recommended)
Replace with `FxHasher` (from `rustc-hash`) or `fnv::FnvHasher`. Both are stable, fast, and have no version-dependent behavior.
- **Effort:** Tiny (add one dependency)
- **Risk:** Low

### Option B: Compute bounds hash manually
XOR the bit representations of x, y, width, height as u64. No hasher needed.
```rust
fn bounds_hash(b: Option<Bounds>) -> u64 {
    let b = b.unwrap_or_default();
    (b.x.to_bits() as u64) ^ ((b.y.to_bits() as u64) << 16)
    ^ ((b.width.to_bits() as u64) << 32) ^ ((b.height.to_bits() as u64) << 48)
}
```
- **Effort:** Tiny
- **Risk:** Low — no dependency, deterministic

### Option C: Use AXIdentifier attribute instead of bounds
Prefer `kAXIdentifierAttribute` (stable accessibility ID set by app) over bounds. Falls back to bounds hash only when AXIdentifier is absent.
- **Effort:** Medium
- **Risk:** Low — better element identity

## Recommended Action

Option B: simple bit-XOR hash. No dependency, deterministic, clear intent. Option C as follow-up enhancement.

## Technical Details

- **File:** `crates/core/src/node.rs`
- **Component:** bounds_hash, RefEntry identity

## Acceptance Criteria

- [ ] `bounds_hash` produces identical output across Rust toolchain versions
- [ ] Existing test fixtures remain valid after the change

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
