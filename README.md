# agent-desktop

**Desktop automation for AI agents.** A fast, cross-platform Rust CLI that gives AI agents structured access to every native application on macOS, Windows, and Linux through OS accessibility trees — no screen scraping, no image recognition, no fragile selectors.

```bash
agent-desktop snapshot -i
agent-desktop click @e3
agent-desktop type @e5 "quarterly report"
agent-desktop screenshot --app "Finder" report.png
```

---

## How it works

Every desktop OS exposes a machine-readable accessibility tree — the same tree that powers screen readers. `agent-desktop` wraps that API behind a clean CLI and outputs structured JSON. AI agents call the binary, read the JSON, and act on element references (`@e1`, `@e2`, …). The observation-action loop lives in the agent, not here.

```
AI Agent
  │
  ├─ agent-desktop snapshot -i          # observe: get tree + ref IDs
  │    └─ {"tree": {...}, "ref_count": 14}
  │
  ├─ agent-desktop click @e7            # act: by ref
  │    └─ {"ok": true, "data": {"action": "click"}}
  │
  └─ agent-desktop snapshot -i          # re-observe after action
       └─ {"tree": {...}, "ref_count": 11}
```

**agent-desktop is not an AI agent.** It is the tool AI agents invoke.

---

## Installation

### Build from source

```bash
git clone https://github.com/lahfir/agent-desktop
cd agent-desktop
cargo build --release
# Binary at: ./target/release/agent-desktop
```

Move to your PATH:

```bash
mv target/release/agent-desktop /usr/local/bin/
```

### Requirements

| Platform | Minimum Version | Accessibility API |
|----------|----------------|-------------------|
| macOS    | 13.0+          | AXUIElement (AXAPI) |
| Windows  | 10+            | UIAutomation *(Phase 2)* |
| Linux    | Any (X11/Wayland) | AT-SPI *(Phase 2)* |

### macOS permissions

The first time you run any command, macOS will prompt for Accessibility permission. You can also trigger it explicitly:

```bash
agent-desktop permissions --request
```

Or grant it manually: **System Settings → Privacy & Security → Accessibility → add your terminal**.

---

## Quick start

```bash
# 1. Get the focused app's interactive elements with ref IDs
agent-desktop snapshot -i

# 2. Read what's on screen
agent-desktop find --role button --name "Open"

# 3. Click a button by ref
agent-desktop click @e4

# 4. Type into a text field
agent-desktop type @e7 "Hello, world"

# 5. Submit with keyboard
agent-desktop press "cmd+return"
```

---

## The ref system

`snapshot` assigns stable identifiers to every interactive element in depth-first order: `@e1`, `@e2`, `@e3`, etc. These refs are valid for subsequent action commands **until the next snapshot replaces them**.

**Only interactive roles receive refs:**

| Role | Role | Role |
|------|------|------|
| `button` | `textfield` | `checkbox` |
| `link` | `menuitem` | `tab` |
| `slider` | `combobox` | `treeitem` |
| `cell` | `radiobutton` | `incrementor` |

Static elements (labels, groups, containers) appear in the tree for context but have no `ref` and cannot be acted upon.

The refmap is persisted at `~/.agent-desktop/last_refmap.json` (permissions: `0600`) and fully replaced on every snapshot. Action commands perform optimistic re-identification — if the element at a ref has changed since the snapshot, they return `STALE_REF`.

---

## JSON output

Every command produces the same envelope:

```json
{
  "version": "1.0",
  "ok": true,
  "command": "click",
  "data": { ... }
}
```

Errors follow the same envelope with a structured error object:

```json
{
  "version": "1.0",
  "ok": false,
  "command": "click",
  "error": {
    "code": "STALE_REF",
    "message": "Element at @e7 no longer matches the last snapshot",
    "suggestion": "Run 'snapshot' to refresh refs, then retry"
  }
}
```

### Error codes

| Code | Meaning |
|------|---------|
| `PERM_DENIED` | Accessibility permission not granted |
| `ELEMENT_NOT_FOUND` | No element matched the given ref or query |
| `APP_NOT_FOUND` | Application is not running or has no open windows |
| `ACTION_FAILED` | The OS rejected the action |
| `ACTION_NOT_SUPPORTED` | Element does not support the requested action |
| `STALE_REF` | Ref is from a previous snapshot |
| `WINDOW_NOT_FOUND` | No window matched the given ID or query |
| `PLATFORM_NOT_SUPPORTED` | Command not implemented on this OS |
| `TIMEOUT` | Wait condition expired |
| `INVALID_ARGS` | Bad argument values |
| `INTERNAL` | Unexpected internal error |

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Structured error (JSON on stdout) |
| `2` | Argument parse error |

---

## Commands

### Observation

#### `snapshot`

Capture the accessibility tree of a window and allocate ref IDs.

```bash
agent-desktop snapshot [OPTIONS]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--app <NAME>` | focused app | Filter to a specific application |
| `--window-id <ID>` | — | Filter to a specific window |
| `--max-depth <N>` | `10` | Maximum tree traversal depth |
| `--include-bounds` | off | Include pixel bounds for every node |
| `--interactive-only` / `-i` | off | Omit non-interactive elements from output |
| `--compact` | off | Single-line JSON output |

```bash
# Snapshot the frontmost window
agent-desktop snapshot

# Snapshot a specific app, interactive elements only
agent-desktop snapshot --app "TextEdit" -i

# Include pixel coordinates for layout analysis
agent-desktop snapshot --include-bounds
```

<details>
<summary>Example output</summary>

```json
{
  "version": "1.0",
  "ok": true,
  "command": "snapshot",
  "data": {
    "ref_count": 8,
    "tree": {
      "role": "window",
      "name": "Untitled",
      "children": [
        {
          "role": "textfield",
          "name": "Document body",
          "ref": "@e1",
          "value": "Hello"
        },
        {
          "role": "button",
          "name": "Save",
          "ref": "@e2"
        }
      ]
    }
  }
}
```

</details>

---

#### `find`

Search the accessibility tree for elements matching a query. Returns all matches across the app.

```bash
agent-desktop find [--app <NAME>] [--role <ROLE>] [--name <TEXT>] [--value <TEXT>]
```

```bash
# Find all buttons
agent-desktop find --role button

# Find the Save button in TextEdit
agent-desktop find --app "TextEdit" --role button --name "Save"

# Find a text field containing specific text
agent-desktop find --role textfield --value "search"
```

<details>
<summary>Example output</summary>

```json
{
  "version": "1.0",
  "ok": true,
  "command": "find",
  "data": {
    "matches": [
      { "ref": "@e2", "role": "button", "name": "Save", "interactive": true },
      { "ref": null,  "role": "group",  "name": "Toolbar", "interactive": false }
    ]
  }
}
```

</details>

---

#### `screenshot`

Capture a PNG screenshot of a window or application.

```bash
agent-desktop screenshot [--app <NAME>] [--window-id <ID>] [PATH]
```

```bash
# Screenshot the frontmost window to stdout (base64 PNG)
agent-desktop screenshot

# Screenshot a specific app to a file
agent-desktop screenshot --app "Finder" ~/Desktop/finder.png
```

---

#### `get`

Read a property of a specific element by ref.

```bash
agent-desktop get <REF> [--property <PROP>]
```

| Property | Description |
|----------|-------------|
| `text` | Display text / label |
| `value` | Current value (text field content, slider position) |
| `title` | Window or element title |
| `bounds` | `{ x, y, width, height }` in screen coordinates |
| `role` | Accessibility role string |
| `states` | Array of active states |

```bash
agent-desktop get @e3 --property value
agent-desktop get @e1 --property bounds
agent-desktop get @e5 --property states
```

<details>
<summary>Example output</summary>

```json
{
  "version": "1.0",
  "ok": true,
  "command": "get",
  "data": { "property": "value", "value": "quarterly-report.pdf" }
}
```

</details>

---

#### `is`

Check a boolean property of an element.

```bash
agent-desktop is <REF> [--property <PROP>]
```

| Property | Description |
|----------|-------------|
| `visible` | Element is visible on screen |
| `enabled` | Element is interactive (not disabled) |
| `checked` | Checkbox/toggle is checked |
| `focused` | Element has keyboard focus |
| `expanded` | Disclosure or tree item is expanded |

```bash
agent-desktop is @e4 --property enabled
agent-desktop is @e6 --property checked
```

<details>
<summary>Example output</summary>

```json
{
  "version": "1.0",
  "ok": true,
  "command": "is",
  "data": { "property": "enabled", "result": true }
}
```

</details>

---

### Interaction

#### `click` / `double-click` / `right-click`

```bash
agent-desktop click <REF>
agent-desktop double-click <REF>
agent-desktop right-click <REF>
```

---

#### `type`

Type text into an element (simulates keyboard input, respects IME).

```bash
agent-desktop type <REF> <TEXT>
```

```bash
agent-desktop type @e3 "your search query"
```

---

#### `set-value`

Directly set the value of an element (faster than `type` for programmatic writes; bypasses key events).

```bash
agent-desktop set-value <REF> <VALUE>
```

```bash
agent-desktop set-value @e3 "2026-02-19"
```

---

#### `focus`

Move keyboard focus to an element.

```bash
agent-desktop focus <REF>
```

---

#### `select`

Select an option in a dropdown or combo box.

```bash
agent-desktop select <REF> <VALUE>
```

```bash
agent-desktop select @e8 "Last 30 days"
```

---

#### `toggle`

Toggle a checkbox, switch, or toggle button.

```bash
agent-desktop toggle <REF>
```

---

#### `expand` / `collapse`

Expand or collapse a disclosure triangle, tree item, or accordion.

```bash
agent-desktop expand <REF>
agent-desktop collapse <REF>
```

---

#### `scroll`

Scroll an element in a direction.

```bash
agent-desktop scroll <REF> [--direction <DIR>] [--amount <N>]
```

| Flag | Default | Options |
|------|---------|---------|
| `--direction` | `down` | `up`, `down`, `left`, `right` |
| `--amount` | `3` | Number of scroll units |

```bash
agent-desktop scroll @e2 --direction down --amount 5
```

---

### Keyboard

#### `press`

Send a keyboard shortcut or key combination.

```bash
agent-desktop press <COMBO>
```

Modifiers: `cmd`, `ctrl`, `alt`/`opt`, `shift`, `fn`. Key names are lowercase. Combine with `+`.

```bash
agent-desktop press "cmd+s"
agent-desktop press "cmd+shift+z"
agent-desktop press "escape"
agent-desktop press "return"
agent-desktop press "tab"
```

---

### App & window management

#### `launch`

Launch an application by name or bundle ID.

```bash
agent-desktop launch <APP> [--wait]
```

| Flag | Description |
|------|-------------|
| `--wait` | Block until the app's main window is visible |

```bash
agent-desktop launch "TextEdit" --wait
agent-desktop launch "com.apple.finder"
```

---

#### `close-app`

Quit an application.

```bash
agent-desktop close-app <APP> [--force]
```

```bash
agent-desktop close-app "TextEdit"
agent-desktop close-app "TextEdit" --force   # SIGKILL
```

---

#### `list-apps`

List all running applications with accessibility trees.

```bash
agent-desktop list-apps
```

<details>
<summary>Example output</summary>

```json
{
  "version": "1.0",
  "ok": true,
  "command": "list-apps",
  "data": {
    "apps": [
      { "name": "Finder", "pid": 391, "bundle_id": "com.apple.finder" },
      { "name": "TextEdit", "pid": 1204, "bundle_id": "com.apple.TextEdit" }
    ]
  }
}
```

</details>

---

#### `list-windows`

List windows for an application.

```bash
agent-desktop list-windows [--app <NAME>]
```

```bash
agent-desktop list-windows --app "Finder"
```

<details>
<summary>Example output</summary>

```json
{
  "version": "1.0",
  "ok": true,
  "command": "list-windows",
  "data": {
    "windows": [
      { "id": "w-4521", "title": "Documents", "app_name": "Finder", "pid": 391 }
    ]
  }
}
```

</details>

---

#### `focus-window`

Bring a window to the foreground.

```bash
agent-desktop focus-window [--window-id <ID>] [--app <NAME>] [--title <TEXT>]
```

```bash
agent-desktop focus-window --app "Finder" --title "Documents"
agent-desktop focus-window --window-id "w-4521"
```

---

### Clipboard

#### `clipboard-get`

Read the current clipboard contents.

```bash
agent-desktop clipboard-get
```

---

#### `clipboard-set`

Write text to the clipboard.

```bash
agent-desktop clipboard-set <TEXT>
```

```bash
agent-desktop clipboard-set "copied text"
```

---

### Wait

#### `wait`

Block for a fixed duration or until a condition is met.

```bash
agent-desktop wait [MS] [--element <REF>] [--window <TITLE>] [--timeout <MS>]
```

| Form | Description |
|------|-------------|
| `wait 2000` | Sleep for 2 seconds |
| `wait --element @e3` | Block until the element at `@e3` is visible (polls) |
| `wait --window "Save"` | Block until a window with this title appears |

```bash
# Wait for a dialog to appear
agent-desktop wait --window "Are you sure?" --timeout 10000

# Wait for a loading spinner to disappear
agent-desktop wait --element @e9 --timeout 15000

# Fixed delay
agent-desktop wait 500
```

---

### System

#### `status`

Report the runtime status of agent-desktop and its platform adapter.

```bash
agent-desktop status
```

<details>
<summary>Example output</summary>

```json
{
  "version": "1.0",
  "ok": true,
  "command": "status",
  "data": {
    "platform": "macos",
    "accessibility": "granted",
    "version": "0.1.0"
  }
}
```

</details>

---

#### `permissions`

Check or request Accessibility permissions.

```bash
agent-desktop permissions [--request]
```

```bash
# Check current permission status
agent-desktop permissions

# Trigger the system permission dialog
agent-desktop permissions --request
```

---

#### `version`

Print the binary version.

```bash
agent-desktop version [--json]
```

---

### Batch

Run multiple commands in a single invocation to reduce process-spawn overhead.

```bash
agent-desktop batch <JSON> [--stop-on-error]
```

The `JSON` argument is an array of `{ "command": "...", "args": { ... } }` objects. Results are returned in order.

```bash
agent-desktop batch '[
  {"command":"click",     "args":{"ref_id":"@e2"}},
  {"command":"type",      "args":{"ref_id":"@e5","text":"hello"}},
  {"command":"press",     "args":{"combo":"return"}}
]' --stop-on-error
```

<details>
<summary>Example output</summary>

```json
{
  "version": "1.0",
  "ok": true,
  "command": "batch",
  "data": {
    "results": [
      { "ok": true,  "command": "click", "data": { "action": "click" } },
      { "ok": true,  "command": "type",  "data": { "action": "type" } },
      { "ok": false, "command": "press", "error": "STALE_REF: ..." }
    ]
  }
}
```

</details>

---

## Common agent patterns

### Observe → identify → act

```bash
# Observe the current window
TREE=$(agent-desktop snapshot -i)

# Find the search field
SEARCH=$(agent-desktop find --role textfield --name "Search")

# Act on it
agent-desktop click @e1
agent-desktop type @e1 "quarterly report"
agent-desktop press "return"
```

### Handle stale refs

Refs become stale when the UI changes. The correct loop:

```
snapshot → act → if STALE_REF → snapshot again → retry
```

### Wait for async UI

```bash
# Trigger an action
agent-desktop click @e12

# Wait for the result dialog
agent-desktop wait --window "Export complete" --timeout 30000

# Confirm
agent-desktop click @e1   # OK button in the new snapshot
```

### Automated form fill

```bash
agent-desktop batch '[
  {"command":"focus",     "args":{"ref_id":"@e1"}},
  {"command":"set-value", "args":{"ref_id":"@e1","value":"John Doe"}},
  {"command":"set-value", "args":{"ref_id":"@e2","value":"john@example.com"}},
  {"command":"select",    "args":{"ref_id":"@e3","value":"Engineering"}},
  {"command":"click",     "args":{"ref_id":"@e10"}}
]'
```

---

## Architecture

```
agent-desktop/
├── Cargo.toml              # workspace (resolver = 2)
├── src/                    # binary crate (entry point)
│   ├── main.rs             # permission check, dispatch, JSON emit
│   ├── cli.rs              # clap derive structs (all 30 subcommands)
│   └── dispatch.rs         # match cmd → commands::execute()
└── crates/
    ├── core/               # agent-desktop-core (platform-agnostic)
    │   └── src/
    │       ├── adapter.rs      # PlatformAdapter trait
    │       ├── snapshot.rs     # SnapshotEngine, RefAllocator
    │       ├── refs.rs         # RefMap, RefEntry, @eN allocation
    │       ├── node.rs         # AccessibilityNode, WindowInfo, Rect
    │       ├── action.rs       # Action enum
    │       ├── error.rs        # AppError, AdapterError, ErrorCode
    │       ├── output.rs       # Response envelope
    │       └── commands/       # one file per CLI command
    ├── macos/              # agent-desktop-macos (Phase 1)
    │   └── src/
    │       ├── adapter.rs      # MacOSAdapter: PlatformAdapter impl
    │       ├── tree.rs         # AXUIElement tree traversal
    │       ├── actions.rs      # CGEvent keyboard/mouse/scroll
    │       ├── app_ops.rs      # launch, close, focus via AppleScript/pkill
    │       └── roles.rs        # AXRole → unified role string
    ├── windows/            # agent-desktop-windows (Phase 2)
    └── linux/              # agent-desktop-linux (Phase 2)
```

### Dependency inversion

`core` defines the `PlatformAdapter` trait. Platform crates implement it. **Core never imports platform crates.** The binary is the only wiring point:

```rust
fn build_adapter() -> impl PlatformAdapter {
    #[cfg(target_os = "macos")]
    { agent_desktop_macos::MacOSAdapter::new() }
    // ...
}
```

This constraint is enforced in CI: `cargo tree -p agent-desktop-core` must contain no platform crate names.

### PlatformAdapter trait

```rust
pub trait PlatformAdapter: Send + Sync {
    fn list_windows(&self, filter: &WindowFilter)    -> Result<Vec<WindowInfo>, AdapterError>;
    fn list_apps(&self)                              -> Result<Vec<AppInfo>, AdapterError>;
    fn get_tree(&self, win: &WindowInfo, opts: &TreeOptions) -> Result<AccessibilityNode, AdapterError>;
    fn execute_action(&self, handle: &NativeHandle, action: Action) -> Result<ActionResult, AdapterError>;
    fn resolve_element(&self, entry: &RefEntry)      -> Result<NativeHandle, AdapterError>;
    fn check_permissions(&self)                      -> PermissionStatus;
    fn focus_window(&self, win: &WindowInfo)         -> Result<(), AdapterError>;
    fn launch_app(&self, id: &str, wait: bool)       -> Result<WindowInfo, AdapterError>;
    fn close_app(&self, id: &str, force: bool)       -> Result<(), AdapterError>;
    fn screenshot(&self, target: ScreenshotTarget)   -> Result<ImageBuffer, AdapterError>;
    fn get_clipboard(&self)                          -> Result<String, AdapterError>;
    fn set_clipboard(&self, text: &str)              -> Result<(), AdapterError>;
}
```

All methods have default implementations returning `Err(AdapterError::not_supported())`, so platform stubs compile without implementing anything.

---

## Platform support

| Feature | macOS | Windows | Linux |
|---------|-------|---------|-------|
| Snapshot / tree | ✅ Phase 1 | 🔜 Phase 2 | 🔜 Phase 2 |
| Click / type / keyboard | ✅ Phase 1 | 🔜 Phase 2 | 🔜 Phase 2 |
| Screenshot | ✅ Phase 1 | 🔜 Phase 2 | 🔜 Phase 2 |
| Clipboard | ✅ Phase 1 | 🔜 Phase 2 | 🔜 Phase 2 |
| App launch / close | ✅ Phase 1 | 🔜 Phase 2 | 🔜 Phase 2 |
| MCP server mode | 🔜 Phase 3 | 🔜 Phase 3 | 🔜 Phase 3 |

### macOS implementation

- **Tree**: `AXUIElementCopyAttributeValue` with cycle detection via visited-set
- **Click/Type**: `AXUIElementPerformAction` + `CGEventCreateKeyboardEvent`
- **Scroll**: `CGEvent::new_scroll_event` (highsierra feature)
- **Screenshot**: `CGWindowListCreateImage`
- **Clipboard**: `NSPasteboard.generalPasteboard` via Cocoa FFI
- **App ops**: PID polling for launch, AppleScript for focus, `pkill -x` for close

---

## Development

### Build

```bash
cargo build                  # debug build
cargo build --release        # optimized, stripped binary (<15MB)
```

### Test

```bash
cargo test --workspace       # all unit tests
cargo clippy --all-targets -- -D warnings
```

### Add a command

Adding a command touches exactly five files:

1. **`crates/core/src/commands/{name}.rs`** — implement `execute(args, adapter)`
2. **`crates/core/src/commands/mod.rs`** — register the module
3. **`src/cli.rs`** — add a subcommand variant and arg struct
4. **`src/dispatch.rs`** — add a match arm
5. **`crates/core/src/action.rs`** — add an `Action` variant if a new native action is needed

No existing files change beyond these registration points.

### Coding standards

- **400 LOC hard limit** per file — split by responsibility when approaching
- **No inline comments** — names must be self-documenting; `///` doc-comments on public items only when necessary
- **Zero `unwrap()`** in non-test code — propagate with `?` or match explicitly
- **One command per file**, one domain type per file
- **Explicit `pub` boundaries** — only `lib.rs` re-exports; internal modules use `pub(crate)`

---

## Roadmap

| Phase | Status | Scope |
|-------|--------|-------|
| Phase 1 | ✅ Complete | macOS adapter, 30 commands, core engine |
| Phase 2 | 🔜 Planned | Windows (UIAutomation), Linux (AT-SPI), 10+ new commands |
| Phase 3 | 🔜 Planned | MCP server mode (`--mcp`), JSON Schema generation |
| Phase 4 | 🔜 Planned | Daemon, sessions, enterprise quality gates |

Phases 2–4 add adapters, transports, and hardening. The core engine is not rebuilt.

---

## License

Apache-2.0 — see [LICENSE](LICENSE).
