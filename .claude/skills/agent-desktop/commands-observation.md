# Observation Commands

Commands for reading UI state without modifying it.

## snapshot

Capture the accessibility tree as structured JSON with `@ref` IDs.

```bash
agent-desktop snapshot --app "System Settings" -i
agent-desktop snapshot --app "Finder" --max-depth 5 --include-bounds
agent-desktop snapshot --app "App" --surface menu
agent-desktop snapshot --app "App" --window-id "w-1234"
agent-desktop snapshot --app "App" -i --compact
```

| Flag | Default | Description |
|------|---------|-------------|
| `--app` | (required) | Application name |
| `--window-id` | | Specific window ID from `list-windows` |
| `-i` / `--interactive-only` | false | Only include interactive elements (buttons, fields, etc.) |
| `--max-depth` | 10 | Maximum tree traversal depth |
| `--include-bounds` | false | Include `{x, y, width, height}` for each element |
| `--compact` | false | Omit empty structural nodes |
| `--surface` | window | Target surface: `window`, `focused`, `menu`, `menubar`, `sheet`, `popover`, `alert` |

**Output structure:**
```json
{
  "version": "1.0",
  "ok": true,
  "command": "snapshot",
  "data": {
    "app": "System Settings",
    "window": { "id": "w-4521", "title": "General" },
    "ref_count": 14,
    "tree": {
      "role": "window",
      "name": "General",
      "children": [
        {
          "ref": "@e1",
          "role": "button",
          "name": "About",
          "states": ["focused"]
        },
        {
          "role": "group",
          "name": "Appearance",
          "children": [
            {
              "ref": "@e2",
              "role": "checkbox",
              "name": "Dark Mode",
              "value": "0",
              "states": ["enabled"]
            }
          ]
        }
      ]
    }
  }
}
```

**Tips:**
- Always use `-i` to keep output compact for LLM context windows
- Use `--surface menu` to capture open context menus or dropdown menus
- Use `--surface sheet` for modal dialogs
- Use `--compact` with `-i` for maximum token efficiency
- Combine `--max-depth 5` to limit deep trees (e.g., Xcode)

## find

Search elements by role, name, value, or text content.

```bash
agent-desktop find --app "Finder" --role button --name "OK"
agent-desktop find --app "TextEdit" --role textfield
agent-desktop find --app "Safari" --text "Sign In" --first
agent-desktop find --app "App" --role checkbox --count
agent-desktop find --app "App" --role button --nth 2
```

| Flag | Description |
|------|-------------|
| `--app` | Application name |
| `--role` | Accessibility role: button, textfield, checkbox, link, menuitem, tab, slider, combobox, treeitem, cell |
| `--name` | Accessible name or label |
| `--value` | Current value |
| `--text` | Fuzzy match across name, value, title, and description |
| `--first` | Return first match only |
| `--last` | Return last match only |
| `--nth N` | Return Nth match (0-indexed) |
| `--count` | Return match count only |

**Output (matches):**
```json
{
  "data": {
    "matches": [
      { "ref": "@e5", "role": "button", "name": "OK", "states": ["enabled"] }
    ],
    "count": 1
  }
}
```

## get

Read a specific property from an element.

```bash
agent-desktop get @e1 --property text
agent-desktop get @e2 --property value
agent-desktop get @e3 --property bounds
agent-desktop get @e4 --property role
agent-desktop get @e5 --property states
agent-desktop get @e1 --property title
```

| Property | Returns |
|----------|---------|
| `text` | Accessible name/label (default) |
| `value` | Current value (text content, slider position, etc.) |
| `title` | Window or element title |
| `bounds` | `{ x, y, width, height }` rectangle |
| `role` | Element role string |
| `states` | Array of active states |

## is

Check a boolean state on an element.

```bash
agent-desktop is @e1 --property visible
agent-desktop is @e2 --property enabled
agent-desktop is @e3 --property checked
agent-desktop is @e4 --property focused
agent-desktop is @e5 --property expanded
```

| Property | Checks |
|----------|--------|
| `visible` | Element is on screen (default) |
| `enabled` | Element is interactable |
| `checked` | Checkbox/switch is checked |
| `focused` | Element has keyboard focus |
| `expanded` | Disclosure/tree item is expanded |

**Output:**
```json
{ "data": { "ref": "@e3", "property": "checked", "result": true } }
```

## screenshot

Capture a PNG screenshot of an application window.

```bash
agent-desktop screenshot --app "Finder"
agent-desktop screenshot --app "Finder" output.png
agent-desktop screenshot --window-id "w-1234" capture.png
```

| Flag | Description |
|------|-------------|
| `--app` | Application name |
| `--window-id` | Specific window ID |
| (positional) | File path to save PNG (omit for base64 in JSON) |

When no output path is given, the screenshot is returned as a base64-encoded string in the JSON `data` field.

## list-surfaces

List available accessibility surfaces for an application.

```bash
agent-desktop list-surfaces --app "Finder"
```

Returns the available surfaces (window, menu, menubar, sheet, popover, alert) for snapshotting. Use this to discover what surfaces are currently available before targeting a specific one with `snapshot --surface`.
