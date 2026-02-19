---
status: pending
priority: p2
issue_id: "012"
tags: [security, code-review, denial-of-service]
---

# wait Command ms Parameter Unbounded (DoS Vector)

## Problem Statement

The `wait` command accepts a `u64` milliseconds parameter with no upper bound. `agent-desktop wait 18446744073709551615` would block for 585 million years, permanently hanging the invoking agent. Even `wait 3600000` (1 hour) would stall an agent indefinitely.

## Findings

**File:** `crates/core/src/commands/wait.rs`

```rust
pub struct WaitArgs {
    pub ms: u64,  // No upper bound
}
// ...
std::thread::sleep(Duration::from_millis(args.ms));
```

An AI agent that misinterprets a value or is given adversarial input could invoke this with an enormous duration, causing the process to hang permanently.

## Proposed Solutions

### Option A: Cap at a reasonable maximum (Recommended)
Define `MAX_WAIT_MS: u64 = 30_000` (30 seconds). Return `INVALID_ARGS` if exceeded.
- **Effort:** Tiny
- **Risk:** Low

### Option B: Validate at CLI layer via clap value_parser
Use `clap`'s `value_parser(1_u64..=30_000_u64)` to reject out-of-range values before reaching execute().
- **Effort:** Tiny
- **Risk:** Low

### Option C: Use a configurable timeout with a hard cap
Allow caller to specify up to N ms (configurable, default 30s, max 300s).
- **Effort:** Small
- **Risk:** Low

## Recommended Action

Option B: validate at CLI layer with `value_parser`. Clean, zero-overhead, clear error message.

## Technical Details

- **File:** `crates/core/src/commands/wait.rs`
- **Component:** wait command

## Acceptance Criteria

- [ ] `wait` rejects ms values > 30,000 (or chosen cap)
- [ ] Error response uses `INVALID_ARGS` error code
- [ ] `wait 100` still works correctly

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
