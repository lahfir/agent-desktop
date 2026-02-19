---
status: pending
priority: p3
issue_id: "044"
tags: [code-review, agent-native, commands]
---

# find Only Returns Elements With ref_id — Non-Interactive Matches Silently Dropped

## Problem Statement

The `find` command searches the tree for matching elements but only emits matches that have a `ref_id`. Non-interactive elements (labels, static text, containers) never get refs and are silently absent from results. An agent cannot distinguish "element not present" from "element present but not interactive." This makes `find` unusable for locating static labels.

## Findings

**File:** `crates/core/src/commands/find.rs:39-49`

```rust
if role_match && name_match && value_match {
    if let Some(ref_id) = &node.ref_id {
        // Only emitted if ref_id is Some
        matches.push(json!({ "ref": ref_id, ... }));
    }
    // Non-interactive match: silently skipped
}
```

An agent searching for a heading label to verify page state will get an empty `matches` array even if the heading exists, with no indication of why.

## Proposed Solutions

### Option A: Return non-interactive matches with ref: null (Recommended)
```json
{ "ref": null, "role": "text", "name": "Welcome", "interactive": false }
```
Agents can check `interactive: false` and understand the element is present but not actionable.
- **Effort:** Small
- **Risk:** Low

### Option B: Add --include-static flag
Only emit non-interactive matches when `--include-static` is passed. Default behavior unchanged.
- **Effort:** Small
- **Risk:** Low

### Option C: Document the limitation
Add `"note"` field to find response: "Only interactive elements with refs are returned."
- **Effort:** Tiny
- **Risk:** None

## Recommended Action

Option A: return all matches with an `interactive` flag. Agents should be able to verify element presence regardless of interactivity.

## Technical Details

- **File:** `crates/core/src/commands/find.rs`
- **Lines:** 39–49

## Acceptance Criteria

- [ ] `find --name "Welcome"` returns the element even if it's a non-interactive label
- [ ] Non-interactive matches have `ref: null` and `interactive: false`
- [ ] Interactive matches still have their ref ID

## Work Log

- 2026-02-19: Finding identified by agent-native-reviewer agent
