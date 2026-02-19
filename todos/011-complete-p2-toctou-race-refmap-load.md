---
status: pending
priority: p2
issue_id: "011"
tags: [security, code-review, race-condition, file-io]
---

# TOCTOU Race Condition on RefMap Load

## Problem Statement

`refs.rs` reads the RefMap file with a size-check via `metadata()` followed by `read_to_string()`. Between the two syscalls, another process (a concurrent agent) could replace the file with a larger one, causing the size-based check to miss oversized content. This is a classic TOCTOU (time-of-check/time-of-use) race.

## Findings

**File:** `crates/core/src/refs.rs:84-97`

```rust
let meta = fs::metadata(&path)?;
if meta.len() > MAX_REFMAP_SIZE {
    return Err(...);
}
let content = fs::read_to_string(&path)?;  // Different file by now!
```

Additionally, there is no file locking, so two concurrent agents running `snapshot` simultaneously can corrupt the RefMap with a partial write.

## Proposed Solutions

### Option A: Read then check size (Recommended)
Read the file content first (bounded by `take(MAX_REFMAP_SIZE + 1)`), then check size. Single open, no race window.
```rust
let mut f = File::open(&path)?;
let mut content = String::new();
f.take(MAX_REFMAP_SIZE + 1).read_to_string(&mut content)?;
if content.len() > MAX_REFMAP_SIZE as usize { return Err(...); }
```
- **Effort:** Small
- **Risk:** Low

### Option B: Add advisory file locking (flock)
Use `fs2::FileExt::lock_exclusive()` during write, `lock_shared()` during read.
- **Effort:** Small
- **Risk:** Low — handles concurrent agent scenario

### Option C: Use temp file + atomic rename (already done for write)
The write path already uses temp+rename. Ensure the read path handles partial writes by checking file size after open, not before.
- **Effort:** Tiny
- **Risk:** Low

## Recommended Action

Option A + Option B. Eliminates TOCTOU and handles concurrent agents.

## Technical Details

- **File:** `crates/core/src/refs.rs`
- **Lines:** 84–97
- **Component:** RefMap loader

## Acceptance Criteria

- [ ] File size check occurs after open, not before
- [ ] Concurrent snapshot runs do not corrupt the RefMap
- [ ] Oversized RefMap returns structured error, not panic

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
