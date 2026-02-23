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

Cross-platform CLI tool enabling AI agents to observe and control desktop applications via native OS accessibility trees.

**Core principle:** agent-desktop is NOT an AI agent. It is a tool that AI agents invoke. It outputs structured JSON with ref-based element identifiers. The observation-action loop lives in the calling agent.

## When to Use

Use agent-desktop when you need to:
- Read UI state from desktop applications (buttons, text fields, menus, checkboxes)
- Interact with desktop app elements (click, type, select, toggle, scroll)
- Automate multi-step desktop workflows (fill forms, navigate menus, transfer data)
- Wait for UI state changes before proceeding
- Take screenshots of application windows

Do NOT use agent-desktop for:
- Web browser automation (use agent-browser instead)
- Custom-rendered or game-engine UIs lacking accessibility exposure
- Applications that don't expose accessibility trees

## Skill Graph

This skill is the index. Platform-specific details and advanced patterns are in sub-skills:

| Skill | When to use |
|-------|-------------|
| `agent-desktop-macos` | macOS permissions, AX API quirks, troubleshooting |
| `commands-observation.md` | Snapshot, find, get, is, screenshot, list-surfaces |
| `commands-interaction.md` | Click, type, set-value, select, toggle, expand, scroll, drag |
| `commands-system.md` | Launch, close, windows, clipboard, wait, batch |
| `workflows.md` | Common automation patterns (forms, menus, dialogs, navigation) |

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

- Refs are assigned in depth-first document order: `@e1`, `@e2`, `@e3`...
- Only interactive elements receive refs: button, textfield, checkbox, link, menuitem, tab, slider, combobox, treeitem, cell
- Static text, groups, and containers do NOT get refs (they remain in the tree for context)
- Refs are deterministic within a snapshot but NOT stable across snapshots if the UI changed
- After any action that changes UI, run `snapshot` again to get fresh refs

## JSON Output Contract

Every command returns a JSON envelope on stdout:

### Success
```json
{
  "version": "1.0",
  "ok": true,
  "command": "snapshot",
  "data": { ... }
}
```

### Error
```json
{
  "version": "1.0",
  "ok": false,
  "command": "click",
  "error": {
    "code": "STALE_REF",
    "message": "RefMap is from a previous snapshot",
    "suggestion": "Run 'snapshot' to refresh, then retry with updated ref"
  }
}
```

### Exit Codes
- `0` — success (check `ok: true`)
- `1` — structured error (JSON with error code)
- `2` — argument/parse error

### Error Codes
| Code | Meaning | Recovery |
|------|---------|----------|
| `PERM_DENIED` | Accessibility permission not granted | Grant in System Settings > Privacy > Accessibility |
| `ELEMENT_NOT_FOUND` | Ref not found in current refmap | Re-run snapshot, use fresh ref |
| `APP_NOT_FOUND` | Application not running | Launch it first with `launch` |
| `ACTION_FAILED` | Accessibility action rejected | Try alternative approach (different action or coordinate-based) |
| `ACTION_NOT_SUPPORTED` | Element doesn't support this action | Check available actions, use different command |
| `STALE_REF` | Ref from old snapshot | Re-run snapshot to get fresh refs |
| `WINDOW_NOT_FOUND` | No matching window | Check app name, use `list-windows` |
| `PLATFORM_NOT_SUPPORTED` | Feature not available on this OS | Check platform support |
| `TIMEOUT` | Wait condition not met in time | Increase `--timeout` or check condition |
| `INVALID_ARGS` | Bad arguments | Check command syntax |
| `INTERNAL` | Unexpected internal error | Report bug with `-v` verbose output |

## Command Quick Reference

### Observation (see commands-observation.md)
```
snapshot --app "App" -i           # Accessibility tree with refs
screenshot --app "App" out.png    # PNG screenshot
find --app "App" --role button    # Search elements
get @e1 --property text           # Read element property
is @e1 --property enabled         # Check element state
list-surfaces --app "App"         # Available surfaces
```

### Interaction (see commands-interaction.md)
```
click @e5                         # Click element
double-click @e3                  # Double-click
right-click @e5                   # Right-click (context menu)
type @e2 "hello"                  # Type text into element
set-value @e2 "new value"         # Set value directly
clear @e2                         # Clear element value
focus @e2                         # Set keyboard focus
select @e4 "Option B"             # Select dropdown option
toggle @e6                        # Toggle checkbox/switch
check @e6                         # Idempotent check
uncheck @e6                       # Idempotent uncheck
expand @e7                        # Expand disclosure
collapse @e7                      # Collapse disclosure
scroll @e1 --direction down       # Scroll element
scroll-to @e8                     # Scroll element into view
```

### Keyboard & Mouse (see commands-interaction.md)
```
press cmd+c                       # Key combo
press return --app "App"          # Key combo targeted at app
key-down shift                    # Hold key
key-up shift                      # Release key
hover @e5                         # Move cursor to element
hover --xy 500,300                # Move cursor to coordinates
drag --from @e1 --to @e5          # Drag between elements
mouse-click --xy 500,300          # Click at coordinates
mouse-move --xy 100,200           # Move cursor
mouse-down --xy 100,200           # Press mouse button
mouse-up --xy 300,400             # Release mouse button
```

### App & Window (see commands-system.md)
```
launch "System Settings"          # Launch and wait for window
close-app "TextEdit"              # Quit gracefully
close-app "TextEdit" --force      # Force kill
list-windows --app "Finder"       # List windows
list-apps                         # List running GUI apps
focus-window --app "Finder"       # Bring window to front
resize-window --app "App" --width 800 --height 600
move-window --app "App" --x 0 --y 0
minimize --app "App"
maximize --app "App"
restore --app "App"
```

### Clipboard (see commands-system.md)
```
clipboard-get                     # Read clipboard
clipboard-set "text"              # Write to clipboard
clipboard-clear                   # Clear clipboard
```

### Wait (see commands-system.md)
```
wait 1000                         # Pause 1 second
wait --element @e5 --timeout 5000 # Wait for element
wait --window "Title"             # Wait for window
wait --text "Done" --app "App"    # Wait for text
wait --menu --app "App"           # Wait for context menu
wait --menu-closed --app "App"    # Wait for menu dismissal
```

### System (see commands-system.md)
```
status                            # Health check
permissions                       # Check accessibility permission
permissions --request             # Trigger system permission dialog
version                           # Version string
version --json                    # Machine-readable version
batch '[...]'                     # Run multiple commands
```

## Global Flag

All commands accept `--verbose` / `-v` for debug logging to stderr.

## Key Principles for Agents

1. **Always snapshot first.** Never assume UI state. Snapshot, parse, then act.
2. **Use `-i` flag.** `snapshot --app "App" -i` filters to interactive elements only, reducing token count.
3. **Refs are ephemeral.** After any action that changes UI, snapshot again.
4. **Prefer AX actions over coordinates.** `click @e5` is more reliable than `mouse-click --xy 500,300`.
5. **Use `wait` for async UI.** After launching apps or triggering dialogs, wait for the expected state.
6. **Check permissions first.** Run `permissions` on first use. macOS requires Accessibility permission.
7. **Handle errors gracefully.** Parse the JSON `error.code` field and follow `error.suggestion`.
8. **Use `find` for targeted searches.** When you know the role and name, `find` is faster than parsing a full snapshot.
9. **Use surfaces for menus.** `snapshot --surface menu` captures open menus and context menus.
10. **Batch for performance.** Use `batch` to run multiple commands in a single invocation when order matters.
