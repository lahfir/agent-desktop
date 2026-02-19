---
status: pending
priority: p1
issue_id: "022"
tags: [code-review, correctness, architecture]
---

# Batch Command Does Not Dispatch Sub-Commands

## Problem Statement

The `batch` command parses its JSON input and counts the commands, but never dispatches them. It returns a note saying "dispatch is in the binary crate" — but `src/dispatch.rs` routes `Commands::Batch` to `batch::execute`, which does not call back into dispatch. The batch command is completely non-functional.

## Findings

**File:** `crates/core/src/commands/batch.rs:18-26`

```rust
pub fn execute(args: BatchArgs, _adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let commands: Vec<BatchCommand> = serde_json::from_str(&args.commands_json)...;
    Ok(json!({
        "note": "Batch dispatch is implemented in the binary crate dispatch layer",
        "count": commands.len()
    }))
}
```

**File:** `src/dispatch.rs:146-152` — routes `Commands::Batch` to `batch::execute`, which does the above. No sub-commands are executed.

The `BatchCommand` fields (`command`, `args`) are never read beyond deserialization, which is why clippy flagged them as `dead_code` (commit 608d4aa). The `#[allow(dead_code)]` suppressor was applied to silence the symptom rather than fix the underlying issue.

## Proposed Solutions

### Option A: Implement batch dispatch in the binary crate (Recommended)
Move batch execution logic to the binary crate where `dispatch()` is accessible. Parse each `BatchCommand`, construct the corresponding `Commands` enum variant, and call `dispatch()` recursively for each. Return an array of results.
- **Effort:** Medium
- **Risk:** Low

### Option B: Implement batch dispatch in core via a callback
Pass a `dispatch_fn: &dyn Fn(Command, &dyn PlatformAdapter) -> Result<Value, AppError>` closure into `batch::execute`. Keeps logic in core.
- **Effort:** Medium
- **Risk:** Low — but requires changing execute() signature

### Option C: Document batch as Phase 2 and return clear NOT_IMPLEMENTED error
If batch is intentionally deferred, return `AppError` with `ErrorCode::NotImplemented` instead of a misleading success response with a note.
- **Effort:** Tiny
- **Risk:** Low — honest API response

## Recommended Action

Option A: implement in the binary crate. The `dispatch` function is already there; batch just needs to parse and call it for each sub-command. Remove the `#[allow(dead_code)]` suppressor once fields are used.

## Technical Details

- **Files:** `crates/core/src/commands/batch.rs`, `src/dispatch.rs`
- **Component:** batch command, binary dispatch

## Acceptance Criteria

- [ ] `batch` with a JSON array of commands executes each sub-command in order
- [ ] `batch` with `stop_on_error: true` halts on first failure
- [ ] Response includes per-command results array
- [ ] `#[allow(dead_code)]` on BatchCommand is removed

## Work Log

- 2026-02-19: Finding identified by git-history-analyzer agent
