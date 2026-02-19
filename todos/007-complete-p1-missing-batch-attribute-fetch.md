---
status: pending
priority: p1
issue_id: "007"
tags: [performance, code-review, macos, ax-api]
---

# Missing AXUIElementCopyMultipleAttributeValues Batch Fetch

## Problem Statement

`build_subtree` in the macOS tree builder makes 6 separate `AXUIElementCopyAttributeValue` IPC calls per node. For a typical app with 2000 nodes, this is 12,000 IPC round-trips. macOS's batch API `AXUIElementCopyMultipleAttributeValues` fetches all attributes in a single IPC call, providing a documented 3–5x speedup. This is explicitly required by the CLAUDE.md architecture spec.

## Findings

**File:** `crates/macos/src/tree.rs:59-73`

```rust
// Current: 6 separate IPC calls per node
let role = copy_string_attr(element, "AXRole")?;
let name = copy_string_attr(element, "AXTitle")?;
let value = copy_string_attr(element, "AXValue")?;
let description = copy_string_attr(element, "AXDescription")?;
let enabled = copy_bool_attr(element, "AXEnabled")?;
let children = copy_children_attr(element, "AXChildren")?;
```

Additionally, `copy_string_attr` creates a new `CFString::new(attr)` on every call, resulting in ~14,000 CFString allocations for a 2000-node tree.

**Impact:** Snapshot of a complex app (e.g., Xcode) will exceed the 2-second CI gate requirement. Typical observed latency is 8–15 seconds.

## Proposed Solutions

### Option A: Use AXUIElementCopyMultipleAttributeValues (Recommended)
Batch-fetch `["AXRole", "AXTitle", "AXValue", "AXDescription", "AXEnabled", "AXChildren"]` in a single call per node. Parse the returned CFArray. This is 1 IPC call instead of 6 per node.
- **Effort:** Medium
- **Risk:** Low — documented Apple API, used by Accessibility Inspector

### Option B: Cache CFString attribute keys as statics
Create static `CFString` values for the 6 attribute names once at startup (or lazy_static). Eliminates 14,000 CFString allocations per snapshot.
- **Effort:** Small
- **Risk:** Low — independent optimization, combine with Option A

### Option C: Parallel tree traversal with rayon
Process subtrees in parallel using rayon. But AXUIElement is not thread-safe (see issue 004), so this requires an AX dispatcher thread with work-stealing.
- **Effort:** Large
- **Risk:** High — dependency on threading model fix

## Recommended Action

Option A + B together. Implement batch fetch AND cache static CFString keys. Expected result: 3–5x speedup on snapshot latency.

## Technical Details

- **File:** `crates/macos/src/tree.rs`
- **Lines:** 59–73
- **Component:** macOS tree builder

## Acceptance Criteria

- [ ] `build_subtree` uses `AXUIElementCopyMultipleAttributeValues` for attribute fetching
- [ ] Attribute name CFStrings are created once (not per-node)
- [ ] Snapshot of Xcode (large tree) completes in < 2 seconds on CI

## Work Log

- 2026-02-19: Finding identified by performance-oracle review agent
