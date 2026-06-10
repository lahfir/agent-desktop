# System Commands

App lifecycle, window management, notifications, clipboard, wait, and system health commands.

## App Lifecycle

### launch
```bash
agent-desktop launch "System Settings"
agent-desktop launch "com.apple.Safari" --timeout 10000
```
Launches an application by name or bundle ID and waits until its window is visible.

| Flag | Default | Description |
|------|---------|-------------|
| `--timeout` | 30000 | Max wait time in ms for window to appear |

### close-app
```bash
agent-desktop close-app "TextEdit"
agent-desktop close-app "TextEdit" --force
```
Requests an application quit. A graceful quit is asynchronous — the app may show an unsaved-changes dialog or refuse — so the response reports only what was guaranteed, never a termination it cannot confirm without blocking:

- Graceful: `{ "app": "TextEdit", "method": "graceful", "requested": true }`. The quit was sent. To confirm it actually closed, observe with `list-apps` or `wait --window`; if a save dialog appears, `snapshot` it and click the choice (`find --role button --name Delete`).
- `--force`: `{ "app": "TextEdit", "method": "force", "requested": true, "closed": true }`. A forced kill is synchronous, so termination is assured.

### list-apps
```bash
agent-desktop list-apps
agent-desktop list-apps --app "Text"
```
Lists running GUI applications, optionally filtered by a case-insensitive name substring. Returns array of `{ name, pid, bundle_id }`.

## Window Management

### list-windows
```bash
agent-desktop list-windows
agent-desktop list-windows --app "Finder"
```
Lists all visible windows, optionally filtered by app. Returns array of `{ id, title, app_name, pid, bounds, is_focused }`. Focus is detected through the platform's frontmost/focused-window APIs, not window stacking order.

### focus-window
```bash
agent-desktop focus-window --app "Finder"
agent-desktop focus-window --title "Documents"
agent-desktop focus-window --window-id "w-4521"
```
Brings a window to the front and confirms the OS reports that same window as focused. At least one identifier is required. If focus does not settle before the deadline, the command returns `ACTION_FAILED` instead of fabricating a focused result.

### resize-window
```bash
agent-desktop resize-window --app "TextEdit" --width 800 --height 600
```

### move-window
```bash
agent-desktop move-window --app "TextEdit" --x 0 --y 0
```

### minimize
```bash
agent-desktop minimize --app "TextEdit"
```

### maximize
```bash
agent-desktop maximize --app "TextEdit"
```
Zooms the window to fill the screen.

### restore
```bash
agent-desktop restore --app "TextEdit"
```
Restores a minimized or maximized window to its previous size.

## Notifications

### list-notifications
```bash
agent-desktop list-notifications
agent-desktop list-notifications --app "Slack"
agent-desktop list-notifications --text "deploy" --limit 5
```
Lists notifications in the Notification Center. Returns array of `{ index, app_name, title, body, actions }`.

| Flag | Default | Description |
|------|---------|-------------|
| `--app` | | Filter by source app name |
| `--text` | | Filter by text content (matches title and body) |
| `--limit` | | Max number of notifications to return |

### dismiss-notification
```bash
agent-desktop dismiss-notification 1
agent-desktop dismiss-notification 3 --app "Slack"
```
Dismisses a single notification by its 1-based index. Returns the dismissed notification info.

| Flag | Default | Description |
|------|---------|-------------|
| (positional) | | 1-based notification index (required) |
| `--app` | | Filter by app before indexing |

### dismiss-all-notifications
```bash
agent-desktop dismiss-all-notifications
agent-desktop dismiss-all-notifications --app "Slack"
```
Dismisses all notifications, optionally filtered by app. Reports per-notification failures.

Returns `{ "dismissed_count": N, "failures": [...], "failed_count": N }`.

### notification-action
```bash
agent-desktop notification-action 1 "Reply"
agent-desktop notification-action 2 "Mark as Read" --expected-app Slack --expected-title "#general"
```
Clicks a named action button on a notification by its 1-based index.

`--expected-app` and `--expected-title` pin the call to the notification
you observed in `list-notifications`. Notification Center reorders
entries between listings, so without a fingerprint an arriving or
dismissed notification can shift the target at `INDEX` and cause the
action to press the wrong row. When either flag is set and the row at
`INDEX` no longer matches, the call fails with `NOTIFICATION_NOT_FOUND`
instead of pressing. Both flags omitted preserves the legacy
index-only behavior for callers that reconcile themselves.

| Flag | Default | Description |
|------|---------|-------------|
| `INDEX` (positional) | | 1-based notification index (required) |
| `ACTION` (positional) | | Action button name to click (required) |
| `--expected-app` | | Fingerprint app name (from `list-notifications`) |
| `--expected-title` | | Fingerprint title (from `list-notifications`) |

### wait --notification
```bash
agent-desktop wait --notification --app "App" --timeout 10000
agent-desktop wait --notification --text "build passed" --timeout 15000
```
Blocks until a new notification appears (detects index-diff from a baseline captured at wait start). Supports `--app` and `--text` filters. Transient Notification Center errors (timeouts, element-not-found) are retried within the `--timeout` budget for both the baseline capture and polling; permanent errors (for example `PERM_DENIED`) fail immediately. Timeout errors include a `last_error` detail with the most recent transient failure.

## Clipboard

### clipboard-get
```bash
agent-desktop clipboard-get
```
Returns `{ "data": { "text": "clipboard contents" } }`.

### clipboard-set
```bash
agent-desktop clipboard-set "Hello, world!"
```

### clipboard-clear
```bash
agent-desktop clipboard-clear
```

## Wait

### wait (time)
```bash
agent-desktop wait 1000
```
Pauses for N milliseconds. Use between actions that need time to settle.

### wait (element)
```bash
agent-desktop wait --element @e5 --snapshot <snapshot_id> --timeout 5000 --app "App"
agent-desktop wait --element @e5 --predicate actionable --timeout 5000
agent-desktop wait --element @e5 --predicate actionable --action type --timeout 5000
agent-desktop wait --element @e5 --predicate value --value "Done" --timeout 5000
```
Blocks until the element ref appears in the accessibility tree. Useful after triggering UI changes.
When `--snapshot` is omitted, the command polls the caller's latest session refmap and refreshes it on the built-in debounce. When `--snapshot` is passed, it resolves that pinned refmap directly. Element resolution is capped by the remaining `--timeout`, and timeout errors include the last observed predicate/actionability state.

`--predicate actionable` checks readiness for a specific action via `--action` (`click` default, `type`, `set-value`, `clear`). Use `--action type` before a wait-then-type flow: the editability check only runs for the editing actions, so the default click check can report ready on a field that cannot accept text.

### wait (window)
```bash
agent-desktop wait --window "Save As" --timeout 10000
```
Blocks until a window with the given title appears.

### wait (text)
```bash
agent-desktop wait --text "Loading complete" --app "Safari" --timeout 5000
```
Blocks until the specified text appears anywhere in the app's accessibility tree. The success body includes `count` only when `--count` is passed; without it, matching stops at the first hit and no count is reported.

### wait (menu)
```bash
agent-desktop wait --menu --app "Finder" --timeout 3000
```
Blocks until a menu surface is detected as open.

### wait (menu-closed)
```bash
agent-desktop wait --menu-closed --app "Finder" --timeout 3000
```
Blocks until the menu surface is dismissed.

| Flag | Default | Description |
|------|---------|-------------|
| (positional) | | Milliseconds to pause |
| `--element` | | Ref to wait for |
| `--snapshot` | latest | Snapshot ID for `--element` waits |
| `--predicate` | exists | Element predicate: `exists`, `enabled`, `visible`, `actionable`, `value` |
| `--value` | | Expected text for `--predicate value` |
| `--action` | click | Action checked by `--predicate actionable`: `click`, `type`, `set-value`, `clear` |
| `--count` | | Expected match count for `--text` waits |
| `--window` | | Window title to wait for |
| `--text` | | Text to wait for; with `--notification`, filters notification title/body |
| `--menu` | false | Wait for menu surface to open |
| `--menu-closed` | false | Wait for menu surface to close |
| `--notification` | false | Wait for a new notification |
| `--timeout` | 30000 | Timeout in ms (for element/window/text/menu waits) |
| `--app` | | Scope the wait to a specific application |

## Batch

### batch
```bash
agent-desktop batch '[{"command":"click","args":{"ref_id":"@e1","snapshot":"<snapshot_id>"}},{"command":"wait","args":{"ms":500}},{"command":"click","args":{"ref_id":"@e2","snapshot":"<snapshot_id>"}}]'
agent-desktop batch '[...]' --stop-on-error
agent-desktop --session run-a batch '[{"command":"status","session":"run-b","args":{}}]'
```
Execute multiple commands in sequence from a JSON array. Each entry has `command` (string) and `args` (object). Use `args`, not `params`. For ref-consuming commands, pass the output `snapshot_id` as the `snapshot` field.

Batch uses the same typed `Commands` enum, command policy preflight, permission report, and dispatch path as the CLI. Unknown fields are rejected instead of being silently ignored. Nested `batch` is rejected.

Each entry may include `"session": "id"` beside `command` and `args`. If omitted, the entry inherits the top-level `--session`. Use per-entry sessions only when intentionally inspecting or coordinating separate agent runs. Top-level `--trace` is inherited by every entry — including entries with a `session` override — so one JSONL file captures the whole batch.

| Flag | Default | Description |
|------|---------|-------------|
| `--stop-on-error` | false | Halt on first failed command |

**Batch format:**
```json
[
  { "command": "click", "args": { "ref_id": "@e1", "snapshot": "<snapshot_id>" } },
  { "command": "wait", "args": { "ms": 500 } },
  { "command": "type", "args": { "ref_id": "@e2", "snapshot": "<snapshot_id>", "text": "hello" } },
  { "command": "status", "session": "other-agent", "args": {} }
]
```

**Per-entry failure shape:**
```json
{
  "version": "2.0",
  "ok": false,
  "command": "click",
  "error": {
    "code": "STALE_REF",
    "message": "Ref '@e1' is stale",
    "suggestion": "Run snapshot again and retry with the new ref"
  }
}
```

**Progressive snapshot in batch** — use `skeleton` and `root` fields inside `snapshot` args:
```json
[
  { "command": "snapshot", "args": { "app": "Slack", "skeleton": true, "interactive_only": true } },
  { "command": "snapshot", "args": { "app": "Slack", "root": "@e3", "snapshot": "<snapshot_id>", "interactive_only": true } }
]
```

`skeleton: true` clamps depth to 3 and tags truncated containers with `children_count`. `root: "@eN"` starts traversal from that ref instead of the window root; it cannot be combined with `surface`.

## System Health

### status
```bash
agent-desktop status
```
Returns adapter health, platform info, permission report, and latest snapshot metadata (`snapshot_id`, `ref_count`) when available.

### permissions
```bash
agent-desktop permissions
agent-desktop permissions --request
```
Checks the cached per-process permission report: `accessibility`, `screen_recording`, and `automation`, each as `{ "state": "granted" }`, `{ "state": "denied", "suggestion": "..." }`, `{ "state": "not_required" }`, or `{ "state": "unknown" }`. The current macOS adapter reports concrete `granted` or `denied` states for Accessibility and Screen Recording, and `not_required` for Automation because shipped commands use Accessibility, Screen Recording for screenshots, and explicit keyboard/mouse input rather than Apple Events. Use `--request` to invoke the platform request path.

`status`, `permissions`, command preflight, and `batch` share one permission probe per process. `permissions --request` is the only path that intentionally asks the platform to prompt again.

### version
```bash
agent-desktop version
agent-desktop version --json
```
Returns version string. Use `--json` for `{ "version": "0.1.3", "platform": "macos", "arch": "aarch64" }`.

## Skills (bundled docs)

Skill markdown ships compiled into the binary. Use these to load up-to-date guidance without hitting the network.

### skills (or `skills list`)
```bash
agent-desktop skills
```
Lists every bundled skill with aliases, summaries, and reference filenames.

### skills get
```bash
agent-desktop skills get desktop                  # Primary guide (this skill's main file)
agent-desktop skills get desktop --full           # Main + every reference inlined with `--- references/<file> ---` separators
agent-desktop skills get desktop workflows        # Single reference; bare stem or `references/workflows.md` both work
agent-desktop skills get ffi                      # Specialized: embedding via the C ABI
```

| Arg / Flag | Description |
|------------|-------------|
| `<name>` | Skill name or alias. `desktop` ↔ `agent-desktop`, `ffi` ↔ `agent-desktop-ffi`. |
| `<reference>` (positional) | Reference filename (stem or full `references/<file>.md`). Omit for the main guide. |
| `--full` | Inline every reference after the main file. Ignored when a specific reference is requested. |

JSON envelope contains the markdown under `data.content`. Pipe to `jq -r .data.content` (or extract with `python3 -c`) to print just the markdown.

### skills path
```bash
agent-desktop skills path
```
Reports `{ "location": "embedded", ... }` — skills are baked into this binary via `include_str!`. To extract a copy on disk, redirect `skills get <name>` output into a file.
