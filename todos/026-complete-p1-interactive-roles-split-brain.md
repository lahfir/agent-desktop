---
status: pending
priority: p1
issue_id: "026"
tags: [code-review, architecture, correctness]
---

# Duplicate INTERACTIVE_ROLES Definition — Split-Brain Role Classification

## Problem Statement

The list of interactive roles (which elements receive `@ref` IDs) is defined in two separate places that can drift independently. Adding a new role to one but not the other causes inconsistency: elements get refs without being counted as interactive, or vice versa.

## Findings

**File 1:** `crates/core/src/snapshot.rs:8-11`
```rust
const INTERACTIVE_ROLES: &[&str] = &[
    "button", "textfield", "checkbox", "link", ...
];
```

**File 2:** `crates/macos/src/roles.rs:36-52`
```rust
pub fn is_interactive_role(role: &str) -> bool {
    matches!(role, "button" | "textfield" | "checkbox" | "link" | ...)
}
```

`is_interactive_role` in `roles.rs` is exported but never called anywhere in the codebase. The two lists are currently identical but have no compile-time guarantee of staying in sync.

## Proposed Solutions

### Option A: Single definition in core, import in platform crates (Recommended)
Keep `INTERACTIVE_ROLES` in `core/src/snapshot.rs` (or move to `core/src/roles.rs`). Delete `is_interactive_role` from `macos/src/roles.rs`. Platform crates that need it import from core.
- **Effort:** Small
- **Risk:** Low

### Option B: Generate from a shared macro
Define the role list once as a macro that produces both the slice and the match pattern. Compile-time enforcement.
- **Effort:** Medium
- **Risk:** Low — over-engineering for this size

## Recommended Action

Option A: delete `is_interactive_role` from macos crate, use `INTERACTIVE_ROLES` from core everywhere.

## Technical Details

- **Files:** `crates/core/src/snapshot.rs:8-11`, `crates/macos/src/roles.rs:36-52`
- **Component:** ref allocation, interactive role classification

## Acceptance Criteria

- [ ] Interactive role list has exactly one definition
- [ ] `is_interactive_role` in macos roles.rs is removed
- [ ] Adding a new role requires changing exactly one file

## Work Log

- 2026-02-19: Finding identified by architecture-strategist review agent
