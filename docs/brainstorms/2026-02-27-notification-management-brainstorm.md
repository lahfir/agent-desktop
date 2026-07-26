# Notification Management — macOS First

> Brainstorm date: 2026-02-27
> Status: Draft
> Scope: macOS notification commands (list, dismiss, dismiss-all, action, wait)
> Reference: [Phase Roadmap — Notification section](../phases.md)

---

## What We're Building

A set of notification management commands for agent-desktop, starting with macOS. These commands let AI agents read, dismiss, and interact with OS-level notifications — a capability gap in the current 50-command surface.

**New commands (5 touch points):**

| Command | Description | Args |
|---------|-------------|------|
| `list-notifications` | List current notifications with app, title, body, timestamp, actions | `--app`, `--limit` |
| `dismiss-notification` | Dismiss a specific notification by positional index | `<index>` |
| `dismiss-all-notifications` | Clear all notifications, optionally filtered by app | `--app` |
| `notification-action` | Click an action button on a notification | `<index> <action-name>` |
| `wait --notification` | Wait for a notification to appear (event-driven) | `--app`, `--text`, `--timeout` |

**New domain type:**

```rust
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
```

Note: `NotificationInfo` has 6 fields (under the 7-field limit). `is_persistent` from the phases.md spec was dropped — it's not reliably detectable from the AX tree and adds no value for the agent.

**New adapter trait methods (3):**

```rust
fn list_notifications(&self, filter: &NotificationFilter) -> Result<Vec<NotificationInfo>, AdapterError>;
fn dismiss_notification(&self, index: usize, app_filter: Option<&str>) -> Result<(), AdapterError>;
fn interact_notification(&self, index: usize, action_name: &str) -> Result<ActionResult, AdapterError>;
```

**New error codes (2):**

- `NOTIFICATION_NOT_FOUND` — Index out of bounds or notification no longer exists
- `NOTIFICATION_UNSUPPORTED` — Platform doesn't support notification management (stub adapters)

---

## Why This Approach

**Approach chosen: AX tree traversal + NSDistributedNotificationCenter observer**

### CRUD Operations — AX Tree

All list/dismiss/interact operations work by programmatically opening Notification Center, traversing its accessibility tree, and performing actions via AXPress.

**How it works:**

1. **Open NC silently:** Use AXUIElement targeting the `com.apple.notificationcenterui` process, or AppleScript `tell application "System Events"` to click the NC clock/date area in the menu bar. The tool opens NC automatically — agents don't need to do this manually.

2. **Traverse NC AX tree:** The Notification Center window exposes notifications as AXGroup children, each containing:
   - AXStaticText elements for title and body
   - AXButton elements for actions ("Reply", "Open", app-specific actions)
   - AXButton for dismiss/close

3. **Identify by positional index:** Notifications are numbered 1-based from top to bottom (newest first). The agent calls `list-notifications` to see the current state, then uses the index for dismiss/interact. This mirrors the snapshot → ref → action pattern — the index is snapshot-scoped, not stable across calls.

4. **Close NC after operation:** After reading or acting, close NC to restore the user's screen state. Use AXPress on the NC toggle or send Escape key.

**Why not other approaches:**
- **SQLite DB approach** requires Full Disk Access permission — unacceptable adoption barrier on top of the existing Accessibility permission requirement.
- **Pure polling for wait** wastes resources and has minimum ~500ms granularity due to NC open/close visual flicker.

### Wait — NSDistributedNotificationCenter

`wait --notification` uses `NSDistributedNotificationCenter` to listen for system-level notification events. This is event-driven — instant detection, no polling, no visual impact.

**How it works:**

1. Register an observer on `NSDistributedNotificationCenter.defaultCenter()` for notification-related events
2. When a notification arrives, check against the filter criteria (`--app`, `--text`)
3. If matched, open NC briefly to read full notification details and return them
4. If `--timeout` exceeded, return a TIMEOUT error

**Fallback:** If the distributed notification mechanism fails (undocumented API changed in a new macOS version), fall back to AX polling with a 1-second interval.

---

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Platform scope | macOS first | Matches Phase 1 additive pattern. Windows/Linux follow in their respective phases. |
| NC access | Auto-open silently | Agents shouldn't need to manually open NC. The tool handles it transparently. |
| Notification identity | Positional index (1-based) | No stable IDs exist. Index is simple, matches the snapshot-scoped ref pattern. Agent must list before acting. |
| Wait mechanism | NSDistributedNotificationCenter + AX fallback | Event-driven for responsiveness. Polling fallback for resilience. |
| System tray | Separate feature | Different AX targets, different design questions. Brainstorm independently. |
| `is_persistent` field | Dropped | Not reliably detectable from AX tree. No agent use case identified. |
| `NotificationFilter` | App name + text content | `--app` filters by source app, `--text` filters by title/body substring. `--limit` caps result count. |
| DND detection | Include as metadata | `list-notifications` response includes a `do_not_disturb: bool` field so agents know if notifications are suppressed. Read via CoreFoundation preferences. |
| NC close behavior | Always close after operation | Prevents NC from staying open and blocking the user's view. Brief flash is acceptable. |

---

## Resolved Questions

1. **AX tree structure stability:** Use heuristic role/attribute matching (find AXGroup with AXStaticText children) rather than hardcoded tree paths. More resilient to macOS version changes. No version-specific parsers.

2. **Grouped notifications:** Flat list ordered by recency. App name is on each notification. Agents don't need to know about NC's grouping structure.

3. **Notification banners vs NC:** NC only. Banners are transient and require a separate AX tree target. Keeping scope to Notification Center contents is sufficient for v1.

---

## JSON Output Examples

### list-notifications

```json
{
  "version": "1.0",
  "ok": true,
  "command": "list-notifications",
  "data": {
    "count": 3,
    "do_not_disturb": false,
    "notifications": [
      {
        "index": 1,
        "app_name": "Messages",
        "title": "John Doe",
        "body": "Hey, are you free for lunch?",
        "timestamp": "2026-02-27T10:30:00",
        "actions": ["Reply", "Open"]
      },
      {
        "index": 2,
        "app_name": "Calendar",
        "title": "Team Standup in 15 minutes",
        "actions": ["Join", "Snooze"]
      },
      {
        "index": 3,
        "app_name": "Slack",
        "title": "#engineering",
        "body": "New message from Alice",
        "actions": ["Open"]
      }
    ]
  }
}
```

### dismiss-notification

```json
{
  "version": "1.0",
  "ok": true,
  "command": "dismiss-notification",
  "data": {
    "dismissed_index": 2,
    "app_name": "Calendar",
    "title": "Team Standup in 15 minutes"
  }
}
```

### notification-action

```json
{
  "version": "1.0",
  "ok": true,
  "command": "notification-action",
  "data": {
    "index": 1,
    "app_name": "Messages",
    "action": "Reply",
    "result": "performed"
  }
}
```

### wait --notification (success)

```json
{
  "version": "1.0",
  "ok": true,
  "command": "wait",
  "data": {
    "condition": "notification",
    "matched": true,
    "notification": {
      "index": 1,
      "app_name": "Messages",
      "title": "John Doe",
      "body": "Hey, are you free for lunch?",
      "actions": ["Reply", "Open"]
    }
  }
}
```

---

## macOS Implementation Outline

### New files

```
crates/core/src/
├── notification.rs                          # NotificationInfo, NotificationFilter structs

crates/core/src/commands/
├── list_notifications.rs                    # list-notifications handler
├── dismiss_notification.rs                  # dismiss-notification handler
├── dismiss_all_notifications.rs             # dismiss-all-notifications handler
└── notification_action.rs                   # notification-action handler

crates/macos/src/notifications/
├── mod.rs                                   # re-exports
├── list.rs                                  # Open NC → traverse AX tree → parse notifications → close NC
├── dismiss.rs                               # Open NC → find by index → AXPress close button → close NC
├── interact.rs                              # Open NC → find by index → find action button → AXPress → close NC
├── nc_control.rs                            # Open/close Notification Center helpers
└── observer.rs                              # NSDistributedNotificationCenter observer for wait
```

### Registration points (existing files to modify)

- `crates/core/src/lib.rs` — add `pub mod notification`
- `crates/core/src/adapter.rs` — add 3 trait methods with `not_supported` defaults
- `crates/core/src/error.rs` — add `NotificationNotFound`, `NotificationUnsupported` variants
- `crates/core/src/commands/mod.rs` — add 4 `pub mod` declarations
- `crates/macos/src/lib.rs` — add `pub mod notifications`
- `crates/macos/src/adapter.rs` — implement 3 trait methods delegating to notifications module
- `src/cli.rs` — add 4 command variants
- `src/cli_args.rs` — add arg structs
- `src/dispatch.rs` — add 4 match arms
- `src/dispatch.rs` — extend `wait` command match arm for `--notification` flag
