---
status: pending
priority: p3
issue_id: "045"
tags: [code-review, naming, spec-compliance]
---

# WindowInfo.app Field Diverges from Spec (Should be app_name)

## Problem Statement

The CLAUDE.md architecture spec defines `WindowInfo` as `{ id, title, app_name, pid, bounds }`. The implementation uses `app` instead of `app_name`. The JSON output field is `"app"` rather than `"app_name"`, breaking any agent or tool that reads the spec-compliant field name.

## Findings

**File:** `crates/core/src/node.rs:58`

```rust
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub app: String,   // CLAUDE.md spec says "app_name"
    pub pid: i32,
    pub bounds: Option<Rect>,
    pub is_focused: bool,
}
```

JSON output: `{"id": "w-1", "title": "...", "app": "Safari", ...}`
Spec: `{"id": "w-1", "title": "...", "app_name": "Safari", ...}`

## Proposed Solutions

### Option A: Rename field + add serde rename attribute
```rust
#[serde(rename = "app_name")]
pub app_name: String,
```
- **Effort:** Tiny (field rename + fix all usages)
- **Risk:** Low — but it's a breaking change to the JSON output

### Option B: Add serde rename without renaming field
Keep field as `app` in Rust code, use `#[serde(rename = "app_name")]` for JSON output.
- **Effort:** Tiny
- **Risk:** Low — preserves internal Rust naming while fixing JSON output

## Recommended Action

Option B: `#[serde(rename = "app_name")]` on the existing `app` field. Fixes the JSON output without touching all internal usages.

## Technical Details

- **File:** `crates/core/src/node.rs:58`
- **Component:** WindowInfo serialization

## Acceptance Criteria

- [ ] `list-windows` JSON output uses `"app_name"` field
- [ ] `snapshot` response uses `"app_name"` field
- [ ] Matches the CLAUDE.md spec definition of WindowInfo

## Work Log

- 2026-02-19: Finding identified by pattern-recognition-specialist agent
