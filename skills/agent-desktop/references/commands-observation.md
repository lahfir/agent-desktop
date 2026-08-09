# Observation Commands

Commands for reading UI state without modifying it.

## snapshot

Capture the accessibility tree as structured JSON with `@ref` IDs.

Output refs are qualified as `@<snapshot_id>:e<N>`. Use that value directly on
later commands. Legacy bare `@eN` input remains valid only with the matching
explicit `--snapshot <snapshot_id>` and inside the same session namespace.

```bash
agent-desktop snapshot --app "System Settings" -i
agent-desktop snapshot --app "Finder" --max-depth 5 --include-bounds
agent-desktop snapshot --app "App" --surface menu
agent-desktop snapshot --app "App" --window-id "w-1234"
agent-desktop snapshot --app "App" -i --compact
agent-desktop snapshot --app "App" --skeleton -i
agent-desktop snapshot --app "App" -w "button:Submit"
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
| `--snapshot <snapshot_id>` | embedded in qualified root | Required only when `--root` is a legacy bare ref |

**Output structure:**
```json
{
  "version": "2.2",
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
          "ref_id": "@s8f3k2p9:e1",
          "role": "button",
          "name": "About",
          "states": ["focused"]
        },
        {
          "role": "group",
          "name": "Appearance",
          "children": [
            {
              "ref_id": "@s8f3k2p9:e2",
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
agent-desktop find --app "App" --role button --name "OK" --exact
agent-desktop find --app "App" --description "Closes the dialog"
agent-desktop find --app "App" --native-id "submitButton"
agent-desktop find --app "App" --state enabled --state focused=false
agent-desktop find --root @s8f3k2p9:e4 --role textfield --value "README.md" --first
agent-desktop find --app "Finder" --surface menubar --name "Go to Folder…" --exact --first
```

Scope the search before widening the query. `--root` searches one ref's subtree
and `--surface` searches an overlay; both return a single ref instead of the
whole tree, which is the difference between a few hundred bytes and a full
menu-bar dump.

| Flag | Description |
|------|-------------|
| `--app` | Application name |
| `--root REF` | Search only inside this ref's subtree instead of the whole window. Pair with `--snapshot` for a legacy bare `@eN` ref |
| `--surface` | Search an overlay instead of the window (`menubar`, `menu`, `sheet`, `alert`, `popover`, ...). A menu bar belongs to the application, so several open windows are not ambiguous here. Cannot be combined with `--root`, which already carries its own surface |
| `--role` | Role to match against the live tree (button, textfield, checkbox, scrollarea, window, ...). Case-insensitive; `textarea`/`textbox`/`searchfield` fold to `textfield`. When a role filter matches nothing, the response carries `roles_present` — the roles actually in the searched tree — so you can tell "none on screen" from a wrong role name and retry |
| `--name` | Accessible name or label |
| `--value` | Current value |
| `--text` | Fuzzy match across name, value, title, and description |
| `--description` | Match by accessible description |
| `--native-id` | Match by native automation id (`AXIdentifier`) |
| `--exact` | Require exact (case-insensitive) matches for `--name`/`--description`/`--value` instead of fuzzy/substring matching |
| `--state TOKEN[=BOOL]` | Filter by state token; repeatable. Bare `TOKEN` requires the state present, `TOKEN=true`/`TOKEN=false` asserts its value (e.g. `--state enabled --state focused=false`) |
| `--first` | Return first match only |
| `--last` | Return last match only |
| `--nth N` | Return Nth match (0-indexed) |
| `--count` | Return match count only |
| `--limit N` | Return at most N matches; defaults to 50 for match lists, use 0 for all |

**Output (matches):**
```json
{
  "data": {
    "snapshot_id": "s8f3k2p9",
    "matches": [
      { "ref_id": "@s8f3k2p9:e5", "role": "button", "name": "OK", "states": ["enabled"] }
    ]
  }
}
```

Every non-count `find` response returns the `snapshot_id` that owns its refs. Pass that exact ID to later ref actions instead of relying on the mutable latest-snapshot pointer, especially when interleaving automation across apps or windows. Count-only responses create no ref namespace and omit `snapshot_id`.

**Output (no match — `roles_present` hint):** when a `--role` filter matches nothing, `roles_present` lists the roles actually in the searched tree so you can tell a wrong role name from "none on screen"; this applies to all non-count selection modes — an empty match list, or a `--first`/`--last`/`--nth` miss — whenever a role filter was active, making it a role-vocabulary hint for retries.
```json
{
  "data": {
    "matches": [],
    "count": 0,
    "roles_present": ["button", "cell", "checkbox", "scrollarea", "statictext"]
  }
}
```

## get

Read a specific property from an element.

```bash
agent-desktop get @s8f3k2p9:e1 --property text
agent-desktop get @e1 --snapshot <snapshot_id> --property text
agent-desktop get @s8f3k2p9:e2 --property value
agent-desktop get @s8f3k2p9:e3 --property bounds
agent-desktop get @s8f3k2p9:e4 --property role
agent-desktop get @s8f3k2p9:e5 --property states
agent-desktop get @s8f3k2p9:e1 --property title
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
agent-desktop is @s8f3k2p9:e1 --property visible
agent-desktop is @e1 --snapshot <snapshot_id> --property visible
agent-desktop is @s8f3k2p9:e2 --property enabled
agent-desktop is @s8f3k2p9:e3 --property checked
agent-desktop is @s8f3k2p9:e4 --property focused
agent-desktop is @s8f3k2p9:e5 --property expanded
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
{ "data": { "ref": "@s8f3k2p9:e3", "property": "checked", "result": true } }
```

## screenshot

Capture a PNG screenshot of an application window.

```bash
agent-desktop screenshot --app "Finder"
agent-desktop screenshot --app "Finder" output.png
agent-desktop screenshot --window-id "w-1234" capture.png
agent-desktop screenshot --screen 0 display.png
```

| Flag | Description |
|------|-------------|
| `--app` | Application name |
| `--window-id` | Specific window ID |
| `--screen` | Capture display by index instead of an app window (from `list-displays`; `0` = primary) |
| (positional) | File path to save PNG (omit for base64 in JSON) |

When no output path is given, the screenshot is returned as a base64-encoded string in the JSON `data` field.

Screenshots require Screen Recording permission. Permission denial is reported as `PERM_DENIED`, not `INTERNAL`.

## list-displays

List connected displays with bounds and scale factor.

```bash
agent-desktop list-displays
```

Returns an array of `{ id, bounds: { x, y, width, height }, is_primary, scale }`, sorted primary-first. Use the array index (not `id`) with `screenshot --screen <index>` — `0` is always the primary display after sorting.

**Output:**
```json
{
  "data": [
    { "id": "1", "bounds": { "x": 0, "y": 0, "width": 2560, "height": 1440 }, "is_primary": true, "scale": 2.0 },
    { "id": "2", "bounds": { "x": 2560, "y": 0, "width": 1920, "height": 1080 }, "is_primary": false, "scale": 1.0 }
  ]
}
```

## list-surfaces

List available accessibility surfaces for an application.

```bash
agent-desktop list-surfaces --app "Finder"
```

Returns the available surfaces (window, menu, menubar, sheet, popover, alert) for snapshotting. Use this to discover what surfaces are currently available before targeting a specific one with `snapshot --surface`.
