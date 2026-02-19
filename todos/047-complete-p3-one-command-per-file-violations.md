---
status: pending
priority: p3
issue_id: "047"
tags: [code-review, naming, architecture]
---

# One-Command-Per-File Rule Violations (click.rs, clipboard.rs)

## Problem Statement

CLAUDE.md mandates "one command per file; filename matches the command name." Two files violate this:
1. `click.rs` contains three commands: `click`, `double-click`, `right-click`
2. `clipboard.rs` contains two commands: `clipboard-get`, `clipboard-set`

## Findings

**File:** `crates/core/src/commands/click.rs`
- `pub fn execute(...)` → `click`
- `pub fn execute_double(...)` → `double-click`
- `pub fn execute_right(...)` → `right-click`

**File:** `crates/core/src/commands/clipboard.rs`
- `pub fn execute_get(...)` → `clipboard-get`
- `pub fn execute_set(...)` → `clipboard-set`

Additionally, four files define `pub struct RefArgs { pub ref_id: String }` identically (`focus.rs`, `toggle.rs`, `expand.rs`, `collapse.rs`). This should be a shared type in `helpers.rs`.

## Proposed Solutions

### Option A: Split into separate files (Recommended)
Create `double_click.rs`, `right_click.rs`, `clipboard_get.rs`, `clipboard_set.rs`. Move shared `RefArgs` to `helpers.rs`.
- **Effort:** Small
- **Risk:** Low — mechanical rename, dispatch.rs updated to import from new files

### Option B: Accept as intentional grouping for closely-related variants
Document the rationale for grouping. The three click variants are truly trivial variations.
- **Effort:** Tiny
- **Risk:** None — but violates project rule

## Recommended Action

Option A: split per CLAUDE.md. The `RefArgs` shared type cleanup should be in the same PR.

## Technical Details

- **Files:** `crates/core/src/commands/click.rs`, `crates/core/src/commands/clipboard.rs`
- **Also:** `focus.rs`, `toggle.rs`, `expand.rs`, `collapse.rs` (duplicate RefArgs)

## Acceptance Criteria

- [ ] `double_click.rs`, `right_click.rs` exist as separate files
- [ ] `clipboard_get.rs`, `clipboard_set.rs` exist as separate files
- [ ] `RefArgs` is defined once in `helpers.rs` and imported by commands that need it
- [ ] All dispatch.rs references updated

## Work Log

- 2026-02-19: Finding identified by pattern-recognition-specialist agent
