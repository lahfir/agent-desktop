# agent-desktop

[![Build](https://github.com/lahfir/agent-desktop/actions/workflows/ci.yml/badge.svg)](https://github.com/lahfir/agent-desktop/actions)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Platform: macOS](https://img.shields.io/badge/platform-macOS-lightgrey.svg)]()

A cross-platform Rust CLI that gives AI agents structured access to native desktop applications through OS accessibility trees. Outputs JSON with ref-based element identifiers (`@e1`, `@e2`, ...) so agents can observe UI state, act on elements, and drive any application programmatically.

**agent-desktop is not an AI agent** — it is the tool AI agents invoke. The observation-action loop lives in the calling agent.

```
AI Agent
  |
  +-- agent-desktop snapshot --app "Finder" -i  → { "tree": {...}, "ref_count": 83 }
  |
  +-- agent-desktop right-click @e66            → { "action": "right_click", "menu": {...} }
  |
  +-- agent-desktop click @e72                  → { "action": "click" }
```

## Installation

```bash
git clone https://github.com/lahfir/agent-desktop
cd agent-desktop
cargo build --release
mv target/release/agent-desktop /usr/local/bin/
```

**Requirements:** Rust 1.78+, macOS 13.0+

## Permissions

macOS requires Accessibility permission to read UI trees and perform actions.

```bash
agent-desktop permissions            # check status
agent-desktop permissions --request  # trigger system dialog
```

Or grant manually: **System Settings > Privacy & Security > Accessibility** — add your terminal app.

## Quick start

```bash
# observe: get the accessibility tree with interactive element refs
agent-desktop snapshot --app "TextEdit" -i

# act: click a button by ref
agent-desktop click @e3

# type into a text field
agent-desktop type @e5 "quarterly report"

# keyboard shortcut
agent-desktop press cmd+return

# right-click returns the context menu inline
agent-desktop right-click @e8

# re-observe after the UI changes
agent-desktop snapshot -i
```

## How interactions work

agent-desktop uses an **AX-first** approach for all interactions. Every action exhausts multiple Accessibility API strategies before falling back to CGEvent (which moves the cursor and requires the window to be frontmost).

### Click chain (13 steps)

1. AXScrollToVisible (ensure element is in viewport)
2. AXPress
3. AXConfirm
4. AXOpen
5. AXPick
6. AXShowAlternateUI + retry children
7. Try child activation (first 3 children)
8. Set AXSelected=true
9. Set AXSelectedRows on parent table/outline/list
10. AXCustomActions
11. Set AXFocused + AXConfirm/AXPress
12. AXUIElementPostKeyboardEvent(Space) to app
13. Try parent activation (walk up 2 ancestors)
14. CGEvent click at center **(last resort)**

### Right-click chain (7 steps)

1. AXShowMenu on element
2. Focus app via AX + AXShowMenu (fixes -25204 CannotComplete)
3. Select element + AXShowMenu
4. Focus element + AXShowMenu
5. AXShowMenu on parent (walk up 3 ancestors)
6. AXShowMenu on child (first 5 children)
7. CGEvent right-click **(last resort)**

The `right-click` command returns the full context menu tree inline with ref_ids on all items, so the agent can immediately click a menu item without a separate snapshot.

### Scroll chain (10 steps)

1. AXScrollToVisible on target element
2. AXIncrement/AXDecrement on AXScrollBar
3. AXScrollDownByPage/AXScrollUpByPage on AXScrollArea
4. Set AXValue (float 0.0-1.0) on AXScrollBar
5. AXPress on scroll bar sub-elements (page/arrow parts)
6. Set AXFocused=true on child in scroll direction
7. Set AXSelectedRows on parent table/outline/list
8. AXUIElementPostKeyboardEvent (Page Up/Down)
9. AXUIElementPostKeyboardEvent (arrow keys)
10. CGEventCreateScrollWheelEvent **(last resort)**

Steps 1-7 are pure AX (no window focus required, works in background). Steps 8-9 use AX keyboard events (need app focus, no cursor movement). Step 10 uses CGEvent (needs focus + screen coordinates).

All CGEvent fallbacks include a focus guard (`ensure_app_focused`) that brings the target app to front via AX before posting events.

## Commands

### Observation

| Command | Description |
|---------|-------------|
| `snapshot` | Capture accessibility tree as JSON with `@eN` refs |
| `screenshot` | Capture PNG screenshot of a window |
| `find` | Search tree for elements by role, name, value, or text |
| `get` | Read a property of an element (text, value, title, bounds, role, states) |
| `is` | Check a boolean state (visible, enabled, checked, focused, expanded) |
| `list-surfaces` | List open surfaces (menus, sheets, popovers, alerts) |

### Interaction

| Command | Description |
|---------|-------------|
| `click` | Smart AX-first click (13-step chain) |
| `double-click` | AXOpen first, then double-activate chain |
| `triple-click` | Select line/paragraph |
| `right-click` | Open context menu via AX, returns menu tree inline |
| `type` | Focus element and type text via keyboard synthesis |
| `set-value` | Set element value directly via AX attribute |
| `clear` | Clear element value to empty string |
| `focus` | Set keyboard focus on element |
| `select` | Select option in list or dropdown |
| `toggle` | Flip checkbox or switch state |
| `check` / `uncheck` | Idempotent checked/unchecked |
| `expand` / `collapse` | Disclosure triangle, tree item, accordion |
| `scroll` | Scroll element (10-step AX-first chain) |
| `scroll-to` | Scroll element into visible area |

### Keyboard

| Command | Description |
|---------|-------------|
| `press` | Send key combo (e.g. `cmd+s`, `cmd+shift+z`, `escape`) |
| `key-down` / `key-up` | Hold or release a key |

### Mouse (raw coordinates)

| Command | Description |
|---------|-------------|
| `hover` | Move cursor to element or coordinates |
| `drag` | Drag from one point/element to another |
| `mouse-move` | Move cursor to absolute coordinates |
| `mouse-click` | Click at absolute coordinates |
| `mouse-down` / `mouse-up` | Press/release at coordinates |

### App & window management

| Command | Description |
|---------|-------------|
| `launch` | Launch app by name or bundle ID |
| `close-app` | Quit app (optional `--force` for SIGKILL) |
| `list-apps` | List running GUI applications |
| `list-windows` | List visible windows |
| `focus-window` | Bring window to foreground |
| `resize-window` | Resize a window |
| `move-window` | Move a window |
| `minimize` / `maximize` / `restore` | Window state |

### Clipboard

| Command | Description |
|---------|-------------|
| `clipboard-get` | Read plain-text clipboard |
| `clipboard-set` | Write text to clipboard |
| `clipboard-clear` | Clear the clipboard |

### Wait

Block for a duration or until a condition is met.

```bash
agent-desktop wait 500                                        # sleep 500ms
agent-desktop wait --window "Save" --timeout 10000            # wait for window
agent-desktop wait --element @e3 --timeout 5000               # wait for element
agent-desktop wait --text "Loading complete" --app "Safari"   # wait for text
agent-desktop wait --menu --timeout 3000                      # wait for menu
```

### Batch

Run multiple commands in a single invocation.

```bash
agent-desktop batch '[
  {"command":"click",  "args":{"ref_id":"@e2"}},
  {"command":"type",   "args":{"ref_id":"@e5","text":"hello"}},
  {"command":"press",  "args":{"combo":"return"}}
]' --stop-on-error
```

### System

```bash
agent-desktop status                  # adapter health, platform, permission state
agent-desktop permissions             # check accessibility permission
agent-desktop permissions --request   # trigger system dialog
agent-desktop version                 # version string
```

## Snapshot options

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
| `--surface <TYPE>` | `window` | Surface: window, focused, menu, menubar, sheet, popover, alert |

## JSON output

Every command produces a standard envelope:

```json
{
  "version": "1.0",
  "ok": true,
  "command": "click",
  "data": { "action": "click" }
}
```

Right-click includes the context menu tree:

```json
{
  "version": "1.0",
  "ok": true,
  "command": "right-click",
  "data": {
    "action": "right_click",
    "menu": {
      "role": "menu",
      "children": [
        { "role": "menuitem", "name": "Open in New Tab", "ref_id": "@e71" },
        { "role": "menuitem", "name": "Open in New Window", "ref_id": "@e72" },
        { "role": "menuitem", "name": "Get Info", "ref_id": "@e78" },
        { "role": "menuitem", "name": "Rename", "ref_id": "@e80" }
      ]
    }
  }
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

`snapshot` assigns identifiers to every interactive element in depth-first order: `@e1`, `@e2`, `@e3`, etc. These refs are valid for action commands until the next snapshot replaces them.

Interactive roles that receive refs:

| | | | |
|---|---|---|---|
| `button` | `textfield` | `checkbox` | `link` |
| `menuitem` | `tab` | `slider` | `combobox` |
| `treeitem` | `cell` | `radiobutton` | `incrementor` |
| `menubutton` | `switch` | `colorwell` | `dockitem` |

Static elements (labels, groups, containers) appear in the tree for context but have no ref.

The refmap is stored at `~/.agent-desktop/last_refmap.json` and fully replaced on every snapshot. Action commands perform optimistic re-identification using `(pid, role, name, bounds_hash)` — if the element has moved or changed, they return `STALE_REF`.

Stale ref recovery:

```
snapshot → act → if STALE_REF → snapshot again → retry
```

## Platform support

| Feature | macOS | Windows | Linux |
|---------|-------|---------|-------|
| Accessibility tree | Phase 1 | Planned | Planned |
| Click / type / keyboard | Phase 1 | Planned | Planned |
| Mouse input | Phase 1 | Planned | Planned |
| Screenshot | Phase 1 | Planned | Planned |
| Clipboard | Phase 1 | Planned | Planned |
| App launch / close | Phase 1 | Planned | Planned |
| Window management | Phase 1 | Planned | Planned |
| MCP server mode | Planned | Planned | Planned |

macOS uses `AXUIElement` for tree traversal and actions, `CGEvent` for keyboard/mouse, `CGWindowListCreateImage` for screenshots, and `NSPasteboard` for clipboard.

## Architecture

Strict dependency inversion. `agent-desktop-core` defines the `PlatformAdapter` trait and all shared types. Platform crates implement the trait. Core never imports platform crates — the binary crate is the only wiring point. Enforced in CI via `cargo tree`.

```
agent-desktop/
├── src/                    # binary crate (entry point, CLI, dispatch)
└── crates/
    ├── core/               # platform-agnostic types, commands, engine
    ├── macos/              # macOS adapter (Phase 1)
    │   ├── tree/           # AX tree reading, element resolution, surfaces
    │   ├── actions/        # smart interaction chains (click, scroll, right-click)
    │   ├── input/          # keyboard, mouse, clipboard synthesis
    │   └── system/         # app lifecycle, windows, permissions
    ├── windows/            # stub (Phase 2)
    └── linux/              # stub (Phase 2)
```

## Contributing

```bash
cargo build                              # debug build
cargo build --release                    # optimized (<15MB)
cargo test --workspace                   # run tests
cargo clippy --all-targets -- -D warnings # lint
```

### Adding a command

1. Create `crates/core/src/commands/{name}.rs` with an `execute()` function
2. Register in `crates/core/src/commands/mod.rs`
3. Add subcommand variant to `src/cli.rs`
4. Add match arm in dispatch
5. If needed: add `Action` variant in `crates/core/src/action.rs`
6. If needed: add adapter method to `PlatformAdapter` with default `not_supported()` impl

### Standards

- 400 LOC hard limit per file
- No inline comments — code must be self-documenting
- Zero `unwrap()` in non-test code
- One command per file, one domain type per file

## License

Apache-2.0
