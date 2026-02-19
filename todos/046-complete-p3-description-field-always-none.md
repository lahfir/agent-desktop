---
status: pending
priority: p3
issue_id: "046"
tags: [code-review, correctness, macos]
---

# AccessibilityNode.description Always None — AXDescription Attribute Discarded

## Problem Statement

The macOS tree builder fetches `kAXDescriptionAttribute` as a fallback for `name`, but when both `kAXTitleAttribute` and `kAXDescriptionAttribute` are present, the description is discarded. The `AccessibilityNode.description` field is always `None` in all snapshot output, despite being part of the public JSON schema.

## Findings

**File:** `crates/macos/src/tree.rs:62-63, 90`

```rust
let name = copy_string_attr(el, kAXTitleAttribute)
    .or_else(|| copy_string_attr(el, kAXDescriptionAttribute));  // Description used as name fallback only

// Later:
description: None,  // Always None — AXDescriptionAttribute never stored separately
```

When an element has both a title (`kAXTitleAttribute`) and a description (`kAXDescriptionAttribute`), the description is silently dropped. For screen reader accessibility, the description often carries the most useful context (e.g., "Save the document to disk" on a "Save" button).

## Proposed Solutions

### Option A: Store description separately when both attributes exist
```rust
let title = copy_string_attr(el, kAXTitleAttribute);
let desc = copy_string_attr(el, kAXDescriptionAttribute);
let name = title.or(desc.clone());
// ...
description: desc.filter(|_| title.is_some()),  // Only set when title also exists
```
- **Effort:** Small
- **Risk:** Low

### Option B: Always read description into AccessibilityNode.description
Read `kAXDescriptionAttribute` unconditionally and store it in `description`, separate from `name`.
- **Effort:** Small
- **Risk:** Low

## Recommended Action

Option B: store description separately. More faithful to the AX data model.

## Technical Details

- **File:** `crates/macos/src/tree.rs`
- **Lines:** 62–63, 90
- **Component:** macOS tree builder, AccessibilityNode population

## Acceptance Criteria

- [ ] Elements with both AXTitle and AXDescription expose `description` in JSON
- [ ] `description` field in AccessibilityNode is not always `null`

## Work Log

- 2026-02-19: Finding identified by pattern-recognition-specialist agent
