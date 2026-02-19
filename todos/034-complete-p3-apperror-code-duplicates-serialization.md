---
status: pending
priority: p3
issue_id: "034"
tags: [code-review, architecture, duplication]
---

# AppError::code() Duplicates ErrorCode Serde Serialization

## Problem Statement

`ErrorCode` uses `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` for automatic serialization. But `AppError::code()` is a manually maintained match that maps enum variants to the same hardcoded strings. When a new error code is added, it must be added in 3 places: the enum, the serde attribute (automatic), and the `code()` match. The `main.rs` response builder uses `e.code()` (the string method) rather than serializing through serde, bypassing the single source of truth.

## Findings

**File:** `crates/core/src/error.rs:107-126`

```rust
pub fn code(&self) -> &str {
    match self {
        AppError::PermissionDenied => "PERM_DENIED",
        AppError::ElementNotFound  => "ELEMENT_NOT_FOUND",
        // ... manually duplicated
    }
}
```

## Proposed Solutions

### Option A: Derive code() from serde (Recommended)
```rust
pub fn code(&self) -> String {
    serde_json::to_value(self.error_code())
        .and_then(|v| v.as_str().map(String::from)
        .unwrap_or("INTERNAL")
}
```
- **Effort:** Small
- **Risk:** Low — eliminates manual maintenance

### Option B: Use strum's Display derive
Add `strum` crate, derive `Display` with `SCREAMING_SNAKE_CASE` transform on `ErrorCode`.
- **Effort:** Tiny (add dependency)
- **Risk:** Low

## Recommended Action

Option A: derive from serde. No new dependencies.

## Technical Details

- **File:** `crates/core/src/error.rs:107-126`

## Acceptance Criteria

- [ ] `AppError::code()` is not a manual match
- [ ] Adding a new error code variant requires exactly one change location

## Work Log

- 2026-02-19: Finding identified by architecture-strategist review agent
