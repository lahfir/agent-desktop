---
status: pending
priority: p3
issue_id: "035"
tags: [code-review, architecture, dead-code]
---

# Response Struct in output.rs Unused — Wire Format Can Diverge Silently

## Problem Statement

`core/src/output.rs` defines typed `Response`, `AppContext`, `WindowContext`, and `ErrorPayload` structs. However, `main.rs` builds the response envelope using raw `serde_json::json!()` macros instead. The typed structs and the actual wire format can silently diverge. The `platform_detail` field on `ErrorPayload` is never populated.

## Findings

**File:** `crates/core/src/output.rs` — defines `Response`, etc.
**File:** `src/main.rs:93-98` — builds response with raw `json!()` macro

The `Response` struct has a `app` field; `main.rs` produces no `app` field. These are already diverged.

## Proposed Solutions

### Option A: Use the typed Response struct in main.rs (Recommended)
Replace `serde_json::json!({ "version": "1.0", ... })` with `Response { version: "1.0", ... }` and serialize via serde.
- **Effort:** Small
- **Risk:** Low — ensures struct and wire format can't diverge

### Option B: Delete output.rs structs
If the structs are truly never used, remove them to eliminate the confusion.
- **Effort:** Tiny
- **Risk:** Low — they're dead code

## Recommended Action

Option A for Phase 3 (MCP server needs typed structs for schema generation). Option B as immediate cleanup if Phase 3 is far off.

## Technical Details

- **Files:** `crates/core/src/output.rs`, `src/main.rs`

## Acceptance Criteria

- [ ] Response envelope is built from typed structs, not raw json! macros
- [ ] `platform_detail` in error responses is populated when available
- [ ] The Response struct and actual wire format cannot diverge

## Work Log

- 2026-02-19: Finding identified by architecture-strategist review agent
