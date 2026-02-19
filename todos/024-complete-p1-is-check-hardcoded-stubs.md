---
status: pending
priority: p1
issue_id: "024"
tags: [code-review, correctness, commands]
---

# is_check Command Returns Hardcoded Stub Values

## Problem Statement

The `is` command (is visible, is enabled, is checked, is focused, is expanded) resolves the element handle then completely ignores it. All five properties return hardcoded values: `visible=true`, `enabled=!role.is_empty()`, `checked=false`, `focused=false`, `expanded=false`. No live AX query is made.

## Findings

**File:** `crates/core/src/commands/is_check.rs:28-34`

```rust
let result = match args.property {
    IsProperty::Visible  => true,                    // always true
    IsProperty::Enabled  => !entry.role.is_empty(), // heuristic, not AX
    IsProperty::Checked  => false,                   // always false
    IsProperty::Focused  => false,                   // always false
    IsProperty::Expanded => false,                   // always false
};
```

The `_handle` from `resolve_element` is discarded. An AI agent using `is checked @e3` to verify a checkbox state will always get `false`. The `states` field is also absent from `RefEntry`, making cache-based answers impossible too.

## Proposed Solutions

### Option A: Add states to RefEntry, answer from cache (Recommended)
Add `states: Vec<String>` to `RefEntry`. Populate it during snapshot from `AccessibilityNode.states`. Answer `is_check` queries from the cached states.
- **Effort:** Medium
- **Risk:** Low — states are already in AccessibilityNode

### Option B: Add query_states to PlatformAdapter
Add `fn query_states(&self, handle: &NativeHandle) -> Result<Vec<String>, AdapterError>`. Implement in macOS adapter via `kAXEnabledAttribute`, `kAXFocusedAttribute`, etc.
- **Effort:** Medium
- **Risk:** Low — clean API addition

## Recommended Action

Option A: populate states into RefEntry during snapshot. This fixes `is_check`, `get states`, and improves bounds detection in one change.

## Technical Details

- **File:** `crates/core/src/commands/is_check.rs`
- **Lines:** 28–34

## Acceptance Criteria

- [ ] `is checked @e3` returns the actual checked state from the AX tree
- [ ] `is focused @e3` returns the actual focus state
- [ ] `is expanded @e3` returns the actual expanded state
- [ ] No hardcoded return values remain

## Work Log

- 2026-02-19: Finding identified by architecture-strategist review agent
