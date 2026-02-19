---
status: pending
priority: p2
issue_id: "013"
tags: [security, code-review, path-traversal]
---

# Screenshot output_path Has No Path Traversal Validation

## Problem Statement

The `screenshot` command accepts an `--output` path without validating it. A caller can write screenshots to arbitrary locations: `/etc/cron.d/evil`, `~/.ssh/authorized_keys`, or `../../../sensitive`. This enables data exfiltration (write sensitive bytes as PNG header) or filesystem writes outside the intended output directory.

## Findings

**File:** `crates/core/src/commands/screenshot.rs:24-26`

```rust
pub struct ScreenshotArgs {
    pub output_path: Option<PathBuf>,  // No validation
}
```

No canonicalization, no `..` component check, no restriction to user-writable directories.

## Proposed Solutions

### Option A: Validate path is within allowed directories (Recommended)
Canonicalize the path. Ensure it doesn't contain `..` components after resolution. Optionally restrict to the current directory or a configured output directory.
- **Effort:** Small
- **Risk:** Low

### Option B: Restrict to .png extension
Require `output_path` to end in `.png` or `.jpg`. Prevents writing to arbitrary paths used as config files.
- **Effort:** Tiny
- **Risk:** Medium — doesn't prevent writing to arbitrary .png paths

### Option C: Write to a temp file and return path in JSON
Never accept a caller-specified path. Write to a temp file, return its path in the JSON response. Caller is responsible for moving it.
- **Effort:** Small
- **Risk:** Low — eliminates the attack surface entirely

## Recommended Action

Option A: canonicalize + `..` check. Also enforce `.png`/`.jpg` extension (Option B) as defense-in-depth.

## Technical Details

- **File:** `crates/core/src/commands/screenshot.rs`
- **Lines:** 24–26
- **Component:** screenshot command

## Acceptance Criteria

- [ ] Paths containing `..` components are rejected with `INVALID_ARGS`
- [ ] Output path is validated before writing
- [ ] Screenshot to valid path in current directory still works

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
