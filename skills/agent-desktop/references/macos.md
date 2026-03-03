# macOS Platform

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

### Surface-First Snapshot Rule

**When a snapshot shows a sheet, alert, popover, or menu is open, ALWAYS re-snapshot with the matching `--surface` flag.** The full app tree includes refs for the entire window behind the overlay — those refs are irrelevant and waste tokens. The surface snapshot returns only the overlay's refs, which are the only ones you can meaningfully interact with.

| Snapshot shows... | Next command |
|-------------------|-------------|
| A sheet is open (Save, Export, Print dialog) | `snapshot --app "App" --surface sheet -i` |
| An alert is visible (confirmation, error) | `snapshot --app "App" --surface alert -i` |
| A menu is open (dropdown, context menu) | `snapshot --app "App" --surface menu -i` |
| A popover appeared (tooltip, picker) | `snapshot --app "App" --surface popover -i` |

**Do not interact with refs from the full window snapshot while an overlay is open.** The overlay captures focus — clicks on background refs will either fail or dismiss the overlay unexpectedly. Snapshot the surface, interact with it, then snapshot the window again after the overlay closes.

## Notification Center

agent-desktop interacts with macOS Notification Center via the accessibility API.

### How It Works

1. **NcSession** opens Notification Center by clicking the clock in ControlCenter (if not already open)
2. Notifications are read from the AX tree under the NotificationCenter process
3. After operations, NcSession closes NC and restores the previously focused app
4. The `Drop` impl ensures cleanup even on errors

### Dismiss Strategy

Headless-first approach (no cursor movement unless needed):

1. **AXDismiss** / **AXRemoveFromParent** — native accessibility actions
2. **Close button** — find and press AXButton named "close", "clear", or "dismiss"
3. **Hover + close button** — move cursor to reveal hidden close button, then press it
4. If all fail, returns `ACTION_FAILED`

**Important:** `AXPress` is intentionally excluded from dismiss — it "clicks" the notification body (opening the source app) without actually dismissing it.

### Stacked Notifications

macOS groups notifications from the same app into stacks. Dismissing the top notification may reveal more underneath. `dismiss-all-notifications` iterates in reverse order but may need multiple rounds for deeply stacked groups.

### Notification Troubleshooting

| Problem | Cause | Fix |
|---------|-------|-----|
| Script Editor opens during dismiss | Notification owned by osascript | Fixed — NcSession restores previous app focus |
| `dismiss-notification` reports success but notification stays | AXPress was firing instead of AXDismiss | Fixed — AXPress removed from dismiss chain |
| Calendar widget can't be dismissed | It's a system widget, not a notification | Expected behavior — not a real notification |
| Notifications disappear before listing | Banner-style notifications are transient | Use `wait --notification` to detect them |

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
