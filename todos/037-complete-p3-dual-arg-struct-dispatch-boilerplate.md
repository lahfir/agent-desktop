---
status: pending
priority: p3
issue_id: "037"
tags: [code-review, simplicity, architecture]
---

# Dual Arg Struct Pattern — dispatch.rs Is 194 LOC of Field-Copying Boilerplate

## Problem Statement

Every command has two parallel arg structs: one in `cli.rs` (with clap derive) and one in `commands/*.rs` (plain Rust). `dispatch.rs` exists entirely to copy fields between them. The intermediate `commands::*Args` structs add ~80-100 LOC of dead weight with no transformation logic.

## Findings

**File:** `src/dispatch.rs` — 194 LOC, every arm is:
```rust
Commands::Snapshot(a) => snapshot::execute(
    snapshot::SnapshotArgs {
        app: a.app,
        window_id: a.window_id,
        max_depth: a.max_depth,
        include_bounds: a.include_bounds,
        interactive_only: a.interactive_only,
        compact: a.compact,
    },
    adapter,
),
```

This pattern repeats for every one of 31 commands. The three `parse_*` helpers (`parse_direction`, `parse_get_property`, `parse_is_property`) add minor value; everything else is pure field-copying.

The `commands::*Args` structs are used only inside their `execute()` functions. If `execute()` accepted the cli arg struct directly, the entire dispatch file shrinks to a 60-line straight match.

## Proposed Solutions

### Option A: Remove commands::*Args; use cli structs directly
Move clap-annotated structs from `cli.rs` to `crates/core/src/commands/*.rs` (or a shared `args.rs`). `dispatch.rs` becomes one-liners.
- **Effort:** Large (touches all 31 commands)
- **Risk:** Medium — introduces clap into core, which the current design intentionally avoids

### Option B: Keep separation; add From impls (Recommended)
Add `impl From<cli::SnapshotArgs> for commands::snapshot::SnapshotArgs` for each command. Dispatch arms become `snapshot::execute(a.into(), adapter)`. Eliminates the field-copy boilerplate while preserving the CLI/core boundary.
- **Effort:** Medium
- **Risk:** Low — clean, no architecture change required

### Option C: Accept current pattern as intentional
Document that the dual-struct pattern is deliberate decoupling. The cost is 80-100 LOC but the benefit is zero clap dependency in core.
- **Effort:** Tiny
- **Risk:** None — defer to Phase 3 cleanup

## Recommended Action

Option C for now (document). Option B in a dedicated cleanup PR when the dispatch file grows further. The pattern is intentional but the `From` impl approach would be the right long-term solution.

## Technical Details

- **File:** `src/dispatch.rs` — 194 LOC
- **Component:** command dispatch layer

## Acceptance Criteria

- [ ] Either From impls exist for all command arg structs, OR
- [ ] A comment in dispatch.rs explains the intentional decoupling pattern

## Work Log

- 2026-02-19: Finding identified by code-simplicity-reviewer agent
