---
name: agent-desktop
description: >
  Desktop automation via native OS accessibility trees using the agent-desktop CLI.
  Use when an AI agent needs to observe, interact with, or automate desktop applications
  (click buttons, fill forms, navigate menus, read UI state, toggle checkboxes, scroll,
  drag, type text, take screenshots, manage windows, use clipboard). Covers 50 commands
  across observation, interaction, keyboard/mouse, app lifecycle, clipboard, and wait.
  Triggers on: "click button", "fill form", "open app", "read UI", "automate desktop",
  "accessibility tree", "snapshot app", "type into field", "navigate menu", "toggle checkbox",
  "take screenshot", "desktop automation", "agent-desktop", or any desktop GUI interaction task.
  Supports macOS (Phase 1), with Windows and Linux planned.
---

# agent-desktop

CLI tool enabling AI agents to observe and control desktop applications via native OS accessibility trees.

**Core principle:** agent-desktop is NOT an AI agent. It is a tool that AI agents invoke. It outputs structured JSON with ref-based element identifiers. The observation-action loop lives in the calling agent.

## Installation

```bash
npm install -g agent-desktop
# or
bun install -g --trust agent-desktop
```

Requires macOS 12+ with Accessibility permission granted to your terminal.

## Reference Files

Detailed documentation is split into focused reference files. Read them as needed:

| Reference | Contents |
|-----------|----------|
| `references/commands-observation.md` | snapshot, find, get, is, screenshot, list-surfaces — all flags, output examples |
| `references/commands-interaction.md` | click, type, set-value, select, toggle, scroll, drag, keyboard, mouse — choosing the right command |
| `references/commands-system.md` | launch, close, windows, clipboard, wait, batch, status, permissions, version |
| `references/workflows.md` | 12 common patterns: forms, menus, dialogs, scroll-find, drag-drop, async wait, anti-patterns |
| `references/macos.md` | macOS permissions/TCC, AX API internals, smart activation chain, surfaces, troubleshooting |

## The Observe-Act Loop

Every automation follows this pattern:

```
1. OBSERVE  → agent-desktop snapshot --app "App Name" -i
2. REASON   → Parse JSON, find target element by ref (@e1, @e2...)
3. ACT      → agent-desktop click @e5  (or type, select, toggle...)
4. VERIFY   → agent-desktop snapshot again to confirm state change
5. REPEAT   → Continue until task is complete
```

Always snapshot before acting. Refs are snapshot-scoped and become stale after UI changes.

## Ref System

- Refs assigned depth-first: `@e1`, `@e2`, `@e3`...
- Only interactive elements get refs: button, textfield, checkbox, link, menuitem, tab, slider, combobox, treeitem, cell
- Static text, groups, containers remain in tree for context but have no ref
- Refs are deterministic within a snapshot but NOT stable across snapshots if UI changed
- After any action that changes UI, run `snapshot` again for fresh refs

## JSON Output Contract

Every command returns a JSON envelope on stdout:

**Success:** `{ "version": "1.0", "ok": true, "command": "snapshot", "data": { ... } }`
**Error:** `{ "version": "1.0", "ok": false, "command": "click", "error": { "code": "STALE_REF", "message": "...", "suggestion": "..." } }`

Exit codes: `0` success, `1` structured error, `2` argument error.

### Error Codes

| Code | Meaning | Recovery |
|------|---------|----------|
| `PERM_DENIED` | Accessibility permission not granted | Grant in System Settings > Privacy > Accessibility |
| `ELEMENT_NOT_FOUND` | Ref not in current refmap | Re-run snapshot, use fresh ref |
| `APP_NOT_FOUND` | App not running | Launch it first |
| `ACTION_FAILED` | AX action rejected | Try alternative approach or coordinate-based click |
| `ACTION_NOT_SUPPORTED` | Element can't do this | Use different command |
| `STALE_REF` | Ref from old snapshot | Re-run snapshot |
| `WINDOW_NOT_FOUND` | No matching window | Check app name, use list-windows |
| `TIMEOUT` | Wait condition not met | Increase --timeout |
| `INVALID_ARGS` | Bad arguments | Check command syntax |

## Command Quick Reference (50 commands)

### Observation
```
agent-desktop snapshot --app "App" -i           # Accessibility tree with refs
agent-desktop screenshot --app "App" out.png    # PNG screenshot
agent-desktop find --app "App" --role button    # Search elements
agent-desktop get @e1 --property text           # Read element property
agent-desktop is @e1 --property enabled         # Check element state
agent-desktop list-surfaces --app "App"         # Available surfaces
```

### Interaction
```
agent-desktop click @e5                         # Click element
agent-desktop double-click @e3                  # Double-click
agent-desktop triple-click @e2                  # Triple-click (select line)
agent-desktop right-click @e5                   # Right-click (context menu)
agent-desktop type @e2 "hello"                  # Type text into element
agent-desktop set-value @e2 "new value"         # Set value directly
agent-desktop clear @e2                         # Clear element value
agent-desktop focus @e2                         # Set keyboard focus
agent-desktop select @e4 "Option B"             # Select dropdown option
agent-desktop toggle @e6                        # Toggle checkbox/switch
agent-desktop check @e6                         # Idempotent check
agent-desktop uncheck @e6                       # Idempotent uncheck
agent-desktop expand @e7                        # Expand disclosure
agent-desktop collapse @e7                      # Collapse disclosure
agent-desktop scroll @e1 --direction down       # Scroll element
agent-desktop scroll-to @e8                     # Scroll into view
```

### Keyboard & Mouse
```
agent-desktop press cmd+c                       # Key combo
agent-desktop press return --app "App"          # Targeted key press
agent-desktop key-down shift                    # Hold key
agent-desktop key-up shift                      # Release key
agent-desktop hover @e5                         # Cursor to element
agent-desktop hover --xy 500,300                # Cursor to coordinates
agent-desktop drag --from @e1 --to @e5          # Drag between elements
agent-desktop mouse-click --xy 500,300          # Click at coordinates
agent-desktop mouse-move --xy 100,200           # Move cursor
agent-desktop mouse-down --xy 100,200           # Press mouse button
agent-desktop mouse-up --xy 300,400             # Release mouse button
```

### App & Window
```
agent-desktop launch "System Settings"          # Launch and wait
agent-desktop close-app "TextEdit"              # Quit gracefully
agent-desktop close-app "TextEdit" --force      # Force kill
agent-desktop list-windows --app "Finder"       # List windows
agent-desktop list-apps                         # List running GUI apps
agent-desktop focus-window --app "Finder"       # Bring to front
agent-desktop resize-window --app "App" --width 800 --height 600
agent-desktop move-window --app "App" --x 0 --y 0
agent-desktop minimize --app "App"
agent-desktop maximize --app "App"
agent-desktop restore --app "App"
```

### Clipboard
```
agent-desktop clipboard-get                     # Read clipboard
agent-desktop clipboard-set "text"              # Write to clipboard
agent-desktop clipboard-clear                   # Clear clipboard
```

### Wait
```
agent-desktop wait 1000                         # Pause 1 second
agent-desktop wait --element @e5 --timeout 5000 # Wait for element
agent-desktop wait --window "Title"             # Wait for window
agent-desktop wait --text "Done" --app "App"    # Wait for text
agent-desktop wait --menu --app "App"           # Wait for context menu
agent-desktop wait --menu-closed --app "App"    # Wait for menu dismissal
```

### System
```
agent-desktop status                            # Health check
agent-desktop permissions                       # Check permission
agent-desktop permissions --request             # Trigger permission dialog
agent-desktop version --json                    # Version info
agent-desktop batch '[...]' --stop-on-error     # Batch commands
```

## Key Principles for Agents

1. **Always snapshot first.** Never assume UI state.
2. **Use `-i` flag.** Filters to interactive elements only, reducing tokens.
3. **Refs are ephemeral.** Snapshot again after any UI-changing action.
4. **Prefer refs over coordinates.** `click @e5` > `mouse-click --xy 500,300`.
5. **Use `wait` for async UI.** After launch/dialog triggers, wait for expected state.
6. **Check permissions first.** Run `permissions` on first use.
7. **Handle errors.** Parse `error.code` and follow `error.suggestion`.
8. **Use `find` for targeted searches.** Faster than full snapshot when you know role/name.
9. **Use surfaces for menus.** `snapshot --surface menu` captures open menus.
10. **Batch for performance.** Multiple commands in one invocation.
