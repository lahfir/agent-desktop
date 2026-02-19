---
status: pending
priority: p1
issue_id: "039"
tags: [code-review, data-integrity, file-io]
---

# RefMap Not Flushed Before Rename — Partial Write Survives Process Kill

## Problem Statement

The RefMap write path calls `file.write_all()` then `fs::rename()` without an explicit `file.flush()` or `file.sync_all()`. The `write_all` writes data into the kernel page cache. The `rename` can succeed while content is still unflushed. On process kill or power loss, `last_refmap.json` exists at the destination path but contains truncated or zero bytes. Next load returns a parse error, making all refs stale.

## Findings

**File:** `crates/core/src/refs.rs:69-80`

```rust
let mut file = OpenOptions::new()
    .write(true).create(true).truncate(true).mode(0o600)
    .open(&tmp)?;
file.write_all(json.as_bytes())?;
// No flush() or sync_all() here
std::fs::rename(&tmp, &path)?;
```

The atomic temp+rename pattern is used correctly for POSIX atomicity at the rename level, but without `flush()` the in-process BufWriter buffer and kernel page cache may not yet be written to disk when `rename` completes.

## Proposed Solutions

### Option A: Add flush() before rename (Recommended)
```rust
file.write_all(json.as_bytes())?;
file.flush()?;
std::fs::rename(&tmp, &path)?;
```
- **Effort:** Tiny
- **Risk:** Low — `flush()` guarantees BufWriter buffer is written to OS; `sync_all()` is stronger (fsync)

### Option B: Use sync_all() for full durability
```rust
file.write_all(json.as_bytes())?;
file.flush()?;
file.sync_all()?;  // calls fsync
std::fs::rename(&tmp, &path)?;
```
- **Effort:** Tiny
- **Risk:** Low — slower on spinning disks, negligible on SSDs

## Recommended Action

Option A for correctness. Option B if crash-safety against power loss is required (SSD-only environments make this nearly free).

## Technical Details

- **File:** `crates/core/src/refs.rs`
- **Lines:** 69–80
- **Component:** RefMap write path

## Acceptance Criteria

- [ ] `file.flush()` is called before `fs::rename`
- [ ] A SIGKILL immediately after `flush()` leaves a valid RefMap file
- [ ] Empty/truncated RefMap on next load returns structured error, not panic

## Work Log

- 2026-02-19: Finding identified by data-integrity-guardian review agent
