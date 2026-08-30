# System Commands

App lifecycle, window management, notifications, clipboard, wait, and system health commands.

## App Lifecycle

### launch
```bash
agent-desktop launch "System Settings"
agent-desktop launch "com.apple.Safari" --timeout 10000
agent-desktop launch "TextEdit" --arg /tmp/notes.txt
agent-desktop launch "MyTool" --arg --flag --arg value --env KEY=VALUE --cwd /tmp
agent-desktop launch "MyTool" --no-attach
agent-desktop launch "TextEdit" --activate
agent-desktop launch "notepad.exe"                 # Windows: system-directory bare name
agent-desktop launch "C:\\Windows\\System32\\notepad.exe"
agent-desktop launch "Obsidian" --cdp
agent-desktop launch "Obsidian" --cdp 9229
```
Launches an application and returns once the process is running. On macOS the identifier is a display name or bundle ID. On Windows the identifier is an absolute path (drive + backslash, UNC, or `\\?\`) or a bare executable name that resolves only under `System32` / the Windows directory (A21-1) — display-name and AUMID launch are not on this path.

| Flag | Default | Description |
|------|---------|-------------|
| `--timeout` | 30000 | Upper bound in ms for the whole launch |
| `--arg` | | Command-line argument passed to the launched app; repeatable, order preserved. For a value that starts with `-`, use the equals form (`--arg=<value>`) — the space form swallows the next flag |
| `--env` | | `KEY=VALUE` environment variable for the launched process; repeatable |
| `--cwd` | | Working directory for the launched process (honored on Windows; rejected by macOS Launch Services) |
| `--no-attach` | false | Require a fresh launch instead of the default attach-if-running behavior |
| `--activate` | false | Bring the app forward so it presents a window, and wait for that window |
| `--cdp` | | Launch fresh with a Chrome DevTools Protocol port, verified before return; optional `[PORT]`, `0` or omitted picks a free port |

The process starting and the app presenting a window are separate outcomes, so the response reports them separately:

```json
{ "app": "TextEdit", "pid": 611, "process_instance": "macos-proc-v1:...",
  "window": { "id": "w-110407", "title": "Open", "visible": true } }
```

`window` is present when the app already has one and **omitted when it does not**. Its absence is a fact, not a failure — `launch` still returns `ok: true`.

When the launched app's bundle is built on Chromium — detected from the bundle's frameworks, not the app's name — the response also carries `renderer: "chromium"` and a `suggestion` string:

```json
{ "app": "Slack", "pid": 2201, "process_instance": "macos-proc-v1:...",
  "window": { "id": "w-8891", "title": "Slack | general", "visible": true },
  "renderer": "chromium",
  "suggestion": "Chromium app: for web-content work, run close-app and then launch --cdp, then drive the web contents with agent-browser or any CDP client. Accessibility commands still cover everything, including native menus and dialogs." }
```

`renderer` and `suggestion` are both optional and omitted on a non-Chromium app. Read `suggestion` as a hint the response carries, not an instruction the command enforces — a plain `launch` still succeeds and the accessibility path still works on a Chromium app; the field only names the faster option for the web-content case. See "Driving the web contents of a Chromium app" below for the `--cdp` flow the suggestion points at.

A launch waits only for the windows the launch itself causes. It polls until the app reports that it finished starting up, plus a short grace for the first window to reach the window server. Most apps therefore return their window in one step. An app that opens its first window only when brought forward — any document-based app — returns without one instead of waiting out `--timeout`.

A launch that finds its process gone before any window appears fails with `APP_UNRESPONSIVE` rather than reporting a windowless success.

When you need the window:

- `--activate` asks the app to present one and waits for it up to `--timeout`, because activation is what causes the window. This brings the app forward, so it is not headless. Pair it with a small `--timeout` for an app that may have no window at all.
- `wait --event window-opened` waits on your terms after you trigger the window some other way.

Windowless, menu-bar-only, and background apps simply report no `window`; use `list-apps` to observe those processes and read their `presentation`. `--no-attach` rejects an already-running app with `ACTION_FAILED` and starts a fresh instance.

On Windows, attach matches by image name across the process snapshot, so multiple running instances are `AMBIGUOUS_TARGET` (dogfood J2 against multi-instance `explorer.exe`). A launcher process whose visible window belongs to a child pid reports no `window` for the pid it launched (A21-1); `list-windows` finds the child's window.

### Driving the web contents of a Chromium app
```bash
agent-desktop launch "Obsidian" --cdp
agent-desktop launch "Obsidian" --cdp 9229
```
Use `--cdp` on Electron and other Chromium-based apps — Slack, VS Code, Discord, Obsidian, Notion, and similar — whose web contents are dense or slow to walk through the accessibility tree. It launches the app fresh with `--remote-debugging-port=<port>` and `--remote-debugging-address=127.0.0.1` (loopback pinned), then polls `http://127.0.0.1:<port>/json/version` until the endpoint answers with a parseable `webSocketDebuggerUrl`, before the command returns. Pass a port number for an explicit choice; omit it or pass `0` to let the OS pick a free one.

The port exists only for a fresh process, so `--cdp` requires a fresh launch. `launch` never quits a running app for you — a silent quit loses the user's state — so an already-running target returns `ACTION_FAILED` with `details.kind: "cdp_requires_fresh_launch"` instead. Run `close-app` first, confirm the process exited, then launch again with `--cdp`.

`--cdp` owns the remote-debugging switches. A user `--arg` naming `--remote-debugging-port`, `--remote-debugging-pipe`, `--remote-debugging-address`, or `--remote-allow-origins` is rejected before launch — see `cdp_switch_conflict` below.

On success, the response adds a `cdp` object and a `suggestion` string naming the next step. "Verified" means the endpoint answered `/json/version` with a parseable `webSocketDebuggerUrl`, so `cdp.websocket_url` is always present on success:

```json
{ "app": "Obsidian", "pid": 4821, "cdp": {
  "port": 9229,
  "http_endpoint": "http://127.0.0.1:9229",
  "websocket_url": "ws://127.0.0.1:9229/devtools/browser/<id>",
  "product": "Chrome/142.0.7444.265"
},
  "suggestion": "Next: run `agent-browser connect <port>` (preferred; `agent-browser skills get electron` has the guide) or connect any CDP client such as Playwright or Puppeteer. If neither is available, ask the user to install agent-browser or continue with accessibility commands. Do not hand-roll raw CDP or call app-internal APIs — that path is unverified and app-specific. Native menus, dialogs, windows, and screenshots stay with agent-desktop." }
```

`suggestion` is informational, the same way `data.cdp` itself is — read it, do not treat it as a command the process enforces.

`--cdp` itself is platform-neutral: the remote-debugging switches are passed to the process the same way on macOS and Windows, and the endpoint is verified by the same loopback probe. What differs is `renderer`. macOS detects a Chromium app from its bundle and reports `"renderer": "chromium"`, which is also what makes the *unprompted* nudge fire — launch a Chromium app **without** `--cdp` there and the response suggests closing and relaunching with it. Windows does not detect the renderer yet, so on Windows `renderer` is absent and that nudge never appears. Absent means undetected, never "not Chromium" — do not read a missing `renderer` on Windows as evidence the app is native. `--cdp` still works on a Windows Chromium app; you just have to know to ask for it.

The probe that verifies the endpoint has a reserved time budget so a slow launch cannot consume it: `reserve = min(5s, one quarter of the remaining launch budget)`, carved out of `--timeout` before the probe starts.

Errors:

| Code | `details.kind` | Meaning | Recovery |
|------|-----------------|---------|----------|
| `ACTION_FAILED` | `cdp_requires_fresh_launch` | The app was already running | `close-app`, confirm it exited, then `launch --cdp` again |
| `INVALID_ARGS` | `cdp_port_in_use` | The explicit port you named is already bound | Name a different port, or omit the number and let agent-desktop pick a free one |
| `INVALID_ARGS` | `cdp_switch_conflict` | `--arg` also carried `--remote-debugging-port`, `--remote-debugging-pipe`, `--remote-debugging-address`, or `--remote-allow-origins` | Drop that `--arg`; `--cdp` owns the remote-debugging switches |
| `ACTION_FAILED` | `cdp_endpoint_unavailable` | No DevTools endpoint answered on the port before the deadline — a non-Chromium app, one that strips debugging switches from its main process, or one still starting up | `details` carries `pid`, `port`, `elapsed_ms`, `probe_budget_ms`, `process_instance`, and `responder_without_devtools_body: true` when something answered over HTTP without a DevTools body. The app is left running; fall back to the accessibility path |

Security: `--remote-debugging-address=127.0.0.1` pins the endpoint to loopback and `--cdp` rejects `--arg` values that would widen it, but while the port is open, any local process running as your user can still reach it and gain full control of the app's web contents — that boundary belongs to the OS, not to agent-desktop. Request `--cdp` only for the step that needs it; `close-app` ends the exposure along with the app itself.

**Handoff:** once `data.cdp` is present, drive the app's web contents with a CDP client — agent-desktop never talks to that port itself. Any framework that speaks CDP can connect: `agent-browser` is preferred (it has the ref-based agent workflow and a bundled `electron` skill), but Playwright, Puppeteer, `chrome-remote-interface`, and other CDP clients work too. Check for `agent-browser` first (`command -v agent-browser`):

- If it is installed, connect with `agent-browser connect <port>`, then use its normal snapshot/click/type workflow. It ships an `electron` skill: `agent-browser skills get electron`.
- If it is not installed but another CDP client is available, connect with that instead.
- If neither is available, ask the user to run `npm install -g agent-browser`, or keep using agent-desktop's accessibility commands — those always work, on this app or any other.

agent-desktop never invokes `agent-browser` itself; the calling agent does. Even with CDP connected, these stay on the accessibility path, because CDP cannot reach them:

- The native menu bar (`snapshot --surface menubar`)
- File dialogs and sheets
- Window management (`list-windows`, `focus-window`, `resize-window`, and related commands)
- Notifications
- Screenshots
- Any app you did not launch yourself — CDP cannot attach to an already-running process, so the accessibility path is the only attach story there

### close-app
```bash
agent-desktop close-app "TextEdit"
agent-desktop close-app "TextEdit" --force
agent-desktop close-app "notepad.exe" --force   # Windows
```
Requests an application quit and returns success only after the process is observed gone. Session-critical processes are refused with `INVALID_ARGS` + `not_delivered` before any native close (`close_app.rs`; dogfood J2 for Windows `explorer.exe`) — not `PERM_DENIED`.

- Graceful: posts a platform quit request (`WM_CLOSE` to every top-level window of the pid on Windows; Apple Events / SIGTERM path on macOS) and reports `{ "app", "method": "graceful", "requested": true, "closed": true }` only after verified exit. If a save dialog appears, `snapshot` it and click the choice.
- `--force`: `{ "app", "method": "force", "requested": true, "closed": true }` after verified termination (`TerminateProcess` on Windows; SIGTERM then SIGKILL on macOS).

On Windows, a steady-state windowless process is `APP_NOT_FOUND` via `list-apps` (window-owning inventory only), unlike macOS, which can close windowless apps.

### list-apps
```bash
agent-desktop list-apps
agent-desktop list-apps --app "Text"
```
Lists running GUI applications, optionally filtered by a case-insensitive name substring. Returns array of `{ name, pid, bundle_id, presentation }`.

`presentation` tells a foreground app from one that only appears on a hotkey or lives in the menu bar:

| Value | Meaning |
|-------|---------|
| `foreground` | Owns ordinary windows and appears in the Dock |
| `background` | No Dock entry — menu-bar and tray items, and overlays summoned by a hotkey. Their windows may exist only while shown |
| omitted | Not registered as an application (helper processes and daemons found through the process table) |

Applications with no user interface at all are excluded.

## Window Management

### list-windows
```bash
agent-desktop list-windows
agent-desktop list-windows --app "Finder"
```
Lists all visible windows, optionally filtered by app. Returns array of `{ id, title, app_name, pid, bounds, is_focused, accessible }`. Focus is detected through the platform's frontmost/focused-window APIs, not window stacking order. `accessible` is false only when the platform confirmed that semantic accessibility commands cannot reach the window; transient probe failures and omitted legacy values preserve the default true value.

The inventory comes from the window server, which knows more windows than the
accessibility layer exposes. Targeting one an application never published
returns `ACTION_NOT_SUPPORTED` with `kind: "window_without_accessibility_element"`
— the window exists and still accepts screenshots and coordinate input, but no
semantic command can reach it. Choose another window from this list.

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
agent-desktop resize-window --window-id w-4521 --width 800 --height 600
```

### move-window
```bash
agent-desktop move-window --app "TextEdit" --x 0 --y 0
agent-desktop move-window --window-id w-4521 --x 0 --y 0
```

### minimize
```bash
agent-desktop minimize --app "TextEdit"
agent-desktop minimize --window-id w-4521
```

### maximize
```bash
agent-desktop maximize --app "TextEdit"
agent-desktop maximize --window-id w-4521
```
Zooms the window to fill the screen.

### restore
```bash
agent-desktop restore --app "TextEdit"
agent-desktop restore --window-id w-4521
```
Undoes a minimize: the window returns to whatever placement it held before,
which for a window that was maximized when it was minimized is maximized
again. Restore does not promise to un-maximize.

On Windows, `minimize` / `maximize` / `restore` / `resize-window` /
`move-window` and any headed command that must focus a window first refuse a
target whose thread has stopped processing messages, reporting
`APP_UNRESPONSIVE` with `not_delivered` rather than blocking: those operations
are delivered to the target's message queue, so a hung application would
otherwise hang the command with no timeout able to interrupt it.

## Notifications

macOS drives Notification Center; Windows drives the Action Center over UI
Automation. The command shapes and JSON fields are identical; the platform
differences are noted inline.

Output is not redacted at the command layer: notification titles and bodies
are returned verbatim (and the notification-area surface publishes the
shell's names of installed background agents). Treat output you route onward
as sensitive.

If Notification Center fails to close after a successful list or dismiss operation, the command still returns its completed result; the close failure is logged internally and is never surfaced as an error, so a cleanup hiccup never discards a completed action.

### list-notifications
```bash
agent-desktop --headed list-notifications
agent-desktop --headed list-notifications --app "Slack"
agent-desktop --headed list-notifications --text "deploy" --limit 5
```
Lists notifications in Notification Center (macOS) or the Action Center
(Windows). Headless mode can observe it only when it is already open — an
open surface is adopted and left as found; when it is closed, a
strict-headless call is refused with `POLICY_DENIED` before the surface is
raised, and `--headed` may open it and restore the prior frontmost app
afterward. Returns array of `{ index, app_name, title, body, actions }`.

| Flag | Default | Description |
|------|---------|-------------|
| `--app` | | Filter by source app name |
| `--text` | | Filter by text content (matches title and body) |
| `--limit` | | Max number of notifications to return |

### dismiss-notification
```bash
agent-desktop --headed dismiss-notification 1 --expected-app "Slack" --expected-title "Deploy complete"
agent-desktop --headed dismiss-notification 3 --app "Slack" --expected-app "Slack"
```
Dismisses a single notification by its 1-based index. Requires `--headed` and at least one fingerprint from the listing (`--expected-app` or `--expected-title`). Returns the dismissed notification info.

| Flag | Default | Description |
|------|---------|-------------|
| (positional) | | 1-based notification index (required) |
| `--app` | | Filter by app before indexing |
| `--expected-app` | | Fingerprint app name (at least one fingerprint required) |
| `--expected-title` | | Fingerprint title (at least one fingerprint required) |

### dismiss-all-notifications
```bash
agent-desktop --headed dismiss-all-notifications
agent-desktop --headed dismiss-all-notifications --app "Slack"
```
Dismisses all notifications, optionally filtered by app. Requires `--headed` because it mutates the focused system notification surface. Reports per-notification failures.

Returns `{ "dismissed_count": N, "failures": [...], "failed_count": N }`.

On Windows the clear is judged against the identity set captured before it:
only captured entries still present afterwards are failures, so entries
arriving while the clear runs are new arrivals, not failures.

### notification-action
```bash
agent-desktop --headed notification-action 1 "Reply" --expected-app Slack
agent-desktop --headed notification-action 2 "Mark as Read" --expected-app Slack --expected-title "#general"
```
Clicks a named action button on a notification by its 1-based index. Requires `--headed` and at least one listing fingerprint. An action name the entry does not offer fails with `ACTION_NOT_SUPPORTED` and leaves the notification unchanged.

`--expected-app` and `--expected-title` pin the call to the notification
you observed in `list-notifications`. Notification Center reorders
entries between listings, so an arriving or dismissed notification can shift
the target at `INDEX`. When the row at
`INDEX` no longer matches, the call fails with `NOTIFICATION_NOT_FOUND`
instead of pressing. Omitting both fingerprints is rejected with `INVALID_ARGS`.

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

Like listing, a headless wait can observe only an already-open Notification Center; use global `--headed` when the command may open and later restore it.

On Windows the wait holds no long-lived session: each poll opens and closes
the Action Center through its own one-call session, adopting a center that
is already present and restoring the entry state afterwards. A toast joins
the center only while it is open, so toasts posted while the center sits
closed between polls never land — hold the center open yourself if you are
staging arrivals.

## Clipboard

### clipboard-get
```bash
agent-desktop clipboard-get
agent-desktop clipboard-get --format auto
agent-desktop clipboard-get --format image --out /tmp/clip.png
agent-desktop clipboard-get --format file-urls
```
Reads a typed clipboard representation.

| Flag | Default | Description |
|------|---------|-------------|
| `--format` | text | Representation to read: `text`, `auto` (richest available: file references, then image, then text), `image`, `file-urls` |
| `--out` | private temp file | Where to write image bytes when `--format image`/`auto` resolves to an image; defaults to a private file under the active session's directory, or `~/.agent-desktop/tmp` with no active session. A user-named `--out` path bypasses the private-file seam via `write_user_atomic` so network shares and foreign-owned directories remain writable |

**Output by format:**
```json
{ "data": { "type": "text", "text": "clipboard contents" } }
{ "data": { "type": "file_urls", "file_urls": ["/Users/me/Documents/report.pdf"] } }
{ "data": { "type": "image", "path": "/Users/me/.agent-desktop/sessions/<id>/clipboard/clipboard-...png", "width": 800, "height": 600 } }
```
When the pasteboard has nothing in the requested representation, the response is `{ "data": { "type": "<requested format>", "found": false } }` with no other payload fields.

**Windows:** formats map through Win32 — `CF_UNICODETEXT` (text), `CF_DIB`/`CF_DIBV5`/registered PNG (image), `CF_HDROP` (file lists). `auto` resolves **FileUrls → Image → Text**, matching macOS. There is one clipboard per window station; hermetic tests use save/restore plus a serialization lock because creating a private window station failed without privilege 1314 (A22-5). Delay-rendered `GetClipboardData` can hang against a non-pumping owner, so the read path abandons a worker rather than blocking past the deadline (A22-3).

### clipboard-set
```bash
agent-desktop clipboard-set "Hello, world!"
agent-desktop clipboard-set --image /tmp/screenshot.png
agent-desktop clipboard-set --file-url /Users/me/Documents/report.pdf
agent-desktop clipboard-set --file-url /tmp/a.txt --file-url /tmp/b.txt
```
Writes typed content to the clipboard. `--file-url` (repeatable) and `--image` each take priority over the positional text argument when present; only one representation is written per call.

| Flag | Description |
|------|-------------|
| (positional) | Text to write (ignored if `--image` or `--file-url` is given) |
| `--image` | Path to a PNG file to write to the clipboard |
| `--file-url` | File path to write as a file reference; repeatable. Every path must exist on disk or the command returns `INVALID_ARGS` |

**Windows:** publishes the same typed representations consumers actually read (`CF_UNICODETEXT`, DIB/PNG image formats, `CF_HDROP`). A write that loses clipboard ownership mid-transaction reports `delivered_unverified` rather than a false success.

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
agent-desktop wait --element @s8f3k2p9:e5 --predicate actionable --timeout 5000
agent-desktop wait --element @s8f3k2p9:e5 --predicate actionable --action type --timeout 5000
agent-desktop wait --element @s8f3k2p9:e5 --predicate value --value "Done" --timeout 5000
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

### wait (event)
```bash
agent-desktop wait --event window-opened --app "Finder" --timeout 10000
agent-desktop wait --event window-closed --window-id "w-1234" --timeout 10000
agent-desktop wait --event app-launched --app "Safari" --timeout 15000
agent-desktop wait --event app-terminated --app "Safari" --timeout 15000
agent-desktop wait --event focus-changed --timeout 10000
agent-desktop wait --event surface-appeared --app "Finder" --timeout 5000
agent-desktop wait --event window-opened --window "Untitled" --timeout 10000
```
Blocks until a desktop lifecycle signal is observed, detected by diffing a baseline captured at wait start against fresh reads — no need to know a new window's id or title up front. `--window-id`/`--window` are optional narrowing filters on top of `--event`, never a requirement by themselves (bare `--window` without `--event` instead selects the `wait (window)` mode above).

**`--app` resolves once, at wait start, and that has three consequences worth
knowing before you rely on it.** The application must already be running:
every event except `app-launched` resolves the target before the first poll,
so scoping to a process that does not exist yet returns `APP_NOT_FOUND`
immediately rather than waiting out the timeout. Use `app-launched` — or an
unscoped wait — when you are racing a launch. The wait then pins to the one
process instance it resolved, so a *second* process of the same name starting
later is invisible to it; if you need any instance of a name, run the wait
unscoped and filter the event yourself. And a target that dies before that
resolution completes also reports `APP_NOT_FOUND`, which reads like a bad
`--app` value but means the opposite: the application existed and its
disappearance is what broke the lookup. For a disappearance you expect,
prefer an unscoped wait.

| Token | Fires when |
|-------|------------|
| `window-opened` | A window not present in the baseline appears |
| `window-closed` | A baseline window disappears |
| `app-launched` | A process not present in the baseline starts |
| `app-terminated` | A baseline process exits |
| `focus-changed` | The OS-focused window differs from the baseline's |
| `surface-appeared` | A menu/sheet/popover/alert surface count increases |
| `surface-dismissed` | A menu/sheet/popover/alert surface count decreases |

Transient errors (timeouts, element-not-found) are retried within the `--timeout` budget for both the baseline capture and polling; other errors fail immediately. Timeout errors include `baseline_counts` and, when a poll errored, `last_error`.

| Flag | Default | Description |
|------|---------|-------------|
| (positional) | | Milliseconds to pause |
| `--element` | | Ref to wait for |
| `--snapshot` | latest | Snapshot ID for `--element` waits |
| `--predicate` | exists | Element predicate: `exists`, `enabled`, `visible`, `actionable`, `value` |
| `--value` | | Expected text for `--predicate value` |
| `--action` | click | Action checked by `--predicate actionable`: `click`, `type`, `set-value`, `clear` |
| `--count` | | Expected match count for `--text` waits |
| `--window` | | Window title to wait for; with `--event`, narrows the event to that window's title instead of selecting a mode |
| `--text` | | Text to wait for; with `--notification`, filters notification title/body |
| `--menu` | false | Wait for menu surface to open |
| `--menu-closed` | false | Wait for menu surface to close |
| `--notification` | false | Wait for a new notification |
| `--event` | | Desktop lifecycle signal to wait for: `window-opened`, `window-closed`, `app-launched`, `app-terminated`, `focus-changed`, `surface-appeared`, `surface-dismissed` |
| `--window-id` | | Narrows `--event` to one window ID (window/focus events only) |
| `--timeout` | 30000 | Timeout in ms (for element/window/text/menu/event waits) |
| `--app` | | Scope the wait to **one already-running process instance**, resolved once at wait start — not to every process of that name for the wait's duration. See the note below |

## Batch

### batch
```bash
agent-desktop batch '[{"command":"click","args":{"ref_id":"@e1","snapshot":"<snapshot_id>"}},{"command":"wait","args":{"ms":500}},{"command":"click","args":{"ref_id":"@e2","snapshot":"<snapshot_id>"}}]'
agent-desktop batch '[...]' --stop-on-error
agent-desktop --session run-a batch '[{"command":"status","session":"run-b","args":{}}]'
agent-desktop batch '[{"command":"launch","args":{"app":"Obsidian","cdp":0}}]'
```
Execute multiple commands in sequence from a JSON array. Each entry has `command` (string) and `args` (object). Use `args`, not `params`. For ref-consuming commands, pass the output `snapshot_id` as the `snapshot` field.

Batch uses the same typed `Commands` enum, command policy preflight, permission report, and dispatch path as the CLI. Unknown fields are rejected instead of being silently ignored. Nested `batch` is rejected.

Each entry may include `"session": "id"` beside `command` and `args`. If omitted, the entry inherits the top-level resolved session. Use per-entry sessions only when intentionally inspecting or coordinating separate agent runs.

**Trace in batch:** when the top-level CLI passes `--trace <path>`, every entry writes to that single file (override). Without `--trace`, entries inherit the resolved session's manifest-gated segment sink; a per-entry `"session"` override re-derives the sink for that session (events never land in the parent session's segment). Session subcommands (`session start`, etc.) are also available in batch JSON via `"action": "start"|"end"|"list"|"gc"`.

| Flag | Default | Description |
|------|---------|-------------|
| `--stop-on-error` | false | Halt on first failed command |

**Batch format:**
```json
[
  { "command": "click", "args": { "ref_id": "@e1", "snapshot": "<snapshot_id>" } },
  { "command": "wait", "args": { "ms": 500 } },
  { "command": "type", "args": { "ref_id": "@e2", "snapshot": "<snapshot_id>", "text": "hello" } },
  { "command": "status", "session": "other-agent", "args": {} },
  { "command": "session", "args": { "action": "start", "name": "batch-run" } }
]
```

**Per-entry failure shape:**
```json
{
  "version": "2.3",
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

## Session lifecycle

Sessions are on-disk containers under `<state root>/sessions/<id>/` with a `session.json` manifest, snapshot refmaps, and (when tracing is on) a `trace/` directory. The state root defaults to `~/.agent-desktop`; setting `AGENT_DESKTOP_HOME` relocates it — the env value is the root itself, applied to every subcommand. A relative or empty value fails with `INVALID_ARGS` before dispatch, and `status` reports the resolved root as `state_root`. Session selection is explicit; `session start` returns an ID but does not activate it for later processes.

### cursor-overlay enable / disable

```bash
agent-desktop session start --cursor                 # session + default cursor in one command
agent-desktop --session <id> cursor-overlay enable --label "Opening menu" --accent "#FF3B7B"
export AGENT_DESKTOP_SESSION=<id>
agent-desktop cursor-overlay disable
```

No flags gives the default look: white body, near-black rim, blue ripple, blue element outline.

Style is stored in the session manifest and inherited by every eligible headless command, batch entries included. Action and batch-entry schemas take no cursor flags. Run `enable` again to restyle; it applies at once.

| Flag | Meaning | Default |
|---|---|---|
| `--label TEXT` | Intent text beside the cursor | none |
| `--max-words N` | Label word limit, 1 to 12 | 6 |
| `--fill HEX` | Cursor body colour | `#FFFFFF` |
| `--rim HEX` | Cursor outline colour | `#111318` |
| `--accent HEX` | Ripple and element outline colour | `#4299FF` |
| `--size N` | Cursor size multiplier, 0.5 to 4.0 | 1.0 |
| `--no-ripple` | No ripple on click | ripple on |
| `--no-highlight` | No element outline on click | outline on |

Behaviour:

- Travel is a human path, 90 to 320 ms. The cursor never rotates or resizes.
- The action waits for the cursor to land, capped at 900 ms. A slow renderer never blocks it.
- A click plays a ripple, then flashes an accent outline around the element for 0.9 s. Both draw below the cursor.
- The card shows the label. With no label there is no card.
- Idle for 6 s it fades out; the next command restores it.
- `disable` removes it and stops the renderer. Ending the session is not needed.
- Headed actions hide it while the real pointer is in use.
- macOS renders it natively; other platforms use the adapter's presentation no-op.

### session start
```bash
agent-desktop session start
agent-desktop session start --name "nightly-run"
agent-desktop session start --no-trace          # Namespace only — no automatic JSONL
agent-desktop session start --cursor            # Also show the default cursor overlay
```
Creates the session directory, pre-creates `trace/` (when tracing is on), writes `session.json` (`trace: on` unless `--no-trace`), and prints `{ "session_id", "name", "trace", "created_at" }`. `--cursor` adds `cursor_overlay` to that response and shows the overlay. Pass that ID through global `--session` or `AGENT_DESKTOP_SESSION` on later commands.

### session end
```bash
agent-desktop session end run-1719763200123-0
agent-desktop --session run-1719763200123-0 session end
```
Seals the manifest with `ended_at`. The ID is required either as the positional argument, global `--session`, or `AGENT_DESKTOP_SESSION`.

### session list
```bash
agent-desktop session list
```
Returns manifest fields only (`session_id`, `name`, `created_at`, `ended_at`, `trace`) — no subtree walk.

### session gc
```bash
agent-desktop session gc
agent-desktop session gc --ended
agent-desktop session gc --older-than 3600
```
Removes ended sessions that are not live. Never reaps a session with a live lock holder or recent `trace/` activity. Refuses symlinked session directories.

### Activation (all commands)

| Source | Precedence |
|--------|------------|
| `--session <id>` | Highest |
| `AGENT_DESKTOP_SESSION` env var | Fallback |

With neither source, commands use the global, non-session namespace. There is no current-session pointer fallback.

Trace-on requires a manifest with `trace: on` from `session start`. Bare `--session` or FFI `ad_adapter_create_with_session` without that manifest selects the snapshot namespace only.

## Trace read and export

Both commands require an active trace-enabled session (`session start` or `--session <id>` with a manifest). They are permissionless — no accessibility or screen-recording grant is needed to read or export traces from disk.

### trace show
```bash
agent-desktop trace show [--limit N] [--event PREFIX]
```
Merges every segment under `<session>/trace/` into one deterministic timeline. Default `--limit 500` returns the **tail**; `--limit 0` returns all events. `--event action.` filters by event-name prefix before the tail slice.

Response `data` includes `session_id`, per-segment stats (`segments[]` with `segment`, `pid`, `schema`, `event_count`, `skipped_lines`), `total_events`, `returned_events`, `truncated`, optional `warnings[]` (`kind`, `message`), and the merged `events[]` (each annotated with `writer_pid` and `segment`).

Reader tolerance: truncated final lines, corrupt JSON, foreign files, symlinked segments, and unpaired `command.start`/`command.end` pairs degrade to counted warnings — never hard errors.

`warnings[].kind` is one of:

| `kind` | Meaning |
|--------|---------|
| `foreign_file` | A file under `trace/` doesn't match the `<pid>-<procTs>.jsonl` segment name pattern (and isn't dotfile-hidden); ignored entirely |
| `unreadable_segment` | The segment file could not be opened or read; the whole segment is skipped |
| `symlinked_segment` | The segment path is a symlink; skipped before any read is attempted |
| `schema_unknown` | The segment's `trace.meta` declares a schema newer than this reader supports; still read best-effort |
| `unpaired_command` | A `command.start` has no matching `command.end` (or vice versa) within the returned event window |

### trace export
```bash
agent-desktop trace export [--out path.html] [--limit N]
```
Builds one self-contained HTML file with embedded JSON and base64 PNG screenshots. Default `--limit 5000` (ten times `trace show`'s default). Works from `file://` with no network fetches.

Without `--out`, the file is written into the **session directory** as `trace-<session_id>.html` (`~/.agent-desktop/sessions/<id>/trace-<id>.html`) — not the current working directory. `--out` overrides the path, including writing outside the session directory.

Response `data` reports `path`, `event_count`, `screenshots_embedded`, `screenshots_skipped`, and `bytes`. Export refuses symlinked `--out` paths and returns `INVALID_ARGS` when the embedded JSON exceeds 200MiB (use a smaller `--limit`).

### Replay artifacts (`--screenshots`)
```bash
agent-desktop session start --screenshots   # manifest artifacts: full
```
Requires tracing (`trace: on`; `--no-trace --screenshots` is rejected). Ref actions capture pre/post PNGs under `trace/screens/`; snapshot saves copy refmaps to `trace/refmaps/`. Skips are recorded in `action.artifacts` events with machine-readable reasons. Artifacts are **unredacted** and may appear in exported HTML — opt in only when that sensitivity is acceptable.

A skip reason lands in `skipped` when the pre- and post-action screenshot outcomes share one reason, otherwise it splits across `skipped_pre`/`skipped_post`. Reasons include (non-exhaustive):

| Token | Meaning |
|-------|---------|
| `no_session` | No active session could be resolved for this action |
| `count_budget` | Per-process screenshot count budget (200) exceeded |
| `budget` | Per-process screenshot byte budget (128MiB) exceeded |
| `write_failed` | Writing the PNG to disk failed |
| `dir: <error>` | Creating `trace/screens/` failed |
| `adapter: <ERROR_CODE>` | The platform screenshot call failed with the given error code |

Refmap copies under `trace/refmaps/` are best-effort — a skipped or failed copy never fails the primary command and leaves any prior copy intact.

## System Health

### status
```bash
agent-desktop status
```
Returns adapter health, platform info, permission report, latest snapshot metadata (`snapshot_id`, `ref_count`) when available, plus **`session_id`** (resolved active session, if any) and **`tracing`** (whether structured trace output is configured for this process — explicit `--trace`, or a trace-enabled session manifest).

When `session_id` resolves to a session with a readable manifest, the response also includes **`artifacts`**: `full` (`session start --screenshots` — screenshots and refmaps captured) or `events` (default — JSONL events only, no binary artifacts). Omitted when there is no active session.

### permissions
```bash
agent-desktop permissions
agent-desktop permissions --request
```
Checks the cached per-process permission report: `accessibility`, `screen_recording`, and `automation`, each as `{ "state": "granted" }`, `{ "state": "denied", "suggestion": "..." }`, `{ "state": "not_required" }`, or `{ "state": "unknown" }`. The current macOS adapter reports concrete `granted` or `denied` states for Accessibility and Screen Recording. Automation is probed against System Events without prompting; `{ "state": "unknown" }` means macOS would need to prompt or the target could not be probed. On Windows, capture has no screen-recording consent gate — `screen_recording` reports `not_required` when capture works and `unknown` only where the session cannot support it. `--request` asks for all three permissions through a bounded isolated helper so a stalled native prompt cannot strand the command process.

`status`, `permissions`, command preflight, and `batch` share one nonprompting permission probe per process. `permissions --request` is the only path that intentionally asks the platform to prompt again, and it does so in the isolated helper.

### version
```bash
agent-desktop version
```
Returns `{ "version": "0.3.1", "target": "aarch64", "os": "macos" }`. Always emitted as a JSON envelope (`ok: true`, `data: { version, target, os }`).

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
