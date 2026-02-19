---
status: pending
priority: p2
issue_id: "016"
tags: [performance, code-review, memory, macos]
---

# Screenshot Allocates Zeroed Buffer Instead of Actual Pixel Data

## Problem Statement

The macOS screenshot implementation allocates a zeroed `Vec<u8>` of the expected size instead of capturing actual pixel data from `CGWindowListCreateImage`. The output is always a blank image. This means the screenshot command is non-functional, and when it does capture real pixels, it will allocate 30MB+ for a Retina display without streaming.

## Findings

**File:** `crates/core/src/commands/screenshot.rs`

```rust
let buffer = vec![0u8; width * height * 4];  // Blank pixel data
```

Issues:
1. Functionally broken — returns blank PNG
2. When fixed, Retina displays produce 30–60MB raw pixel data held entirely in memory
3. No streaming to disk — entire image in RAM before write

## Proposed Solutions

### Option A: Implement CGWindowListCreateImage + stream PNG encode (Recommended)
Call `CGWindowListCreateImage` → `CGImageGetDataProvider` → `CGDataProviderCopyData`. Encode to PNG via `image` crate with streaming write to file.
- **Effort:** Medium
- **Risk:** Low — straightforward CGImage → PNG pipeline

### Option B: Use screencapture CLI tool
Spawn `screencapture -x -t png {output_path}` as a subprocess. Simple, handles all edge cases, but adds process spawn overhead (~100ms).
- **Effort:** Tiny
- **Risk:** Low — but subprocess dependency

### Option C: Return base64-encoded PNG in JSON (no output_path)
Encode PNG to base64, return inline in JSON response. Avoids path traversal concern (013) entirely.
- **Effort:** Medium
- **Risk:** Low for small screenshots; Large responses for Retina

## Recommended Action

Option A for correctness and performance. Use Option B as a temporary fix to unblock functional testing while Option A is implemented.

## Technical Details

- **File:** `crates/core/src/commands/screenshot.rs`, `crates/macos/src/adapter.rs`
- **Component:** screenshot command, macOS screenshot adapter

## Acceptance Criteria

- [ ] Screenshot returns actual pixel data (not zeroed buffer)
- [ ] Retina display screenshot does not OOM (streaming or bounded allocation)
- [ ] Output PNG is valid and viewable

## Work Log

- 2026-02-19: Finding identified by performance-oracle review agent
