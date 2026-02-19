---
status: pending
priority: p1
issue_id: "023"
tags: [code-review, correctness, macos]
---

# read_bounds Always Returns None — Bounds Never Populated

## Problem Statement

`read_bounds` in the macOS tree builder is a stub that always returns `None`. The `--include-bounds` flag is accepted by the CLI and threaded through the entire call chain, but no bounds data is ever populated. Any feature relying on bounds (coordinate-based element identification, `get bounds`, `scroll`, visual testing) silently returns no data.

## Findings

**File:** `crates/macos/src/tree.rs:160`

```rust
fn read_bounds(_el: &AXElement) -> Option<Rect> {
    None  // Stub — bounds never implemented
}
```

This also means:
1. `RefEntry.bounds_hash` will always hash `None`, making all interactive elements produce the same bounds hash — zero entropy for element re-identification
2. `agent-desktop get bounds @e5` will always return empty data
3. `agent-desktop snapshot --include-bounds` returns a tree with no bounds despite the flag

The `kAXPositionAttribute` + `kAXSizeAttribute` attributes on AXUIElement provide exact bounds. `CGPoint` and `CGSize` can be extracted via `AXValueGetValue(AXValueType::CGPoint, ...)` and `AXValueGetValue(AXValueType::CGSize, ...)`.

## Proposed Solutions

### Option A: Implement bounds extraction via AX attributes (Recommended)
```rust
fn read_bounds(el: &AXElement) -> Option<Rect> {
    let pos = read_cgpoint(el, "AXPosition")?;
    let size = read_cgsize(el, "AXSize")?;
    Some(Rect { x: pos.x as f32, y: pos.y as f32,
                width: size.width as f32, height: size.height as f32 })
}
```
Extract from `kAXPositionAttribute` (returns `CGPoint`) and `kAXSizeAttribute` (returns `CGSize`) using `AXValueGetValue`.
- **Effort:** Small
- **Risk:** Low — standard AX API

### Option B: Batch fetch position+size with other attributes
Include `kAXPositionAttribute` and `kAXSizeAttribute` in the multi-attribute batch fetch (issue 007). Parse bounds there.
- **Effort:** Small (part of the batch fetch fix)
- **Risk:** Low — natural companion to issue 007

## Recommended Action

Option B: implement as part of the batch fetch fix (issue 007). Both need to touch the same tree-traversal code.

## Technical Details

- **File:** `crates/macos/src/tree.rs`
- **Line:** 160
- **Component:** macOS tree builder, bounds extraction

## Acceptance Criteria

- [ ] `snapshot --include-bounds` returns non-null bounds for all rendered elements
- [ ] `get bounds @e5` returns `{x, y, width, height}`
- [ ] `bounds_hash` in RefEntry has meaningful entropy (not all-same for all elements)

## Work Log

- 2026-02-19: Finding identified by git-history-analyzer agent
