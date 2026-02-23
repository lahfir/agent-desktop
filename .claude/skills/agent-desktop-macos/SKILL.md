---
name: agent-desktop-macos
description: >
  macOS platform details for agent-desktop. Use when troubleshooting macOS accessibility
  permissions (TCC), debugging AX API failures, understanding the smart activation chain,
  working with macOS surfaces (menu, sheet, alert), or handling macOS-specific errors like
  PERM_DENIED, ACTION_FAILED, or empty accessibility trees.
---

# agent-desktop: macOS Platform

macOS-specific details for agent-desktop. Covers permissions, accessibility API behavior, troubleshooting, and platform quirks.

## Prerequisites

### Accessibility Permission (TCC)

macOS requires explicit Accessibility permission for any process that reads or controls UI elements across applications.

```bash
# Check permission status
agent-desktop permissions

# Trigger the system dialog
agent-desktop permissions --request
```

**To grant manually:**
1. Open System Settings > Privacy & Security > Accessibility
2. Click the lock to make changes
3. Add your terminal application (Terminal.app, iTerm2, Warp, VS Code, etc.)
4. Toggle it ON

**Important:** The permission is granted to the **terminal application**, not to agent-desktop itself. If you run agent-desktop from a different terminal, you need to grant that terminal too.

After granting, restart the terminal for the permission to take effect.

### Supported macOS Versions

- macOS 12 (Monterey) and later
- Both Intel (x86_64) and Apple Silicon (ARM64)

## macOS Accessibility API (AX)

agent-desktop uses the macOS Accessibility API (`AXUIElement`) to read and manipulate UI elements.

### How It Works

1. `AXUIElementCreateApplication(pid)` creates a handle to an app's accessibility tree
2. Tree traversal reads `kAXChildrenAttribute` recursively
3. Attributes like `kAXRoleAttribute`, `kAXNameAttribute`, `kAXValueAttribute` provide element details
4. Actions like `kAXPressAction`, `kAXConfirmAction` trigger element behavior

### Smart Activation Chain

When you run `click @ref`, agent-desktop doesn't just do a simple click. It runs a multi-step activation chain:

1. **AXScrollToVisible** — ensure element is on screen
2. **AXPress** — standard press action
3. **AXConfirm** — confirmation action
4. **AXOpen** — open action (for links, files)
5. **AXPick** — picker action
6. **AXShowAlternateUI** — reveal hidden UI, then press child
7. **Child activation** — try pressing child elements
8. **AXSelected** — set selected attribute
9. **Select via parent** — set parent's selected rows (for tables/lists)
10. **Custom actions** — AXPerformCustomAction
11. **Focus + activate** — set focus then press/confirm
12. **Keyboard activate** — focus + synthesize space key
13. **Parent activation** — try pressing ancestor elements
14. **Coordinate click** — final fallback: CGEvent click at element bounds center

For `right-click`, the chain tries AXShowMenu first, then various focus/select combinations before falling back to a coordinate-based right-click.

### Surfaces

macOS apps can have multiple accessibility surfaces:

| Surface | Description | When to use |
|---------|-------------|-------------|
| `window` | Main application window (default) | General UI interaction |
| `focused` | Currently focused element's context | Inspecting active element |
| `menu` | Open dropdown or context menu | After click/right-click on menu triggers |
| `menubar` | Application menu bar | Navigating File/Edit/View menus |
| `sheet` | Modal sheet (Save dialog, etc.) | After triggering sheet dialogs |
| `popover` | Popover/popup content | Inspecting tooltips, popovers |
| `alert` | System or app alert | Handling alert dialogs |

```bash
# List available surfaces
agent-desktop list-surfaces --app "Finder"

# Snapshot a specific surface
agent-desktop snapshot --app "Finder" --surface menu -i
```

## Troubleshooting

### PERM_DENIED

```
"code": "PERM_DENIED",
"message": "Accessibility permission not granted"
```

**Fix:**
1. `agent-desktop permissions --request` to trigger dialog
2. System Settings > Privacy & Security > Accessibility
3. Add and enable your terminal
4. Restart terminal

### Empty or Sparse Tree

Some apps don't expose full accessibility trees:
- **Electron apps** (VS Code, Slack): Generally good accessibility support
- **Custom-rendered UIs** (games, some creative tools): May have no accessibility tree
- **Web views**: Content inside WebViews may be limited

**Try:**
- Remove `-i` flag to see all elements including non-interactive ones
- Increase `--max-depth` to explore deeper
- Use `screenshot` as a visual fallback

### STALE_REF

```
"code": "STALE_REF",
"message": "RefMap is from a previous snapshot"
```

The UI changed between your snapshot and action. Run `snapshot` again and use the new refs.

### ACTION_FAILED

```
"code": "ACTION_FAILED"
```

The accessibility action was rejected. This can happen when:
- The element is disabled
- The app is busy or unresponsive
- The element doesn't support the requested action

**Try:**
1. Check `is @ref --property enabled` first
2. Try coordinate-based click: get bounds with `get @ref --property bounds`, then `mouse-click --xy x,y`
3. Use keyboard: `focus @ref` then `press return`

### APP_NOT_FOUND

The application isn't running. Launch it first:
```bash
agent-desktop launch "App Name"
```

### Slow Snapshots

Large apps (Xcode, Safari with many tabs) can have deep trees.

**Optimize:**
- Use `-i` to filter to interactive elements only
- Use `--max-depth 5` to limit depth
- Use `--compact` to remove empty structural nodes
- Target a specific window with `--window-id`
- Use `find` instead of full snapshot when you know what you're looking for

### Context Menu Doesn't Appear

After `right-click @ref`, if no menu appears:
1. The element may not support context menus
2. The app may need to be focused first: `focus-window --app "App"` before right-clicking
3. Try `mouse-click --xy x,y --button right` with coordinates from `get @ref --property bounds`

## macOS-Specific Behavior

### App Identification

Apps can be referenced by:
- **Display name:** `"System Settings"`, `"Finder"`, `"TextEdit"`
- **Bundle ID:** `"com.apple.systempreferences"`, `"com.apple.finder"`

`launch` accepts both. Other commands use the display name with `--app`.

### Window IDs

Window IDs (like `w-4521`) are assigned by macOS and are stable for the lifetime of the window. Use `list-windows` to discover them.

### Keyboard Shortcuts

macOS uses `cmd` (Command) as the primary modifier:
```bash
agent-desktop press cmd+c   # Copy
agent-desktop press cmd+v   # Paste
agent-desktop press cmd+z   # Undo
agent-desktop press cmd+s   # Save
agent-desktop press cmd+w   # Close window
agent-desktop press cmd+q   # Quit app (use with caution!)
```

### Full-Screen Apps

Apps in full-screen mode are accessible but may behave differently:
- `list-windows` still shows them
- Bounds may report full screen dimensions
- Some animations may delay UI state updates (use `wait`)

### Menu Bar Apps

Menu bar apps (status bar items) can be accessed via `--surface menubar`. The menu bar is a separate surface from the application window.
