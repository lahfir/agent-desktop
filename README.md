# agent-desktop

[![Build](https://github.com/lahfir/agent-desktop/actions/workflows/ci.yml/badge.svg)](https://github.com/lahfir/agent-desktop/actions)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Platform: macOS](https://img.shields.io/badge/platform-macOS-lightgrey.svg)]()

A cross-platform Rust CLI that gives AI agents structured access to native desktop applications through OS accessibility trees. It outputs JSON with ref-based element identifiers (`@e1`, `@e2`, ...) so agents can observe UI state, act on elements, and drive any application programmatically. **agent-desktop is not an AI agent** — it is the tool AI agents invoke.

## Quick example

```bash
# 1. Observe: get the accessibility tree with interactive element refs
agent-desktop snapshot --app "TextEdit" -i

# 2. Act: click a button by ref
agent-desktop click @e3

# 3. Type into a text field
agent-desktop type @e5 "quarterly report"

# 4. Submit with a keyboard shortcut
agent-desktop press cmd+return

# 5. Re-observe after the UI changes
agent-desktop snapshot -i
```

The observation-action loop lives in the calling agent, not here.

```
AI Agent
  |
  +-- agent-desktop snapshot -i          → {"tree": {...}, "ref_count": 14}
  |
  +-- agent-desktop click @e7            → {"ok": true, "data": {"action": "click"}}
  |
  +-- agent-desktop snapshot -i          → {"tree": {...}, "ref_count": 11}
```

## Installation

### Build from source

```bash
git clone https://github.com/lahfir/agent-desktop
cd agent-desktop
cargo build --release
```

The binary is at `./target/release/agent-desktop`. Move it to your PATH:

```bash
mv target/release/agent-desktop /usr/local/bin/
```

### Requirements

- **Rust** 1.78+ (pinned via `rust-toolchain.toml`)
- **macOS** 13.0+ (Phase 1 — the only supported platform today)

## Permissions

macOS requires Accessibility permission for agent-desktop to read UI trees and perform actions.

```bash
# Check current permission status
agent-desktop permissions

# Trigger the system permission dialog
agent-desktop permissions --request
```

Or grant manually: **System Settings > Privacy & Security > Accessibility** and add your terminal application.

If permission is missing, commands return a `PERM_DENIED` error with guidance.

## Commands

### Observation

#### `snapshot`

Capture the accessibility tree of a window as structured JSON with `@eN` ref IDs.

```bash
agent-desktop snapshot [OPTIONS]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--app <NAME>` | focused app | Filter to a specific application |
| `--window-id <ID>` | — | Filter to a specific window |
| `--max-depth <N>` | `10` | Maximum tree traversal depth |
| `--include-bounds` | off | Include pixel bounds for every node |
| `-i` / `--interactive-only` | off | Omit non-interactive elements from output |
| `--compact` | off | Omit empty structural nodes |
| `--surface <TYPE>` | `window` | Surface to snapshot: window, focused, menu, sheet, popover, alert |

```bash
agent-desktop snapshot --app "TextEdit" -i
agent-desktop snapshot --include-bounds --max-depth 15
agent-desktop snapshot --surface menu --app "Finder"
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

#### `screenshot`

Capture a PNG screenshot of a window.

```bash
agent-desktop screenshot [--app <NAME>] [--window-id <ID>] [PATH]
```

```bash
agent-desktop screenshot --app "Finder" ~/Desktop/finder.png
agent-desktop screenshot   # base64 PNG to stdout
```

#### `find`

Search the accessibility tree for elements matching a query.

```bash
agent-desktop find [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--app <NAME>` | Filter to application |
| `--role <ROLE>` | Match by role (button, textfield, checkbox ...) |
| `--name <TEXT>` | Match by accessible name or label |
| `--value <TEXT>` | Match by current value |
| `--text <TEXT>` | Match by text in name, value, title, or description |
| `--count` | Return match count only |
| `--first` | Return first match only |
| `--last` | Return last match only |
| `--nth <N>` | Return Nth match (0-indexed) |

```bash
agent-desktop find --role button --name "Save"
agent-desktop find --app "TextEdit" --text "hello" --first
agent-desktop find --role textfield --count
```

#### `get`

Read a property of an element by ref.

```bash
agent-desktop get <REF> --property <PROP>
```

Properties: `text` (default), `value`, `title`, `bounds`, `role`, `states`.

```bash
agent-desktop get @e3 --property value
agent-desktop get @e1 --property bounds
agent-desktop get @e5 --property states
```

#### `is`

Check a boolean state of an element.

```bash
agent-desktop is <REF> --property <PROP>
```

Properties: `visible` (default), `enabled`, `checked`, `focused`, `expanded`.

```bash
agent-desktop is @e4 --property enabled
agent-desktop is @e6 --property checked
```

#### `list-surfaces`

List accessibility surfaces for an application (window, menu, sheet, popover, alert).

```bash
agent-desktop list-surfaces [--app <NAME>]
```

### Interaction

#### `click` / `double-click` / `triple-click` / `right-click`

```bash
agent-desktop click <REF>
agent-desktop double-click <REF>
agent-desktop triple-click <REF>       # select line or paragraph
agent-desktop right-click <REF>        # open context menu
```

#### `type`

Focus an element and type text (simulates keyboard input).

```bash
agent-desktop type <REF> <TEXT>
```

```bash
agent-desktop type @e3 "hello@example.com"
```

#### `set-value`

Set an element's value directly via the accessibility attribute (bypasses key events).

```bash
agent-desktop set-value <REF> <VALUE>
```

```bash
agent-desktop set-value @e3 "2026-02-20"
```

#### `clear`

Clear an element's value to an empty string.

```bash
agent-desktop clear <REF>
```

#### `focus`

Set keyboard focus on an element.

```bash
agent-desktop focus <REF>
```

#### `select`

Select an option in a list or dropdown.

```bash
agent-desktop select <REF> <VALUE>
```

```bash
agent-desktop select @e8 "Last 30 days"
```

#### `toggle` / `check` / `uncheck`

```bash
agent-desktop toggle <REF>      # flip checkbox or switch state
agent-desktop check <REF>       # set to checked (idempotent)
agent-desktop uncheck <REF>     # set to unchecked (idempotent)
```

#### `expand` / `collapse`

Expand or collapse a disclosure triangle, tree item, or accordion.

```bash
agent-desktop expand <REF>
agent-desktop collapse <REF>
```

#### `scroll` / `scroll-to`

```bash
agent-desktop scroll <REF> [--direction up|down|left|right] [--amount <N>]
agent-desktop scroll-to <REF>   # scroll element into visible area
```

```bash
agent-desktop scroll @e2 --direction down --amount 5
agent-desktop scroll-to @e14
```

### Keyboard

#### `press`

Send a key combo. Modifiers: `cmd`, `ctrl`, `alt`, `shift`. Combine with `+`.

```bash
agent-desktop press <COMBO> [--app <NAME>]
```

```bash
agent-desktop press cmd+s
agent-desktop press cmd+shift+z
agent-desktop press escape
agent-desktop press return
agent-desktop press tab
agent-desktop press cmd+c --app "TextEdit"
```

#### `key-down` / `key-up`

Hold or release a key or modifier.

```bash
agent-desktop key-down shift
agent-desktop key-up shift
```

### Mouse

#### `hover`

Move the cursor to an element or absolute coordinates.

```bash
agent-desktop hover <REF>
agent-desktop hover --xy 500,300
agent-desktop hover @e5 --duration 2000    # hold for 2s
```

#### `drag`

Drag from one element or point to another.

```bash
agent-desktop drag --from @e1 --to @e5
agent-desktop drag --from-xy 100,200 --to-xy 400,500 --duration 500
```

#### `mouse-move` / `mouse-click` / `mouse-down` / `mouse-up`

Low-level mouse operations at absolute screen coordinates.

```bash
agent-desktop mouse-move --xy 500,300
agent-desktop mouse-click --xy 500,300 --button left --count 2
agent-desktop mouse-down --xy 100,200 --button left
agent-desktop mouse-up --xy 400,500 --button left
```

### App & window management

#### `launch`

Launch an application by name or bundle ID and wait for its window.

```bash
agent-desktop launch <APP> [--timeout <MS>]
```

```bash
agent-desktop launch "TextEdit"
agent-desktop launch "com.apple.finder" --timeout 10000
```

#### `close-app`

Quit an application.

```bash
agent-desktop close-app <APP> [--force]
```

```bash
agent-desktop close-app "TextEdit"
agent-desktop close-app "TextEdit" --force   # SIGKILL
```

#### `list-apps`

List all running GUI applications.

```bash
agent-desktop list-apps
```

#### `list-windows`

List all visible windows.

```bash
agent-desktop list-windows [--app <NAME>]
```

#### `focus-window`

Bring a window to the foreground.

```bash
agent-desktop focus-window --app "Finder" --title "Documents"
agent-desktop focus-window --window-id "w-4521"
```

#### `resize-window`

```bash
agent-desktop resize-window --app "TextEdit" --width 800 --height 600
```

#### `move-window`

```bash
agent-desktop move-window --app "TextEdit" --x 100 --y 50
```

#### `minimize` / `maximize` / `restore`

```bash
agent-desktop minimize --app "TextEdit"
agent-desktop maximize --app "TextEdit"
agent-desktop restore --app "TextEdit"
```

### Clipboard

```bash
agent-desktop clipboard-get          # read plain-text clipboard
agent-desktop clipboard-set "text"   # write text to clipboard
agent-desktop clipboard-clear        # clear the clipboard
```

### Wait

Block for a duration or until a condition is met.

```bash
agent-desktop wait [MS] [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `<MS>` | Sleep for N milliseconds |
| `--element <REF>` | Block until element appears |
| `--window <TITLE>` | Block until window appears |
| `--text <TEXT>` | Block until text appears in the app's tree |
| `--menu` | Block until a context menu is open |
| `--menu-closed` | Block until a context menu is dismissed |
| `--app <NAME>` | Scope wait to a specific application |
| `--timeout <MS>` | Timeout (default 30000) |

```bash
agent-desktop wait 500
agent-desktop wait --window "Save" --timeout 10000
agent-desktop wait --element @e3 --timeout 5000
agent-desktop wait --text "Loading complete" --app "Safari" --timeout 5000
agent-desktop wait --menu --timeout 3000
```

### System

```bash
agent-desktop status                  # adapter health, platform, permission state
agent-desktop permissions             # check accessibility permission
agent-desktop permissions --request   # trigger system dialog
agent-desktop version                 # version string
agent-desktop version --json          # machine-readable version
```

### Batch

Run multiple commands in a single invocation.

```bash
agent-desktop batch <JSON> [--stop-on-error]
```

```bash
agent-desktop batch '[
  {"command":"click",     "args":{"ref_id":"@e2"}},
  {"command":"type",      "args":{"ref_id":"@e5","text":"hello"}},
  {"command":"press",     "args":{"combo":"return"}}
]' --stop-on-error
```

## JSON output format

Every command produces a standard envelope:

```json
{
  "version": "1.0",
  "ok": true,
  "command": "click",
  "data": { "action": "click", "ref": "@e3" }
}
```

Errors include a machine-readable code and recovery hint:

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
| `ELEMENT_NOT_FOUND` | No element matched the ref or query |
| `APP_NOT_FOUND` | Application not running or no windows |
| `ACTION_FAILED` | The OS rejected the action |
| `ACTION_NOT_SUPPORTED` | Element does not support this action |
| `STALE_REF` | Ref is from a previous snapshot |
| `WINDOW_NOT_FOUND` | No window matched the ID or query |
| `PLATFORM_NOT_SUPPORTED` | Not implemented on this OS |
| `TIMEOUT` | Wait condition expired |
| `INVALID_ARGS` | Invalid argument values |
| `INTERNAL` | Unexpected internal error |

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Structured error (JSON on stdout) |
| `2` | Argument parse error |

## Ref system

`snapshot` assigns identifiers to every interactive element in depth-first order: `@e1`, `@e2`, `@e3`, etc. These refs are valid for action commands **until the next snapshot replaces them**.

Only interactive roles receive refs:

| | | |
|---|---|---|
| `button` | `textfield` | `checkbox` |
| `link` | `menuitem` | `tab` |
| `slider` | `combobox` | `treeitem` |
| `cell` | `radiobutton` | `incrementor` |

Static elements (labels, groups, containers) appear in the tree for context but have no ref and cannot be acted upon.

The refmap is stored at `~/.agent-desktop/last_refmap.json` (permissions `0600`) and fully replaced on every snapshot. Action commands perform optimistic re-identification — if the element has changed since the snapshot, they return `STALE_REF`.

Stale ref recovery pattern:

```
snapshot -> act -> if STALE_REF -> snapshot again -> retry
```

## Platform support

| Feature | macOS | Windows | Linux |
|---------|-------|---------|-------|
| Accessibility tree | Phase 1 | Planned (Phase 2) | Planned (Phase 2) |
| Click / type / keyboard | Phase 1 | Planned (Phase 2) | Planned (Phase 2) |
| Mouse input | Phase 1 | Planned (Phase 2) | Planned (Phase 2) |
| Screenshot | Phase 1 | Planned (Phase 2) | Planned (Phase 2) |
| Clipboard | Phase 1 | Planned (Phase 2) | Planned (Phase 2) |
| App launch / close | Phase 1 | Planned (Phase 2) | Planned (Phase 2) |
| Window management | Phase 1 | Planned (Phase 2) | Planned (Phase 2) |
| MCP server mode | Planned (Phase 3) | Planned (Phase 3) | Planned (Phase 3) |

macOS implementation uses `AXUIElement` for tree traversal and actions, `CGEvent` for keyboard and mouse input, `CGWindowListCreateImage` for screenshots, and `NSPasteboard` for clipboard.

## Architecture

The workspace follows strict dependency inversion. `agent-desktop-core` defines the `PlatformAdapter` trait and all shared types. Platform crates (`macos`, `windows`, `linux`) implement the trait. Core never imports platform crates — the binary crate is the only wiring point. This constraint is enforced in CI via `cargo tree`.

```
agent-desktop/
├── src/                    # binary crate (entry point, CLI, dispatch)
└── crates/
    ├── core/               # platform-agnostic types, commands, engine
    ├── macos/              # macOS adapter (Phase 1)
    ├── windows/            # stub (Phase 2)
    └── linux/              # stub (Phase 2)
```

## Contributing

### Build

```bash
cargo build                  # debug
cargo build --release        # optimized, stripped binary (<15MB)
```

### Test

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

### Adding a command

1. Create `crates/core/src/commands/{name}.rs` with an `execute()` function
2. Register in `crates/core/src/commands/mod.rs`
3. Add subcommand variant to `src/cli.rs`
4. Add match arm in `src/dispatch.rs`
5. If needed: add `Action` variant in `crates/core/src/action.rs`
6. If needed: add adapter method to `PlatformAdapter` with default `not_supported()` impl

### Standards

- 400 LOC hard limit per file
- No inline comments — code must be self-documenting
- Zero `unwrap()` in non-test code
- One command per file, one domain type per file

## License

Apache-2.0
