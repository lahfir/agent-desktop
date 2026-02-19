---
status: pending
priority: p3
issue_id: "018"
tags: [code-review, concurrency, file-io]
---

# No File Locking for Concurrent Agent Snapshot Writes

## Problem Statement

When two AI agents run `agent-desktop snapshot` simultaneously, they both write to `~/.agent-desktop/last_refmap.json`. The write uses a temp file + atomic rename, which prevents torn writes. However, agent B's snapshot will silently overwrite agent A's snapshot, invalidating any refs agent A is about to use. There is no coordination mechanism.

## Findings

**File:** `crates/core/src/refs.rs`

The write path is:
```rust
fs::write(&tmp_path, &json_bytes)?;
fs::rename(&tmp_path, &refmap_path)?;
```

This is atomic at the filesystem level, but two concurrent snapshots will result in one overwriting the other with no error to the losing agent.

## Proposed Solutions

### Option A: Session-scoped RefMap files
Name the RefMap file by session: `~/.agent-desktop/sessions/{session_id}/last_refmap.json`. Each agent instance uses its own file. No contention.
- **Effort:** Medium
- **Risk:** Low — cleaner design; enables multi-agent workflows

### Option B: Advisory flock on write
Use `fs2` crate's exclusive lock before writing. If lock fails, return error rather than overwriting.
- **Effort:** Small
- **Risk:** Low

### Option C: Document the limitation
Add a `///` doc-comment noting that concurrent snapshots are not supported in Phase 1.
- **Effort:** Tiny
- **Risk:** None — defers to Phase 4 daemon

## Recommended Action

Option C now (document), Option A in Phase 4 (session-scoped files as part of daemon design).

## Technical Details

- **File:** `crates/core/src/refs.rs`
- **Component:** RefMap write path

## Acceptance Criteria

- [ ] Limitation is documented
- [ ] Phase 4 design includes session-scoped RefMap files

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
