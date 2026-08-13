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
| `--timeout-ms <MS>` | 3000 | Observation deadline. A cold Chromium/Electron settle can take 10-25s; raise this when a fresh snapshot returns a shell-thin tree |
| `--force-electron-a11y` | false | Assume Chromium renderer accessibility is already forced, so the adapter skips activation guidance and returns the observed tree |

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
    "complete": true,
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

**Partial snapshots (`data.complete`):**
- `complete` is present on every snapshot. `true` means the whole tree was observed
- A snapshot that exhausts its observation budget still succeeds: `ok: true` with `"complete": false`, the tree it did observe, `"truncated": true`, and `"nodes_observed"` — it is not a `TIMEOUT` error, so read `complete` rather than branching on an error code to detect an oversized tree
- Every node whose descendants were cut short carries `"subtree_truncated": true`, emitted only when true, so you can walk from the root to each boundary and drill in with `--root`
- Raise `--timeout-ms` or lower `--max-depth` to turn a partial tree into a complete one
- A `--root` drill-down replaces refs inside an existing snapshot, so it is all-or-nothing: an incomplete observation returns `TIMEOUT` instead of a partial tree

**Skeleton mode (`--skeleton`):**
- Produces a shallow overview by clamping depth to `min(max_depth, 3)`
- Truncated containers include a `children_count` field showing how many children were omitted
- Named or described containers at the truncation boundary receive refs with empty `available_actions`, serving as drill-down targets for `--root`

**Optional descriptor fields** (emitted by Windows; absent on macOS and Linux; all four are optional and omitted unless a provider produces them):
- `subrole` — finer role refinement from UIA `AriaRole` (web content)
- `role_description` — provider's localized control-type description
- `placeholder` — `HelpText` where it is not already the description
- `dom_classes` — DOM class list; no Windows producer yet, so always absent on Windows in the current phase

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
```

| Flag | Description |
|------|-------------|
| `--app` | Application name |
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

Capture a PNG screenshot of an application window or display.

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

When no output path is given, the screenshot is returned as a base64-encoded string in the JSON `data` field. A positional PATH writes through the user-path atomic writer (not the private-file seam), so network shares and foreign-owned directories stay writable; omitting the path keeps bytes in the JSON envelope.

**macOS:** screenshots require Screen Recording permission. Permission denial is reported as `PERM_DENIED`, not `INTERNAL`.

**Windows:** runtime precedence is Modern (`Windows.Graphics.Capture`) then Legacy (`PrintWindow` / `BitBlt`). Gate on the runtime `IsSupported` predicate and successful interop activation, not on OS build number (A22-1). When modern is unavailable or fails to activate — including hosts where `IsSupported` is true but interop cannot activate — the command attempts Legacy silently; a 200ms floor is reserved for the Legacy attempt out of the overall deadline, but budget exhaustion or a Legacy failure can still surface as an error rather than guaranteeing `ok: true` (`LEGACY_DEADLINE_FLOOR` in `capture_backend.rs`). Windows has no screen-recording consent gate; `permissions` reports `screen_recording` as `not_required` when capture works. Bare `screenshot PATH` (no `--app` / `--screen`) maps to the primary display, matching `--screen 0`.

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
