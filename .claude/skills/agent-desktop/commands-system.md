# System Commands

App lifecycle, window management, clipboard, wait, and system health commands.

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
Quits an application gracefully. Use `--force` to kill the process.

### list-apps
```bash
agent-desktop list-apps
```
Lists all running GUI applications. Returns array of `{ name, pid, bundle_id }`.

## Window Management

### list-windows
```bash
agent-desktop list-windows
agent-desktop list-windows --app "Finder"
```
Lists all visible windows, optionally filtered by app. Returns array of `{ id, title, app_name, pid, bounds }`.

### focus-window
```bash
agent-desktop focus-window --app "Finder"
agent-desktop focus-window --title "Documents"
agent-desktop focus-window --window-id "w-4521"
```
Brings a window to the front. At least one identifier required.

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
agent-desktop wait --element @e5 --timeout 5000 --app "App"
```
Blocks until the element ref appears in the accessibility tree. Useful after triggering UI changes.

### wait (window)
```bash
agent-desktop wait --window "Save As" --timeout 10000
```
Blocks until a window with the given title appears.

### wait (text)
```bash
agent-desktop wait --text "Loading complete" --app "Safari" --timeout 5000
```
Blocks until the specified text appears anywhere in the app's accessibility tree.

### wait (menu)
```bash
agent-desktop wait --menu --app "Finder" --timeout 3000
```
Blocks until a context menu is detected as open.

### wait (menu-closed)
```bash
agent-desktop wait --menu-closed --app "Finder" --timeout 3000
```
Blocks until the context menu is dismissed.

| Flag | Default | Description |
|------|---------|-------------|
| (positional) | | Milliseconds to pause |
| `--element` | | Ref to wait for |
| `--window` | | Window title to wait for |
| `--text` | | Text to wait for |
| `--menu` | false | Wait for context menu to open |
| `--menu-closed` | false | Wait for context menu to close |
| `--timeout` | 30000 | Timeout in ms (for element/window/text/menu waits) |
| `--app` | | Scope the wait to a specific application |

## Batch

### batch
```bash
agent-desktop batch '[{"command":"click","args":{"ref_id":"@e1"}},{"command":"wait","args":{"ms":500}},{"command":"click","args":{"ref_id":"@e2"}}]'
agent-desktop batch '[...]' --stop-on-error
```
Execute multiple commands in sequence from a JSON array. Each entry has `command` (string) and `args` (object).

| Flag | Default | Description |
|------|---------|-------------|
| `--stop-on-error` | false | Halt on first failed command |

**Batch format:**
```json
[
  { "command": "click", "args": { "ref_id": "@e1" } },
  { "command": "wait", "args": { "ms": 500 } },
  { "command": "type", "args": { "ref_id": "@e2", "text": "hello" } }
]
```

## System Health

### status
```bash
agent-desktop status
```
Returns adapter health, platform info, and permission state.

### permissions
```bash
agent-desktop permissions
agent-desktop permissions --request
```
Checks accessibility permission status. Use `--request` to trigger the macOS system dialog.

### version
```bash
agent-desktop version
agent-desktop version --json
```
Returns version string. Use `--json` for `{ "version": "0.1.3", "platform": "macos", "arch": "aarch64" }`.
