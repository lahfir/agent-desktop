---
status: pending
priority: p2
issue_id: "030"
tags: [code-review, architecture, correctness]
---

# find Command Always Runs Full Snapshot, Silently Overwrites RefMap

## Problem Statement

The `find` command unconditionally runs a full snapshot, which as a side-effect replaces `~/.agent-desktop/last_refmap.json`. An agent calling `find` mid-workflow invalidates all refs it was using without any warning. The command also uses default snapshot options (`max_depth=10`) regardless of the current context.

## Findings

**File:** `crates/core/src/commands/find.rs:12-13`

```rust
let opts = crate::adapter::TreeOptions::default();
let result = snapshot::run(adapter, &opts, args.app.as_deref(), None)?;
```

An agent workflow: `snapshot → click @e3 → find "submit" → click @e3` — the second click will fail because `find` replaced the refmap, making `@e3` stale. The agent has no way to know this happened.

## Proposed Solutions

### Option A: Make find non-destructive — use a temporary refmap (Recommended)
Run the snapshot for find into a temp `RefMap` in memory. Do not write to disk. Return matching elements with their refs. The agent's current refmap is preserved.
- **Effort:** Medium
- **Risk:** Low

### Option B: Accept existing refmap for find
Add a `--no-snapshot` flag: skip the snapshot, search the existing refmap entries for matching text. Only re-snapshot if element not found.
- **Effort:** Small
- **Risk:** Medium — may miss newly appeared elements

### Option C: Document the side-effect
Add a clear warning in the JSON response: `"warning": "refmap was replaced; previous refs are now stale"`.
- **Effort:** Tiny
- **Risk:** None — but doesn't fix the underlying issue

## Recommended Action

Option C immediately (document), Option A in a follow-up. The side-effect is architectural; full fix needs design thought.

## Technical Details

- **File:** `crates/core/src/commands/find.rs`
- **Lines:** 12–13
- **Component:** find command, snapshot interaction

## Acceptance Criteria

- [ ] `find` side-effect (refmap replacement) is documented in JSON response
- [ ] Longer term: `find` does not silently invalidate an agent's working refmap

## Work Log

- 2026-02-19: Finding identified by architecture-strategist review agent
