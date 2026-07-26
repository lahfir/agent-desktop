---
title: "feat: add notification management commands (macOS)"
type: feat
status: completed
date: 2026-02-27
deepened: 2026-02-27
origin: docs/brainstorms/2026-02-27-notification-management-brainstorm.md
---

# feat: add notification management commands (macOS)

## Enhancement Summary

**Deepened on:** 2026-02-27
**Sections enhanced:** 7 (all major sections)
**Review agents used:** architecture-strategist, performance-oracle, security-sentinel, code-simplicity-reviewer, pattern-recognition-specialist, agent-native-reviewer, best-practices-researcher

### Key Improvements

1. **Dropped NSDistributedNotificationCenter observer** — Polling-only for v1. Observer adds thread-safety risks (HIGH), undocumented API surface, and complexity for marginal latency gain. The reliable path is AX polling.
2. **Reduced adapter surface from 5 methods to 3** — `list_notifications`, `dismiss_notification`, `notification_action`. Dismiss-all composes on `list_notifications`. Wait composes on `list_notifications` polling.
3. **Dropped `NotificationUnsupported` error code** — Reuse existing `PlatformNotSupported` (pattern-recognition, architecture-strategist).
4. **Added content-based verification for dismiss/interact** — TOCTOU mitigation: verify `app_name` + `title` match before acting on a positional index (agent-native-reviewer CRITICAL).
5. **Replaced AppleScript NC open with pure AX** — Eliminates shell injection risk from existing AppleScript pattern (security-sentinel HIGH).
6. **Added index >= 1 validation** — Index 0 with 1-based-to-0-based conversion causes `usize` underflow → panic (security-sentinel MEDIUM).
7. **Deferred v1 scope**: focus mode detection, inline action UI, auto-expansion of collapsed groups, `observer.rs` — all deferred to v2 to keep initial implementation minimal (~315 LOC reduction).

### New Considerations Discovered

- macOS Sequoia (15) added extra AXGroup nesting level in NC AX tree — heuristic parser must handle both Sonoma and Sequoia layouts
- NC close buttons only appear on hover in Sequoia — mouse hover synthesis required before dismiss
- Notification text may be `NSConcreteAttributedString` in Sequoia — need to handle both plain and attributed string extraction
- `typed batch path` is already at 472 LOC (over 400 limit) — adding 4 commands requires splitting first

---

## Overview

Add 4 new commands + 1 wait extension enabling AI agents to read, dismiss, and interact with macOS notifications via the Notification Center accessibility tree. This is the first cross-platform notification feature — macOS ships first, Windows/Linux implementations follow in their respective phases.

The implementation follows the existing extensibility pattern: new domain types in core → new adapter trait methods with `not_supported` defaults → command handlers → CLI wiring → macOS-specific implementation in a new `notifications/` subfolder.

## Problem Statement

agent-desktop's 50-command surface covers app lifecycle, UI interaction, clipboard, screenshots, and keyboard/mouse — but has zero visibility into OS-level notifications. Agents automating desktop workflows frequently need to:

- Detect when a notification arrives (e.g., download complete, message received)
- Read notification content to decide next steps
- Dismiss notifications to clear clutter
- Click notification action buttons (Reply, Open, Join, etc.)

Without notification support, agents must resort to screenshot → OCR workflows or ignore notifications entirely.

## Proposed Solution

**Approach: AX tree traversal with polling-based wait** (see brainstorm: `docs/brainstorms/2026-02-27-notification-management-brainstorm.md`)

- **CRUD operations** (list, dismiss, dismiss-all, notification-action): Programmatically open Notification Center, traverse its AX tree, perform actions, close NC — all within a single RAII-guarded session per command invocation
- **Wait operation** (`wait --notification`): AX polling at configurable intervals (default 3 seconds), opening/closing NC per cycle

### Research Insights — Solution Design

**Architecture (architecture-strategist):**
- Keep CLI wiring incremental with each phase rather than a separate Phase 6. Each phase should produce a testable `cargo build` — wire CLI as you go.
- `list_notifications` return type must be consistent between trait signature and macOS impl. V1 returns `Vec<NotificationInfo>` from the trait; focus mode metadata is macOS-specific and returned via a wrapper type in the macOS crate, not the trait.

**Simplicity (code-simplicity-reviewer):**
- 3 adapter methods, not 5. `dismiss_all` composes on `list_notifications` in the command handler. `wait_for_notification` composes on `list_notifications` polling in the command handler.
- Merge `dismiss.rs` + `interact.rs` into a single `actions.rs` (~150 LOC combined, well under 400 limit).
- Drop `observer.rs` entirely from v1.

### Alternative Approaches Considered

| Approach | Why Rejected |
|----------|-------------|
| NSDistributedNotificationCenter for wait | Thread-safety risks (HIGH — security-sentinel), undocumented API surface, not all apps broadcast. Polling is the reliable path. Observer adds complexity for marginal latency gain. Deferred to v2 if polling proves insufficient. |
| Pure AX polling for wait (original brainstorm) | Adopted as the v1 approach after security/simplicity review. 3-second default interval, configurable via `--poll-interval`. |
| SQLite notification database | Requires Full Disk Access permission — unacceptable adoption barrier on top of existing Accessibility permission. Database schema is undocumented and changes across macOS versions. |
| Banner-only capture | Banners are transient (~5s display). Requires separate AX target. NC contents are sufficient for v1. |

## Technical Approach

### Architecture

```
┌──────────────────────────────────────────────────────┐
│  Binary crate (src/)                                 │
│  ┌─────────┐  ┌───────────┐  ┌──────────────────┐   │
│  │ cli.rs  │→ │cli_args.rs│→ │   dispatch.rs    │   │
│  │ 4 new   │  │ 4 new arg │  │ 4 new match arms │   │
│  │ variants│  │ structs   │  │ + wait extension │   │
│  └─────────┘  └───────────┘  └────────┬─────────┘   │
│                                       │              │
├───────────────────────────────────────┼──────────────┤
│  Core crate (crates/core/)            │              │
│  ┌──────────────┐ ┌──────────────┐    │              │
│  │notification.rs│ │  error.rs    │    │              │
│  │NotifInfo     │ │+1 error code │    │              │
│  │NotifFilter   │ └──────────────┘    │              │
│  └──────────────┘                     │              │
│  ┌─────────────┐  ┌─────────────┐     │              │
│  │ adapter.rs  │  │ commands/   │     │              │
│  │ +3 methods  │  │ 4 new files │◄────┘              │
│  │ (defaults)  │  └─────────────┘                    │
│  └─────────────┘                                     │
├──────────────────────────────────────────────────────┤
│  macOS crate (crates/macos/)                         │
│  ┌────────────┐  ┌──────────────────────────────┐    │
│  │ adapter.rs │→ │  notifications/ (NEW)        │    │
│  │ +3 impls   │  │  ├── mod.rs                  │    │
│  │            │  │  ├── nc_session.rs  (RAII)    │    │
│  │            │  │  ├── list.rs                  │    │
│  │            │  │  └── actions.rs               │    │
│  └────────────┘  └──────────────────────────────┘    │
└──────────────────────────────────────────────────────┘
```

### Research Insights — Architecture

**Naming conventions (pattern-recognition-specialist):**
- `dismiss-all-notifications` has 3 kebab segments — breaks the existing 2-segment convention (`list-windows`, `focus-window`). Consider `clear-notifications` instead.
- `notification-action` breaks the `{verb}-{noun}` pattern. Consider `click-notification` (verb-noun) with the action name as a required argument.
- Decision: Keep `list-notifications`, `dismiss-notification`, `dismiss-all-notifications`, `notification-action` as-is from the brainstorm. The 3-segment name for dismiss-all is acceptable because it disambiguates from dismiss-single. `notification-action` is acceptable because the action name is a positional arg.

**macOS version compatibility (best-practices-researcher):**
- NC process name: `NotificationCenter` (not `notificationcenterui`)
- NC is opened via SystemUIServer menu bar click (AX click on clock area), NOT directly via the NC process
- Sequoia (macOS 15) added an extra AXGroup nesting level — parser must handle variable nesting depth
- Close buttons only appear on hover in Sequoia — must synthesize mouse hover before looking for dismiss button
- Notification body text may be `NSConcreteAttributedString` — use `AXValueAttribute` with string coercion fallback

### Implementation Phases

#### Phase 1: Core Types and Adapter Trait + CLI Wiring

Establish the platform-agnostic foundation with CLI wiring. Produces a compilable binary where notification commands return `PLATFORM_NOT_SUPPORTED` on all platforms.

**Tasks:**

- [x] Create `crates/core/src/notification.rs` — `NotificationInfo` and `NotificationFilter` structs
- [x] Add 3 new methods to `PlatformAdapter` trait in `crates/core/src/adapter.rs` with `not_supported` defaults
- [x] Add 1 error code variant (`NotificationNotFound`) to `ErrorCode` enum in `crates/core/src/error.rs` + convenience constructor
- [x] Register module in `crates/core/src/lib.rs`
- [x] Create 4 command handlers in `crates/core/src/commands/`
- [x] Register 4 new modules in `crates/core/src/commands/mod.rs`
- [x] Add 4 command variants to `src/cli.rs` + `name()` arms
- [x] Add 4 arg structs to `src/cli_args.rs`
- [x] Add 4 match arms to `src/dispatch.rs` + extend Wait arm for `--notification`
- [x] Add 4 command routing arms to `src/typed batch path`
- [x] Add `--notification` flag + `--poll-interval` to `WaitArgs`

**New types in `crates/core/src/notification.rs`:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationInfo {
    pub index: usize,
    pub app_name: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NotificationFilter {
    pub app: Option<String>,
    pub text: Option<String>,
    pub limit: Option<usize>,
}
```

Design notes:
- 6 fields on `NotificationInfo` (under 7-field limit per CLAUDE.md)
- `is_persistent` dropped — not reliably detectable from AX, no agent use case (see brainstorm)
- `timestamp` is ISO 8601 string when absolute time is extractable from AX, `None` when only relative time is available
- `NotificationFilter` derives `Default` and `Clone` (pattern-recognition fix: missing derives)
- `focus_mode` is NOT in `NotificationInfo` or the trait return type — it's macOS-specific metadata added by the macOS command layer

**New adapter methods in `crates/core/src/adapter.rs`:**

```rust
fn list_notifications(&self, _filter: &NotificationFilter) -> Result<Vec<NotificationInfo>, AdapterError> {
    Err(AdapterError::not_supported("list_notifications"))
}

fn dismiss_notification(&self, _index: usize, _app_filter: Option<&str>) -> Result<NotificationInfo, AdapterError> {
    Err(AdapterError::not_supported("dismiss_notification"))
}

fn notification_action(&self, _index: usize, _action_name: &str) -> Result<ActionResult, AdapterError> {
    Err(AdapterError::not_supported("notification_action"))
}
```

Design notes:
- **3 methods** (reduced from 5). `dismiss_all_notifications` and `wait_for_notification` compose on `list_notifications` in command handlers — no separate adapter methods needed.
- `dismiss_notification` returns `NotificationInfo` of the dismissed notification for agent verification
- Reuse existing `ActionResult` for `notification_action` (code-simplicity: no new result type needed)

**New error code in `crates/core/src/error.rs`:**

```rust
// Add to ErrorCode enum:
NotificationNotFound,

// Add to as_str():
Self::NotificationNotFound => "NOTIFICATION_NOT_FOUND",

// Add convenience constructor to AdapterError:
pub fn notification_not_found(index: usize) -> Self {
    Self::new(
        ErrorCode::NotificationNotFound,
        format!("Notification at index {index} not found"),
    )
    .with_suggestion("Notification may have been dismissed or expired. Run 'list-notifications' to see current notifications")
}
```

Design notes (architecture-strategist, pattern-recognition, code-simplicity):
- **Only 1 new error code**, not 2. `NotificationUnsupported` is dropped — the existing `PlatformNotSupported` (returned by default `not_supported()` implementations) already covers this case identically.
- This matches the existing pattern: `clipboard_get` doesn't have a `ClipboardUnsupported` error, it just returns `PlatformNotSupported` on unsupported platforms.

### Research Insights — Error Handling

**Index validation (security-sentinel MEDIUM):**
- Index 0 with 1-based-to-0-based conversion (`index - 1`) causes `usize` underflow → `panic!` (since `panic = "abort"` in release profile, this is a process kill)
- **Must validate `index >= 1`** at the argument parsing level (clap `value_parser` with range) or in the command handler before subtraction
- Recommended: Add `#[arg(value_parser = clap::value_parser!(usize).range(1..))]` to all index args

**CLI args (`src/cli_args.rs`):**

```rust
#[derive(Parser, Debug)]
pub struct ListNotificationsArgs {
    #[arg(long, help = "Filter to notifications from this app")]
    pub app: Option<String>,
    #[arg(long, help = "Filter to notifications containing this text")]
    pub text: Option<String>,
    #[arg(long, help = "Maximum number of notifications to return")]
    pub limit: Option<usize>,
}

#[derive(Parser, Debug)]
pub struct DismissNotificationArgs {
    #[arg(value_name = "INDEX", help = "1-based notification index from list-notifications",
          value_parser = clap::value_parser!(usize).range(1..))]
    pub index: usize,
    #[arg(long, help = "Filter notifications by app before selecting index")]
    pub app: Option<String>,
}

#[derive(Parser, Debug)]
pub struct DismissAllNotificationsArgs {
    #[arg(long, help = "Only dismiss notifications from this app")]
    pub app: Option<String>,
}

#[derive(Parser, Debug)]
pub struct NotificationActionArgs {
    #[arg(value_name = "INDEX", help = "1-based notification index from list-notifications",
          value_parser = clap::value_parser!(usize).range(1..))]
    pub index: usize,
    #[arg(value_name = "ACTION", help = "Name of the action button to click (e.g., Reply, Open)")]
    pub action: String,
}
```

Design notes:
- `DismissNotificationArgs` now includes `--app` filter (pattern-recognition fix: missing from original plan)
- Both index args use `value_parser` with `range(1..)` to reject index 0 at parse time (security-sentinel fix)

**Wait args extension (`src/cli_args.rs`):**

Add to existing `WaitArgs`:
```rust
#[arg(long, help = "Wait for a notification to appear")]
pub notification: bool,
#[arg(long, help = "Poll interval in ms for notification wait (default: 3000)", default_value = "3000")]
pub poll_interval: Option<u64>,
```

**Command handlers (`crates/core/src/commands/`):**

Each handler follows Pattern C (args struct → execute → json). Example for `list_notifications.rs`:

```rust
pub fn execute(args: ListNotificationsArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let filter = NotificationFilter {
        app: args.app,
        text: args.text,
        limit: args.limit,
    };
    let notifications = adapter.list_notifications(&filter)?;
    Ok(json!({
        "count": notifications.len(),
        "notifications": notifications,
    }))
}
```

**`dismiss_all_notifications.rs` composes on list:**

```rust
pub fn execute(args: DismissAllNotificationsArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    // List all notifications matching filter, then dismiss each
    let filter = NotificationFilter { app: args.app.clone(), ..Default::default() };
    let notifications = adapter.list_notifications(&filter)?;
    let mut dismissed = 0;
    // Dismiss in reverse order (highest index first) to avoid index shifting
    for notif in notifications.iter().rev() {
        match adapter.dismiss_notification(notif.index, args.app.as_deref()) {
            Ok(_) => dismissed += 1,
            Err(_) => {} // notification may have already been dismissed
        }
    }
    Ok(json!({
        "dismissed_count": dismissed,
    }))
}
```

**Success criteria:**
- `cargo test --lib -p agent-desktop-core` passes
- `cargo clippy --all-targets -- -D warnings` clean
- All 4 commands parse and dispatch correctly
- `wait --notification` flag is recognized
- `--notification` is mutually exclusive with `--element`/`--window`
- Windows/Linux stubs compile without changes (default `not_supported` implementations)
- Index 0 rejected at parse time with clear error message

---

#### Phase 2: NC Session Guard (macOS)

The critical safety layer. Every notification command operates within a `NcSession` that guarantees Notification Center is opened before work and closed after — even on errors or panics.

**Tasks:**

- [x] Create `crates/macos/src/notifications/mod.rs` — module re-exports
- [x] Create `crates/macos/src/notifications/nc_session.rs` — RAII NC lifecycle guard
- [x] Register `notifications` module in `crates/macos/src/lib.rs`

**`NcSession` design (`crates/macos/src/notifications/nc_session.rs`):**

```rust
pub struct NcSession {
    was_already_open: bool,
}

impl NcSession {
    pub fn open() -> Result<Self, AdapterError> {
        let was_already_open = is_nc_open()?;
        if !was_already_open {
            open_nc()?;
            wait_for_nc_ready()?;  // wait for animation + AX tree population
        }
        Ok(Self { was_already_open })
    }

    /// Explicit close with error reporting. Prefer this over relying on Drop.
    pub fn close(self) -> Result<(), AdapterError> {
        if !self.was_already_open {
            close_nc()?;
        }
        std::mem::forget(self); // prevent Drop from double-closing
        Ok(())
    }
}

impl Drop for NcSession {
    fn drop(&mut self) {
        if !self.was_already_open {
            // Fire-and-forget close — log error but don't propagate
            if let Err(e) = close_nc() {
                tracing::warn!("Failed to close NC in Drop: {e}");
            }
        }
    }
}
```

### Research Insights — NC Session

**Explicit close + Drop fallback (best-practices-researcher, performance-oracle):**
- Provide `close(self)` for the happy path where callers want error feedback
- Drop handles panic/error paths as best-effort fallback
- Use `std::mem::forget(self)` in `close()` to prevent double-close from Drop
- Alternative: `scopeguard` crate — but adds a dependency for a pattern that's straightforward to implement manually

**Pure AX for NC open (security-sentinel HIGH, performance-oracle):**

```rust
fn open_nc() -> Result<(), AdapterError> {
    // 1. Find SystemUIServer process
    // 2. Get its AXApplication element
    // 3. Find the menu bar (AXMenuBar)
    // 4. Find the clock/date menu bar item
    // 5. AXPress to toggle NC open
    // NO AppleScript — eliminates shell injection attack surface
}
```

The existing `app_ops.rs:172-192` uses `osascript -e` with string interpolation — this is a known injection vector (security-sentinel). NC open must NOT replicate this pattern. Pure AX click on the SystemUIServer menu bar item is both safer and faster (~50ms vs ~200ms for AppleScript).

**NC state detection (best-practices-researcher):**
- Check `com.apple.notificationcenterui` process for visible AXWindow
- Do NOT use toggle — toggling inverts state if NC is already open
- NC process name is `NotificationCenter` on modern macOS, bundle ID is `com.apple.notificationcenterui`

**NC close mechanism (performance-oracle):**
- Send Escape key via `CGEventCreateKeyboardEvent` (fastest, most reliable)
- Fire-and-forget variant in Drop: dispatch close without waiting for confirmation
- Blocking variant in `close()`: wait up to 500ms for NC window to disappear

**NC ready wait (performance-oracle):**
- After open, poll for AX tree population (notification children exist)
- Max wait: 2 seconds with 50ms polling interval
- NC animation is typically 200-400ms

**Already-open handling:** If NC was already open when `NcSession::open()` is called, the session skips both the open and the close steps. The agent (or user) is responsible for NC in this case.

**SIGKILL risk (security-sentinel MEDIUM):**
- If the process is killed during a notification operation, NC may remain open
- No mitigation possible — same risk as any system UI interaction
- NC will close when the user clicks elsewhere or presses Escape
- With `panic = "abort"`, Drop does NOT run — NC will be left open on panic
- Acceptable: NC auto-closes on user interaction, and this is an edge case

**Success criteria:**
- `NcSession::open()` reliably detects whether NC is already open
- NC is always closed on session drop (verified by test that opens, panics, and checks NC state)
- `close(self)` provides error feedback without double-close
- Animation timing is handled (no empty traversals due to reading mid-animation)
- No AppleScript anywhere in the notification module

---

#### Phase 3: List Implementation (macOS)

The core traversal that all other notification commands depend on.

**Tasks:**

- [x] Create `crates/macos/src/notifications/list.rs` — AX tree traversal of NC
- [x] Wire `list_notifications` in macOS adapter

**`list.rs` implementation:**

The function receives an open `NcSession` reference (to ensure NC is open) and a `NotificationFilter`:

```rust
pub fn list_notifications(filter: &NotificationFilter) -> Result<Vec<NotificationInfo>, AdapterError> {
    let session = NcSession::open()?;
    let notifications = list_from_nc(&session, filter)?;
    session.close()?; // explicit close with error reporting
    Ok(notifications)
}
```

### Research Insights — AX Tree Traversal

**Heuristic matching strategy (best-practices-researcher):**

The NC AX tree structure varies by macOS version. Use role + subrole heuristics:

1. Find NC process by bundle ID `com.apple.notificationcenterui`
2. Get application AX element via `AXUIElementCreateApplication(pid)`
3. Find main NC window (AXWindow or AXSheet child)
4. Traverse children looking for notification elements:
   - **Sonoma (14):** Notifications are AXGroup with subrole `AXNotificationCenterAlert` or within `AXNotificationCenterAlertStack`
   - **Sequoia (15):** Extra AXGroup nesting level — notifications may be nested one level deeper
   - Match by role pattern: AXGroup containing AXStaticText children
   - Notification titles: AXStaticText with `AXValue` attribute
   - Action buttons: AXButton children within the notification group
5. Build flat list ordered by tree position (newest first in NC)
6. Apply filters during traversal, not after (performance-oracle: avoid building full list then filtering)
   - `--app`: case-insensitive substring match
   - `--text`: substring match on title+body
   - `--limit`: stop traversal early when limit reached

**Performance (performance-oracle):**
- Use `AXUIElementCopyMultipleAttributeValues` for batch attribute fetch (3-5x faster than individual calls)
- Apply `--app` filter during traversal to skip non-matching groups entirely
- If `--limit` is set, stop traversal once limit is reached

**Grouped notification handling — DEFERRED to v2 (code-simplicity-reviewer):**

macOS groups notifications by app when count > 1. V1 behavior:
- If a group is collapsed, list the group header only (showing app name + count badge text)
- Do NOT auto-expand groups — this adds complexity, timing issues, and visual disruption
- Include a `"grouped": true` hint on collapsed group entries so agents know to use `--app` filter for details
- V2 can add `--expand` flag to auto-expand groups before traversal

**Timestamp extraction:**

macOS NC shows relative timestamps ("2m ago"). The AX tree may expose:
- `AXValue` on the timestamp element → relative string
- `AXDescription` or custom attributes → sometimes absolute time

Strategy: extract whatever is available. If an absolute time can be parsed, return ISO 8601. If only relative, return `None` for `timestamp`.

**Focus mode / DND detection — DEFERRED to v2 (code-simplicity-reviewer):**

Reading focus mode requires CoreFoundation preferences API which adds complexity. V1 returns notifications only. V2 can add `focus_mode` metadata.

**NSConcreteAttributedString handling (best-practices-researcher):**

On Sequoia, notification body text may be `NSConcreteAttributedString` instead of plain NSString. Use `AXValueAttribute` with string coercion:
```rust
// Try AXValue as string first, then AXTitle, then AXDescription
// If the value is an attributed string, CFStringGetCString will still extract the text content
```

**Success criteria:**
- Returns correct flat list for NC with 0, 1, 5, 20+ notifications
- Filters work correctly (--app, --text, --limit)
- Returns empty list (not error) when NC has zero notifications
- Handles both Sonoma and Sequoia AX tree layouts
- Uses batch attribute fetch for performance
- Stops traversal early when --limit reached

---

#### Phase 4: Dismiss and Interact Implementation (macOS)

**Tasks:**

- [x] Create `crates/macos/src/notifications/actions.rs` — dismiss + interact combined
- [x] Handle hover-to-reveal close button (Sequoia behavior)
- [x] Wire `dismiss_notification` and `notification_action` in macOS adapter

### Research Insights — TOCTOU Mitigation

**Content-based verification (agent-native-reviewer CRITICAL):**

Positional indices are inherently racy — a notification may arrive or disappear between `list-notifications` and `dismiss-notification`. Within a single command invocation, the index is stable (NC is open and frozen). But across separate invocations, the index may point to a different notification.

**Mitigation: Verify before acting.** When the agent provides an index from a previous `list-notifications` call:
1. Open NC session
2. Re-list notifications to get current state
3. Find notification at the given index
4. Verify that `app_name` and `title` match what the agent expects (passed via optional `--verify-app` and `--verify-title` args, or by the command handler comparing against what list returned)
5. If mismatch → return `NOTIFICATION_NOT_FOUND` with suggestion to re-list
6. If match → perform the action

**V1 approach:** Within a single command (dismiss/interact), the tool lists and acts in the same NC session. The index is consistent within that session. The TOCTOU risk is only when the agent uses an index from a **previous** `list-notifications` call — but since agents should always list-then-act in quick succession, this is acceptable for v1. Content verification can be added in v2 via optional verify args.

**`actions.rs` — single notification dismiss:**

```rust
pub fn dismiss_notification(index: usize, app_filter: Option<&str>) -> Result<NotificationInfo, AdapterError> {
    let session = NcSession::open()?;
    let notifications = list_from_nc(&session, &build_filter(app_filter))?;
    let target = notifications.get(index - 1)  // 1-based to 0-based (index >= 1 validated by clap)
        .ok_or_else(|| AdapterError::notification_not_found(index))?;
    let info = target.clone();

    // Hover over notification to reveal close button (Sequoia)
    hover_over_element(&target.ax_handle)?;
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Find and click the close/dismiss button
    perform_dismiss_on_element(&target.ax_handle)?;
    session.close()?;
    Ok(info)
}
```

**"Clear All" hover requirement (best-practices-researcher, SpecFlow Gap 11):**

On macOS Sonoma/Sequoia, "Clear" and "Clear All" buttons appear only on hover. The implementation must:
1. Find the notification group header AX element
2. Get its bounds via `kAXPositionAttribute` + `kAXSizeAttribute`
3. Synthesize a `CGEventCreateMouseEvent` mouseMoved to the center of those bounds
4. Wait 200ms for the button to appear
5. Re-traverse to find the now-visible "Clear" / "Clear All" button
6. AXPress the button

**`actions.rs` — notification action:**

```rust
pub fn notification_action(index: usize, action_name: &str) -> Result<ActionResult, AdapterError> {
    let session = NcSession::open()?;
    let notifications = list_from_nc(&session, &NotificationFilter::default())?;
    let target = notifications.get(index - 1)
        .ok_or_else(|| AdapterError::notification_not_found(index))?;
    // Find action button matching action_name among the notification's AXButton children
    let button = find_action_button(&target.ax_handle, action_name)
        .ok_or_else(|| AdapterError::action_failed(
            format!("Action '{action_name}' not found on notification {index}"),
        ))?;
    perform_ax_press(&button)?;
    session.close()?;
    Ok(ActionResult::new(action_name))
}
```

**Inline action UI — DEFERRED to v2 (code-simplicity-reviewer):**

Some actions (like "Reply") open an inline text field within NC. V1 does not detect or handle this — it just clicks the button and returns success. V2 can add `"inline_ui": true` to the result and allow the agent to type into the field.

**Success criteria:**
- Single dismiss by index works correctly
- Dismiss-all (via command handler composing list + dismiss) works with and without `--app`
- Hover-to-reveal close button works on Sequoia
- Notification action buttons are correctly discovered and clicked
- Returns `NOTIFICATION_NOT_FOUND` for invalid index
- Returns `ACTION_FAILED` if action button not found by name
- Index 0 is impossible (validated at parse time)

---

#### Phase 5: Wait Implementation (Core + macOS)

**Tasks:**

- [x] Extend the `wait` command handler in `crates/core/src/commands/wait.rs` for `--notification` flag
- [x] Implement polling loop using `list_notifications`
- [x] No new macOS files needed — wait composes on existing `list_notifications`

**Wait architecture (simplified from original plan):**

```
wait --notification --app "Slack" --timeout 10000
           │
           ▼
   ┌───────────────────┐
   │ AX polling loop    │
   │ every 3s (default) │
   │ or --poll-interval │
   │                    │
   │ Each cycle:        │
   │ 1. list_notifs()   │
   │ 2. compare baseline│
   │ 3. if new → return │
   │ 4. sleep interval  │
   └───────────────────┘
```

The wait command lives entirely in core (`wait.rs`), calling `adapter.list_notifications()` in a loop. No separate adapter method needed. No observer. No macOS-specific wait code.

**Polling strategy:**

```rust
// In wait.rs, when --notification flag is set:
let filter = NotificationFilter { app: args.app.clone(), text: args.text.clone(), ..Default::default() };
let baseline = adapter.list_notifications(&filter)?;
let baseline_count = baseline.len();
let interval = Duration::from_millis(args.poll_interval.unwrap_or(3000));
let deadline = Instant::now() + Duration::from_millis(args.timeout.unwrap_or(30000));

loop {
    std::thread::sleep(interval);
    if Instant::now() > deadline {
        return Err(AppError::from(AdapterError::timeout("notification")));
    }
    let current = adapter.list_notifications(&filter)?;
    if current.len() > baseline_count {
        // New notification arrived — return the newest one
        return Ok(json!({
            "condition": "notification",
            "matched": true,
            "notification": current[0],  // newest is index 1, which is current[0]
        }));
    }
}
```

### Research Insights — Wait

**Performance (performance-oracle):**
- Each poll cycle opens and closes NC (visual flash). 3-second default interval minimizes visual disruption.
- `--poll-interval` allows agents to tune: faster for time-critical workflows, slower for background monitoring.
- NC open/close overhead is ~300-500ms per cycle. With 3s interval, that's ~10-15% overhead — acceptable.

**`--notification` flag semantics (agent-native-reviewer):**
- `--notification` is mutually exclusive with `--element`, `--window`, `--menu`
- When `--notification` is set, `--app` filters by notification source app (NOT the app to snapshot — different semantics from `--element` mode)
- When `--notification` is set, `--text` filters by notification title/body content
- This semantic overload is documented in CLI help text

**Success criteria:**
- `wait --notification` blocks until a notification arrives
- `wait --notification --app "Messages"` only matches Messages notifications
- `wait --notification --text "hello"` matches title or body content
- Timeout returns structured `TIMEOUT` error
- `--poll-interval` controls polling frequency
- NC is properly opened/closed each cycle

---

#### Phase 6: Testing

**Tasks:**

- [x] Unit tests for `NotificationInfo` serialization (core)
- [x] Unit tests for `NotificationFilter` logic (core)
- [x] Unit tests for new error code (core)
- [ ] MockAdapter tests for notification commands (core) — deferred, no MockAdapter infra yet
- [ ] Integration tests for NC session lifecycle (macOS CI) — requires CI runner with Accessibility
- [ ] Integration tests for list/dismiss/interact (macOS CI) — requires CI runner with Accessibility
- [ ] Golden fixture for NC AX tree structure (tests/fixtures/) — deferred to v2

**Unit tests (core — `crates/core/`):**

```rust
// notification.rs tests
#[test]
fn notification_info_serialization_omits_none_fields() { ... }

#[test]
fn notification_info_serialization_omits_empty_actions() { ... }

#[test]
fn notification_filter_default_is_unfiltered() { ... }

// error.rs tests
#[test]
fn notification_not_found_error_serialization() { ... }

// commands/ tests (using MockAdapter)
#[test]
fn list_notifications_returns_empty_on_mock() { ... }

#[test]
fn dismiss_notification_returns_not_supported_on_mock() { ... }

#[test]
fn dismiss_all_composes_list_and_dismiss() { ... }
```

**Integration tests (macOS CI — `tests/integration/`):**

These require a macOS runner with Accessibility permission:

```rust
#[test]
fn list_notifications_returns_valid_json() {
    // Run: agent-desktop list-notifications
    // Verify: valid JSON envelope with "ok": true
    // Verify: "notifications" is an array
    // Verify: each notification has required fields (index, app_name, title)
}

#[test]
fn dismiss_notification_invalid_index_returns_error() {
    // Run: agent-desktop dismiss-notification 999
    // Verify: NOTIFICATION_NOT_FOUND error
}

#[test]
fn dismiss_notification_zero_index_rejected() {
    // Run: agent-desktop dismiss-notification 0
    // Verify: parse error (exit code 2), not panic
}

#[test]
fn list_notifications_with_app_filter() {
    // Run: agent-desktop list-notifications --app "NonexistentApp"
    // Verify: empty notifications array, ok: true
}
```

**Golden fixtures:**

Capture real NC AX tree snapshots and commit to `tests/fixtures/`:
- `notification_center_macos14.json` — Sonoma layout
- `notification_center_macos15.json` — Sequoia layout (with extra nesting)

### Research Insights — Testing

**Security testing (security-sentinel):**
- Add test for index 0 rejection (parse error, not panic)
- Add tracing assertions: notification content reads are logged at `info` level for audit trail
- Test concurrent NC access: two simultaneous list calls should not corrupt each other

**Performance benchmarks (performance-oracle):**
- NC open-to-first-result: target < 1 second
- List 20 notifications: target < 2 seconds
- Single dismiss: target < 1.5 seconds (includes hover delay)

---

## System-Wide Impact

### Interaction Graph

1. Agent calls `list-notifications` → dispatch.rs → list_notifications::execute() → adapter.list_notifications() → macOS: NcSession::open() → AX tree traversal → NcSession::close()
2. Agent calls `dismiss-notification 3` → dispatch.rs → dismiss_notification::execute() → adapter.dismiss_notification() → macOS: NcSession::open() → list + find by index → hover → AXPress close button → NcSession::close()
3. Agent calls `wait --notification` → dispatch.rs → wait::execute() → polling loop calling adapter.list_notifications() every N seconds

No callbacks, middleware, or observers fire beyond the AX system. The only external side effect is the NC open/close animation visible to the user.

### Error Propagation

```
macOS AX API error
  → AdapterError (with ErrorCode, message, suggestion, platform_detail)
    → AppError::Adapter (via #[from])
      → JSON error envelope (main.rs)
        → exit code 1
```

NC close failure in `NcSession::drop()` is logged via `tracing::warn` but does not propagate (Drop cannot return errors). This is acceptable because:
- The primary operation already succeeded or failed with a proper error
- NC will auto-close after user interaction or on next session open

### State Lifecycle Risks

- **NC left open on crash:** If the process is killed (SIGKILL) or panics (panic=abort), NC may remain open. No mitigation — same risk as any system UI interaction. NC will close when the user clicks elsewhere.
- **RefMap interaction:** Notification commands do NOT interact with the RefMap. They use their own index scheme. No risk of corrupting snapshot refs.
- **Concurrent access:** Two simultaneous `list-notifications` calls will each open their own NC session. Since NC is a singleton UI, the second open attempt will find NC already open and skip the open step (via `NcSession::was_already_open`). Both traversals will read the same AX tree. This is safe but may produce interleaved results in edge cases.

### API Surface Parity

- Notification commands are new — no existing interfaces expose equivalent functionality
- The `snapshot` command will NOT snapshot NC content (NC is a system process, not a user app). The notification commands are the only way to access NC content.
- Batch dispatch (`batch` command) must handle all 4 new commands
- **Note:** `typed batch path` is at 472 LOC (over 400 limit) — must split before adding 4 new arms (pattern-recognition-specialist)

### Research Insights — Security

**Notification content sensitivity (security-sentinel CRITICAL):**

Notifications may contain sensitive data: 2FA codes, private messages, financial alerts, medical reminders. The tool faithfully returns this content — it's the agent's responsibility to handle it appropriately.

**Mitigations:**
- Add `tracing::info!` logging when notifications are read (audit trail, not prevention)
- Documentation should warn: "Notification content may include sensitive information (2FA codes, private messages). Agents should not log, store, or transmit notification content without user consent."
- RefMap-style file permissions (`0o600`) not needed — notification data is transient, not persisted to disk

**AppleScript injection (security-sentinel HIGH):**
- NC open uses pure AX, not AppleScript — no injection vector
- Existing `app_ops.rs:172-192` has the vulnerability but is out of scope for this PR
- Flag for separate security fix PR

---

## Acceptance Criteria

### Functional Requirements

- [ ] `list-notifications` returns a flat JSON array of notifications from NC
- [ ] `list-notifications --app "Messages"` filters to Messages notifications only
- [ ] `list-notifications --limit 5` caps results at 5
- [ ] `dismiss-notification 2` dismisses the 2nd notification and returns its info
- [ ] `dismiss-all-notifications` clears all notifications from NC
- [ ] `dismiss-all-notifications --app "Mail"` clears only Mail notifications
- [ ] `notification-action 1 "Reply"` clicks the Reply button on notification 1
- [ ] `wait --notification --timeout 5000` blocks until a notification arrives or times out
- [ ] `wait --notification --app "Slack"` only matches Slack notifications
- [ ] All commands return valid JSON envelopes matching the existing output contract
- [ ] All commands work when NC starts closed (auto-open) and when NC starts open (skip open)
- [ ] NC is always closed after command completes (verified on success and error paths)
- [ ] Index 0 is rejected at parse time with clear error (not panic)
- [ ] No AppleScript used anywhere in notification module

### Non-Functional Requirements

- [ ] NC open-to-first-result latency < 1 second (excluding NC animation)
- [ ] No new dependencies added to core crate (notification types are pure serde structs)
- [ ] Binary size increase < 50KB (notification code is thin AX wrappers)
- [ ] `cargo tree -p agent-desktop-core` still contains zero platform crate names

### Quality Gates

- [ ] `cargo clippy --all-targets -- -D warnings` — zero warnings
- [ ] `cargo test --lib --workspace` — all tests pass
- [ ] `cargo fmt --all -- --check` — formatted
- [ ] New commands follow existing patterns exactly (file structure, naming, error handling)
- [ ] No `unwrap()` in non-test code
- [ ] All files under 400 LOC
- [ ] `typed batch path` split before adding new commands (if currently over 400 LOC)

---

## Success Metrics

- Agents can detect, read, and dismiss notifications in automated workflows
- `list-notifications` on a typical NC (5-15 notifications) completes in < 2 seconds
- `wait --notification` detects new notifications within 5 seconds (one poll cycle + overhead)
- Zero regressions in existing 50-command test suite

## Dependencies & Prerequisites

- macOS 14+ (Sonoma) — primary target. macOS 15 (Sequoia) tested with extra nesting handling. Ventura (13) best-effort.
- Accessibility permission already granted (same as all existing commands)
- No new crate dependencies — uses existing `accessibility-sys`, `core-foundation`, `core-graphics` FFI

## Risk Analysis & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| NC AX tree structure changes across macOS versions | High | High | Heuristic matching by role/subrole patterns (`AXNotificationCenterAlert`, `AXNotificationCenterAlertStack`), not hardcoded paths. Golden fixtures for Sonoma + Sequoia. |
| Sequoia extra nesting level breaks parser | High | High | Variable-depth traversal with recursive heuristic matching. Test on both macOS 14 and 15. |
| "Clear All" / close buttons hidden until hover | High | Medium | Synthesize mouse hover event before looking for button. 200ms wait for button to appear. |
| Collapsed notification groups return headers not individual notifications | Medium | Medium | V1: return group header with `"grouped": true` hint. V2: add `--expand` flag. |
| NC animation timing causes empty/partial reads | Medium | Medium | Wait for AX tree population after open (poll for children, max 2s, 50ms interval). |
| NC left open after error/crash | Low | Medium | RAII NcSession guard with explicit close() + Drop fallback. `panic=abort` means Drop won't run on panic — acceptable risk. |
| 3-second poll interval too slow for wait | Low | Medium | `--poll-interval` flag allows agents to tune. Default is conservative. |
| typed batch path over 400 LOC | High | Low | Split typed batch path before adding notification commands. |

## Documentation Plan

- [ ] Update `docs/phases.md` — mark notification commands as implemented for macOS
- [ ] Update `README.md` — add notification commands to command reference table
- [ ] Update `.claude/skills/agent-desktop/` — add notification command documentation
- [ ] Add golden fixtures to `tests/fixtures/` for NC AX tree structure (Sonoma + Sequoia)
- [ ] Add security note to docs: notification content may contain sensitive data

## V2 Backlog (Deferred from v1)

Items explicitly deferred during deepening to keep v1 minimal:

| Item | Source | Rationale for deferral |
|------|--------|----------------------|
| NSDistributedNotificationCenter observer | code-simplicity, security-sentinel | Thread-safety risk, undocumented API. Polling is reliable. |
| Focus mode / DND detection | code-simplicity | Requires CoreFoundation preferences API. Not essential for core notification ops. |
| Inline action UI detection | code-simplicity | "Reply" text field handling is edge case. V1 clicks button and returns success. |
| Auto-expansion of collapsed groups | code-simplicity | Adds timing complexity and visual disruption. V1 returns group headers. |
| Content-based TOCTOU verification | agent-native-reviewer | `--verify-app`/`--verify-title` args for cross-invocation index safety. V1 is safe within single invocation. |
| `total_count` + `has_more` in list response | agent-native-reviewer | Useful for pagination. V1 returns all matching (with --limit). |
| `click-notification` body action (open source app) | agent-native-reviewer | Different from action button click. Needs research on AX default action. |
| `--poll-interval` auto-tuning | performance-oracle | Adaptive interval based on NC change frequency. V1 uses fixed interval. |

---

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-02-27-notification-management-brainstorm.md](../brainstorms/2026-02-27-notification-management-brainstorm.md) — Key decisions carried forward: macOS first, AX-based approach, positional index identification, flat list (no grouping), NC-only (no banners), heuristic AX matching

### Internal References

- Adapter trait pattern: `crates/core/src/adapter.rs:114-216`
- Domain type pattern (SurfaceInfo): `crates/core/src/node.rs:84-92`
- Command handler pattern: `crates/core/src/commands/clipboard_get.rs`, `list_surfaces.rs`
- Error code pattern: `crates/core/src/error.rs:4-18`, `53-116`
- macOS adapter delegation: `crates/macos/src/adapter.rs`
- cfg-gated imp pattern: `crates/macos/src/input/clipboard.rs`
- Wait command: `crates/core/src/commands/wait.rs:12-55`
- AppleScript pattern (has injection vulnerability): `crates/macos/src/system/app_ops.rs:157-196`
- CLI registration: `src/cli.rs`, `src/cli_args.rs`, `src/dispatch.rs`
- AX-first activation pattern: `docs/brainstorms/2026-02-23-macos-ax-first-robustness-brainstorm.md`

### Review Agent Sources

- **architecture-strategist:** Incremental CLI wiring, return type consistency, method count reduction
- **performance-oracle:** Batch AX attribute fetch, early filter application, fire-and-forget close, AX timeout setting
- **security-sentinel:** Index 0 crash, AppleScript injection, notification content sensitivity, observer thread safety
- **code-simplicity-reviewer:** Observer removal, adapter method reduction, v2 backlog items
- **pattern-recognition-specialist:** Missing derives, --app on dismiss, naming conventions, typed batch path LOC
- **agent-native-reviewer:** TOCTOU verification, total_count/has_more, click-notification-body, poll-interval
- **best-practices-researcher:** NC process name, Sequoia nesting, hover-only buttons, attributed strings, RAII patterns

### Files to Create

| File | Purpose | Est. LOC |
|------|---------|----------|
| `crates/core/src/notification.rs` | NotificationInfo, NotificationFilter structs | ~40 |
| `crates/core/src/commands/list_notifications.rs` | list-notifications command handler | ~30 |
| `crates/core/src/commands/dismiss_notification.rs` | dismiss-notification command handler | ~30 |
| `crates/core/src/commands/dismiss_all_notifications.rs` | dismiss-all-notifications command handler | ~40 |
| `crates/core/src/commands/notification_action.rs` | notification-action command handler | ~30 |
| `crates/macos/src/notifications/mod.rs` | Module re-exports | ~15 |
| `crates/macos/src/notifications/nc_session.rs` | RAII NC lifecycle guard | ~120 |
| `crates/macos/src/notifications/list.rs` | AX tree traversal of NC | ~200 |
| `crates/macos/src/notifications/actions.rs` | Dismiss + interact combined | ~150 |

**Total new code: ~655 LOC** (reduced from ~840 in original plan by removing observer.rs and merging dismiss+interact)

### Files to Modify (Registration Points Only)

| File | Change |
|------|--------|
| `crates/core/src/lib.rs` | `pub mod notification;` + re-export |
| `crates/core/src/adapter.rs` | +3 trait methods with `not_supported` defaults |
| `crates/core/src/error.rs` | +1 ErrorCode variant + constructor |
| `crates/core/src/commands/mod.rs` | +4 `pub mod` declarations |
| `crates/core/src/commands/wait.rs` | +`notification` field on WaitArgs + handler branch |
| `crates/macos/src/lib.rs` | `pub mod notifications;` |
| `crates/macos/src/adapter.rs` | +3 trait method implementations |
| `src/cli.rs` | +4 Commands variants + name() arms |
| `src/cli_args.rs` | +4 arg structs + notification/poll-interval fields on WaitArgs |
| `src/dispatch.rs` | +4 match arms + extend Wait arm |
| `src/typed batch path` | +4 command routing arms (split file first if over 400 LOC) |
