---
status: pending
priority: p1
issue_id: "025"
tags: [code-review, correctness, commands]
---

# get Command Returns null for value, bounds, and states

## Problem Statement

The `get` command supports 6 properties (text, value, title, bounds, role, states) but three of them always return null/empty regardless of the element's actual content. `get value`, `get bounds`, and `get states` are non-functional stubs.

## Findings

**File:** `crates/core/src/commands/get.rs:22-29`

```rust
GetProperty::Value  => json!(null),   // never implemented
GetProperty::Bounds => json!(null),   // never implemented
GetProperty::States => json!([]),     // never implemented
```

An AI agent calling `get @e3 --property value` on a text field will always receive `null`. This makes the command useless for the primary use cases of verifying form field content and reading UI state.

Root cause: `RefEntry` does not store `value` or `states`; bounds are not populated (see issue 023).

## Proposed Solutions

### Option A: Store value and states in RefEntry (Recommended)
Add `value: Option<String>` and `states: Vec<String>` to `RefEntry`. Populate from `AccessibilityNode` during snapshot. Answer from cache in get command.
- **Effort:** Medium
- **Risk:** Low

### Option B: Live-query via PlatformAdapter
Add `query_attribute(handle, attr) -> Result<Value, AdapterError>` to the trait. Call on-demand in get command.
- **Effort:** Medium
- **Risk:** Low — allows reading attributes not captured at snapshot time

## Recommended Action

Option A for value and states (cache from snapshot). Bounds from issue 023 fix. Both needed before Phase 1 is shippable.

## Technical Details

- **File:** `crates/core/src/commands/get.rs`
- **Lines:** 22–29

## Acceptance Criteria

- [ ] `get @e3 --property value` returns the text content of the element
- [ ] `get @e3 --property states` returns array of state strings
- [ ] `get @e3 --property bounds` returns `{x, y, width, height}` (depends on issue 023 fix)
- [ ] No `json!(null)` stubs remain in get.rs

## Work Log

- 2026-02-19: Finding identified by architecture-strategist review agent
