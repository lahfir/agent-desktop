---
status: pending
priority: p2
issue_id: "015"
tags: [security, code-review, configuration]
---

# home_dir() Uses Only HOME Env Var (Not getpwuid_r Fallback)

## Problem Statement

`refs.rs` uses `std::env::var("HOME")` to locate the RefMap directory. If the tool is invoked in an environment where `HOME` is unset (sandboxed environment, su/sudo context, CI runner, systemd service), it will fail with a confusing error instead of falling back to the system password database (`getpwuid_r`).

## Findings

**File:** `crates/core/src/refs.rs:112-114`

```rust
let home = std::env::var("HOME")
    .map_err(|_| AppError::internal("HOME env var not set"))?;
```

`HOME` can be unset in sandboxed macOS apps, in CI via `sudo`, or in daemon contexts. The POSIX-correct approach is to try `HOME` first, then fall back to `getpwuid_r(getuid())`.

## Proposed Solutions

### Option A: Use dirs::home_dir() crate (Recommended)
The `dirs` crate handles `HOME` + `getpwuid_r` fallback correctly on all platforms.
- **Effort:** Tiny (add one dependency)
- **Risk:** Low

### Option B: Manual getpwuid_r fallback
Call `libc::getpwuid_r(libc::getuid(), ...)` when HOME is unset. Platform-correct but verbose.
- **Effort:** Small
- **Risk:** Low

### Option C: Make RefMap path configurable via env var
Introduce `AGENT_DESKTOP_DATA_DIR` env var. Default to `$HOME/.agent-desktop` when set. Useful for testing and CI regardless.
- **Effort:** Small
- **Risk:** Low — good flexibility, independent of the HOME fallback fix

## Recommended Action

Option A (dirs crate) + Option C (configurable env var).

## Technical Details

- **File:** `crates/core/src/refs.rs`
- **Lines:** 112–114
- **Component:** RefMap path resolution

## Acceptance Criteria

- [ ] Works correctly when HOME is unset
- [ ] Falls back to getpwuid_r or equivalent
- [ ] Optional: accepts AGENT_DESKTOP_DATA_DIR env override

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
