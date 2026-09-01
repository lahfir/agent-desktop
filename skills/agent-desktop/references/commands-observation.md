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
| `--surface` | window | Target surface: `window`, `focused`, `menu`, `menubar`, `sheet`, `popover`, `alert`. Windows additionally serves the shell kinds `taskbar`, `system-tray`, `system-tray-overflow`, `start-menu`, `action-center` |
| `--skeleton` | false | Clamp traversal to depth 3 and add `children_count` to truncated containers |
| `--root <REF>` | | Drill down from a ref discovered in a previous snapshot. Cannot be combined with `--surface` |
| `--snapshot <snapshot_id>` | embedded in qualified root | Required only when `--root` is a legacy bare ref |
| `--timeout-ms <MS>` | 3000 | Observation deadline. A cold Chromium/Electron settle can take 10-25s; raise this when a fresh snapshot returns a shell-thin tree |
| `--force-electron-a11y` | false | Assume Chromium renderer accessibility is already forced, so the adapter skips activation guidance and returns the observed tree |

**Output structure:**
```json
{
  "version": "2.3",
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
- Each truncated branch exposes its deepest safely resolvable drill target using stable text, native ID, or bounds evidence; an anonymous boundary falls back to its nearest resolvable ancestor

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
- Use `--surface menu` to capture open context menus or dropdown menus (macOS and Windows)
- Use `--surface sheet` for modal dialogs (both platforms)

**Surfaces are platform-specific, and the honest list is in `status`.** Run
`agent-desktop status` and read `supported_surfaces` before requesting one:
macOS serves `window`, `focused`, `menu`, `menubar`, `sheet`, `popover` and
`alert`, while Windows serves `window`, `focused`, `sheet`, `menu`, and the
shell kinds `taskbar`, `system-tray`, `system-tray-overflow`, `start-menu`
and `action-center`. A surface the adapter does not serve returns
`PLATFORM_NOT_SUPPORTED` with the supported list in `details`, so the failure
is honest — but it is cheaper to read `status` first than to discover it from
an error.

The shell kinds are OS chrome no application owns: `snapshot --surface
<kind>` resolves an already-present shell surface with no `--app`, and a
closed one returns `WINDOW_NOT_FOUND` with a suggestion naming
`open-system-surface` as the way to raise it — see that command below.
- Use `--compact` with `-i` for maximum token efficiency
- Combine `--max-depth 5` to limit deep trees (e.g., Xcode)
- Use exact `find` first when you know the target role or name; otherwise use `--skeleton` for a high-level map, then `--root` to drill into specific regions
- Combine `--skeleton` with `-i` and `--compact` for the most token-efficient initial overview
- For a Chromium-based app's web contents (Slack, VS Code, Discord, and similar), `launch --cdp` plus a CDP client is a faster alternative to skeleton traversal on a fresh launch — see `references/commands-system.md`
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
| `--window-id ID` | Search one window from `list-windows` instead of every window the app owns |
| `--timeout-ms MS` | Traversal deadline, default 5000. A large tree — a shell file dialog, whose folder tree and item list both populate — can exceed it; raise this or narrow the search rather than reading the timeout as an unresponsive app |
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
| `text` | The element's current text content — today the same read as `value` |
| `value` | Current value (text content, slider position, etc.) |
| `title` | Accessible name or label — this is the one that answers "what does this button say" |
| `bounds` | `{ x, y, width, height }` rectangle |
| `role` | Element role string |
| `states` | Array of active states |

`text` is the default, and on a control with a label but no value — a button,
a menu item — it comes back empty. `title` carries the accessible name there.
The two are not yet distinct reads: `text` and `value` resolve identically,
and whether `text` should prefer the name is an open contract question rather
than settled behaviour, so this table describes what ships rather than what
the name suggests.

For `--property bounds`, the response carries a sibling `live` boolean. `true`
means the rectangle came from a live read taken just now; `false` means the
live read succeeded but found no current bounds (collapsed, not laid out,
virtualized) or the platform could not perform one, so the snapshot-time
rectangle was returned instead. A caller piping `bounds` straight into
`mouse-click --x --y` should check `live` first — a `false` rectangle may no
longer be where the element is.

## is

Check a boolean state on an element.

```bash
agent-desktop is @s8f3k2p9:e1 --property visible
agent-desktop is @e1 --snapshot <snapshot_id> --property visible
agent-desktop is @s8f3k2p9:e2 --property enabled
agent-desktop is @s8f3k2p9:e3 --property checked
agent-desktop is @s8f3k2p9:e4 --property focused
agent-desktop is @s8f3k2p9:e5 --property expanded
agent-desktop is @s8f3k2p9:e6 --property selected
```

| Property | Checks |
|----------|--------|
| `visible` | Element is on screen (default) |
| `enabled` | Element is interactable |
| `checked` | Checkbox/switch is checked |
| `focused` | Element has keyboard focus |
| `expanded` | Disclosure/tree item is expanded |
| `selected` | Selectable element is selected |

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

List the surfaces an application presents right now.

```bash
agent-desktop list-surfaces --app "Finder"
```

Returns the surfaces available for `snapshot --surface`. On macOS these are
the app-owned overlay surfaces (window, menu, menubar, sheet, popover,
alert). On Windows the inventory is per-process: every top-level window as
`window`, the foreground one also `focused`, a modal window as `sheet`, and
an open menu as one `menu` surface carrying `item_count`. Shell surfaces
belong to the OS rather than to any process and never appear here — use
`snapshot --surface <kind>` for those.

## open-system-surface

Raise a shell surface and get the identity of the window it actually
presents — the same `w-<id>` shape `list-windows` emits, so the round trip
into `snapshot` needs no second lookup.

```bash
agent-desktop --headed open-system-surface --surface action-center
agent-desktop --headed open-system-surface --surface start-menu
```

| Flag | Description |
|------|-------------|
| `--surface` | Shell surface to open. Windows: `start-menu`, `taskbar`, `system-tray`, `system-tray-overflow`, `action-center` |

The command takes the foreground, so it enforces the same floor as every
chrome-raising command: a strict-headless call is refused with
`POLICY_DENIED` before anything is raised — pass global `--headed`. An
already-present surface is returned without being raised again.

A kind the running OS does not expose returns `PLATFORM_NOT_SUPPORTED` with
a `platform_detail` naming the build and the surface that carries the
capability instead — `quick-settings` on pre-Windows-11 builds points at
`action-center`, whose pane holds the quick actions there. `start-menu`
resolves to whatever the OS accelerator actually raises, which on
pre-Windows-11 builds is a search-hosted overlay. The macOS kinds
(`spotlight`, `dock`, `menu-bar-extras`) are not implemented yet and return
`PLATFORM_NOT_SUPPORTED`.
