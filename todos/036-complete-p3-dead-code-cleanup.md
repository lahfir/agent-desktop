---
status: pending
priority: p3
issue_id: "036"
tags: [code-review, dead-code, simplicity]
---

# Dead Code Cleanup — Multiple Verified Unused Items

## Problem Statement

Several symbols were added for anticipated use cases that never materialized. All confirmed dead by code search. Estimated ~175 LOC that can be deleted with no functional change.

## Findings

### output.rs — Entire file is unused (92 LOC)
`crates/core/src/output.rs` — `Response`, `ErrorPayload`, `AppContext`, `WindowContext` are never instantiated. `main.rs` builds JSON envelopes inline with `serde_json::json!()`. The re-export in `lib.rs:17` is also dead.

### focused_window() — Never called (12 LOC)
`crates/core/src/adapter.rs:151-153` and `crates/macos/src/adapter.rs:99-103` — `focused_window()` is in the trait and implemented in macOS, but no command, no dispatch arm, and no test ever calls it. The snapshot engine resolves focus via `is_focused` filtering on `list_windows()`.

### SourceError struct — Source field always None (5 LOC)
`crates/core/src/error.rs:31-33` — `SourceError` is a wrapper type for the `#[source]` annotation on `AdapterError`. But `AdapterError.source: Option<Box<SourceError>>` is always `None` — no code ever constructs a `SourceError`.

### RefEntry.source_app — Always None (never set)
`crates/core/src/refs.rs` — `source_app: Option<String>` in `RefEntry` is always `None` at construction and never read by any code path.

### VersionArgs.json: bool — Meaningless flag (6 LOC)
`crates/core/src/commands/version.rs:4-14` — `args.json` is accepted but ignored. The tool exclusively produces JSON for every command; a `--json` mode flag is meaningless.

### is_interactive_role() — Never called (17 LOC)
`crates/macos/src/roles.rs:36-52` — Duplicates the `INTERACTIVE_ROLES` constant in core. Already tracked in issue 026 but worth deleting in the same PR.

## Proposed Solutions

### Option A: Delete all dead code in one PR (Recommended)
Delete `output.rs`, `focused_window()` (trait + impl), `SourceError`, `source_app`, `--json` from version, `is_interactive_role`. One PR, ~175 LOC removed.
- **Effort:** Small
- **Risk:** Low — all confirmed dead by grep with zero callers

## Recommended Action

Option A: batch deletion. Run `cargo test` and `cargo clippy` after to confirm no regressions.

## Technical Details

| Item | File | LOC |
|------|------|-----|
| output.rs (whole file) | crates/core/src/output.rs | 92 |
| lib.rs re-export | crates/core/src/lib.rs:17 | 1 |
| focused_window() trait | crates/core/src/adapter.rs:151-153 | 3 |
| focused_window() macOS | crates/macos/src/adapter.rs:99-103 | 5 |
| SourceError | crates/core/src/error.rs:31-33 | 3 |
| AdapterError.source | crates/core/src/error.rs:25-26 | 2 |
| RefEntry.source_app | crates/core/src/refs.rs | 2 |
| VersionArgs.json | crates/core/src/commands/version.rs | 6 |
| is_interactive_role | crates/macos/src/roles.rs:36-52 | 17 |

## Acceptance Criteria

- [ ] All listed items are removed
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes

## Work Log

- 2026-02-19: Finding identified by code-simplicity-reviewer agent
