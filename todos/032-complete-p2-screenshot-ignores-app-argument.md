---
status: pending
priority: p2
issue_id: "032"
tags: [code-review, correctness, commands]
---

# screenshot --app Argument Silently Ignored

## Problem Statement

When `screenshot --app Safari` is specified without a `--window-id`, the code takes a full-screen screenshot instead of the app's window. The `_app` variable is bound but never used.

## Findings

**File:** `crates/core/src/commands/screenshot.rs:16-19`

```rust
let target = match (&args.window_id, &args.app) {
    (Some(id), _)       => ScreenshotTarget::Window(id.clone()),
    (None, Some(_app))  => ScreenshotTarget::FullScreen,  // _app ignored!
    (None, None)        => ScreenshotTarget::FullScreen,
};
```

The second arm should resolve the app name to a window ID via `list_windows` (as `snapshot::run` does) and produce `ScreenshotTarget::Window`.

## Proposed Solutions

### Option A: Resolve app to window ID via list_windows (Recommended)
```rust
(None, Some(app)) => {
    let windows = adapter.list_windows(&WindowFilter { app: Some(app.clone()), ..Default::default() })?;
    let win = windows.first().ok_or_else(|| AppError::not_found("No window found for app"))?;
    ScreenshotTarget::Window(win.id.clone())
}
```
- **Effort:** Small
- **Risk:** Low

### Option B: Return INVALID_ARGS if --app without --window-id
Reject the combination and tell the caller to use `list-windows` first.
- **Effort:** Tiny
- **Risk:** Low — honest, explicit

## Recommended Action

Option A: consistent with how `snapshot` handles the same case.

## Technical Details

- **File:** `crates/core/src/commands/screenshot.rs`
- **Lines:** 16–19

## Acceptance Criteria

- [ ] `screenshot --app Safari` captures Safari's frontmost window
- [ ] The `_app` binding is replaced with actual usage

## Work Log

- 2026-02-19: Finding identified by architecture-strategist review agent
