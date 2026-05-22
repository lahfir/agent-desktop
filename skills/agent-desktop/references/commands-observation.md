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
agent-desktop snapshot --app "App" --skeleton -i
agent-desktop snapshot --root @e12 --snapshot <snapshot_id> -i
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
| `--skeleton` | false | Clamp traversal to depth 3 and add `children_count` to truncated containers |
| `--root <REF>` | | Drill down from a ref discovered in a previous snapshot. Cannot be combined with `--surface` |
| `--snapshot <snapshot_id>` | latest | Snapshot ID to use when resolving `--root` |

**Output structure:**
```json
{
  "version": "2.0",
  "ok": true,
  "command": "snapshot",
  "data": {
    "app": "System Settings",
    "window": { "id": "w-4521", "title": "General" },
    "ref_count": 14,
    "snapshot_id": "s8f3k2p9",
    "tree": {
      "role": "window",
      "name": "General",
      "children": [
        {
          "ref_id": "@e1",
          "role": "button",
          "name": "About",
          "states": ["focused"]
        },
        {
          "role": "group",
          "name": "Appearance",
          "children": [
            {
              "ref_id": "@e2",
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

**Skeleton mode (`--skeleton`):**
- Produces a shallow overview by clamping depth to `min(max_depth, 3)`
- Truncated containers include a `children_count` field showing how many children were omitted
- Named or described containers at the truncation boundary receive refs with empty `available_actions`, serving as drill-down targets for `--root`

**Root mode (`--root <REF>`):**
- Starts tree traversal from the given ref instead of the window root
- Merges new refs into the existing refmap with scoped invalidation: only refs from the previous drill of the same root are replaced, leaving all other refs intact
- Cannot be combined with `--surface`
- Use `--snapshot <snapshot_id>` when drilling from a specific snapshot rather than the latest snapshot pointer

**Progressive drill-down workflow:**
```bash
# Step 1: Get skeleton overview
agent-desktop snapshot --skeleton --app Slack -i

# Step 2: Drill into a discovered region
agent-desktop snapshot --root @e3 --snapshot <snapshot_id> -i

# Step 3: Re-drill same region (scoped invalidation replaces @e3's refs)
agent-desktop snapshot --root @e3 --snapshot <snapshot_id> -i
```

**Tips:**
- Always use `-i` to keep output compact for LLM context windows
- Use `--surface menu` to capture open context menus or dropdown menus
- Use `--surface sheet` for modal dialogs
- Use `--compact` with `-i` for maximum token efficiency
- Combine `--max-depth 5` to limit deep trees (e.g., Xcode)
- Use `--skeleton` first to get a high-level map, then `--root` to drill into specific regions
- Combine `--skeleton` with `-i` and `--compact` for the most token-efficient initial overview
- Keep `snapshot_id` when commands must resolve against a specific snapshot instead of the latest snapshot pointer

## find

Search elements by role, name, value, or text content.

```bash
agent-desktop find --app "Finder" --role button --name "OK"
agent-desktop find --app "TextEdit" --role textfield
agent-desktop find --app "Safari" --text "Sign In" --first
agent-desktop find --app "App" --role checkbox --count
agent-desktop find --app "App" --role button --nth 2
agent-desktop find --app "App" --role button --limit 20
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
| `--limit N` | Return at most N matches; defaults to 50 for match lists, use 0 for all |

**Output (matches):**
```json
{
  "data": {
    "matches": [
      { "ref_id": "@e5", "role": "button", "name": "OK", "states": ["enabled"] }
    ],
    "count": 1
  }
}
```

## get

Read a specific property from an element.

```bash
agent-desktop get @e1 --property text
agent-desktop get @e1 --snapshot <snapshot_id> --property text
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
agent-desktop is @e1 --snapshot <snapshot_id> --property visible
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

Screenshots require Screen Recording permission. Permission denial is reported as `PERM_DENIED`, not `INTERNAL`.

## list-surfaces

List available accessibility surfaces for an application.

```bash
agent-desktop list-surfaces --app "Finder"
```

Returns the available surfaces (window, menu, menubar, sheet, popover, alert) for snapshotting. Use this to discover what surfaces are currently available before targeting a specific one with `snapshot --surface`.
