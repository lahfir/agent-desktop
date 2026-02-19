---
status: pending
priority: p2
issue_id: "038"
tags: [code-review, correctness, zero-unwrap]
---

# unwrap() Violations — Breaks Project Zero-unwrap Rule

## Problem Statement

Two locations in non-test code call `unwrap()`, violating the explicit project rule from CLAUDE.md: "Zero `unwrap()` in non-test code." Both are technically safe due to preceding guards, but they create fragile implicit invariants that future refactors can break.

## Findings

### wait.rs — Double get() + unwrap
**File:** `crates/core/src/commands/wait.rs:47-48`

```rust
if refmap.get(&ref_id).is_some()
    && adapter.resolve_element(refmap.get(&ref_id).unwrap()).is_ok()
```

`get()` is called twice; the second call `unwrap()`s under an assumption that the first check guarantees `Some`. In a Phase 4 daemon scenario where the refmap is hot-reloaded, the entry could be removed between the two calls. Fix:
```rust
if let Some(entry) = refmap.get(&ref_id) {
    if adapter.resolve_element(entry).is_ok() { ... }
}
```

### press.rs — unwrap() after is_empty guard
**File:** `crates/core/src/commands/press.rs:41`

```rust
let key = parts.last().unwrap();
```

An `is_empty()` check a few lines earlier makes this safe in current code. But `unwrap()` in non-test code is forbidden by the project rules. Fix:
```rust
let Some(key) = parts.last() else {
    return Err(AppError::invalid_input("Empty key combo"));
};
```

## Proposed Solutions

### Option A: Fix both with idiomatic pattern matching (Recommended)
Apply the `if let` and `let...else` patterns shown above.
- **Effort:** Tiny
- **Risk:** Low

## Recommended Action

Option A: immediate fix, two lines changed.

## Technical Details

- **Files:** `crates/core/src/commands/wait.rs:47-48`, `crates/core/src/commands/press.rs:41`

## Acceptance Criteria

- [ ] Zero `unwrap()` calls in non-test code
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] Behavior is identical; no functional change

## Work Log

- 2026-02-19: Finding identified by code-simplicity-reviewer agent
