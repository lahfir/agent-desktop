---
status: pending
priority: p3
issue_id: "021"
tags: [code-review, correctness]
---

# Blocked Combos Modifier Order Not Normalized in press.rs

## Problem Statement

`press.rs` has a blocklist of dangerous key combos (e.g., `cmd+shift+q` for logout, `cmd+q` for quit all). The check compares the string representation of the combo, but modifier order is not normalized. `"shift+cmd+q"` and `"cmd+shift+q"` represent the same combo but the check would only block one of them.

## Findings

**File:** `crates/core/src/commands/press.rs`

The blocked combo check compares formatted combo strings directly. If the caller specifies modifiers in a different order than the hardcoded blocklist, the check is bypassed:

```rust
// Blocked: "cmd+shift+q"
// Not blocked: "shift+cmd+q" — same key, different modifier order
```

## Proposed Solutions

### Option A: Normalize modifier order before comparison (Recommended)
Sort modifiers canonically: Ctrl < Alt < Shift < Cmd. Compare normalized representations.
- **Effort:** Tiny
- **Risk:** Low

### Option B: Use a HashSet of modifiers for comparison
Store blocked combos as `(key: String, modifiers: HashSet<Modifier>)` tuples. Compare sets, not strings.
- **Effort:** Small
- **Risk:** Low

## Recommended Action

Option B: compare modifier sets. More robust and easier to read.

## Technical Details

- **File:** `crates/core/src/commands/press.rs`
- **Component:** key combo blocklist

## Acceptance Criteria

- [ ] `cmd+shift+q` and `shift+cmd+q` are treated identically
- [ ] All dangerous combo variants are blocked regardless of modifier order

## Work Log

- 2026-02-19: Finding identified by security-sentinel review agent
