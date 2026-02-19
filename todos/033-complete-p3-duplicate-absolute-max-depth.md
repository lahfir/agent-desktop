---
status: pending
priority: p3
issue_id: "033"
tags: [code-review, architecture, duplication]
---

# ABSOLUTE_MAX_DEPTH Constant Duplicated in core and macos

## Problem Statement

`ABSOLUTE_MAX_DEPTH = 50` is defined in both `crates/core/src/snapshot.rs` and `crates/macos/src/tree.rs`. If the two values diverge (during debugging or tuning), the limits enforce independently and silently.

## Findings

**File 1:** `crates/core/src/snapshot.rs:13` — `const ABSOLUTE_MAX_DEPTH: u8 = 50;`
**File 2:** `crates/macos/src/tree.rs:4` — `const ABSOLUTE_MAX_DEPTH: u8 = 50;`

The platform crate constant is the actual enforced limit during tree traversal. The core constant is used for the depth-cap validation. If macos is set to 30 but core is set to 50, the core validation would accept max_depth=45 which the macOS traversal would then silently cap at 30.

## Proposed Solutions

### Option A: Define in core, re-export for platform use (Recommended)
Keep in `core`, export as `pub const ABSOLUTE_MAX_DEPTH`. Platform crates import from core.
- **Effort:** Tiny
- **Risk:** Low

### Option B: Use a shared constants module
Move to `core/src/constants.rs` and re-export through `lib.rs`.
- **Effort:** Tiny
- **Risk:** Low

## Recommended Action

Option A: single definition in core, import in macos tree.rs.

## Technical Details

- **Files:** `crates/core/src/snapshot.rs:13`, `crates/macos/src/tree.rs:4`

## Acceptance Criteria

- [ ] `ABSOLUTE_MAX_DEPTH` has exactly one definition
- [ ] Changing the value in one place updates both behaviors

## Work Log

- 2026-02-19: Finding identified by architecture-strategist review agent
