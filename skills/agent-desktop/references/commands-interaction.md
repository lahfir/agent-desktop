# Interaction Commands

Commands for modifying UI state — clicking, typing, selecting, scrolling, and input synthesis.

### Headless (default) vs `--headed`

Ref-based actions run in two modes, Playwright-style:

- **Headless (default).** Semantic accessibility operations only. The action never silently steals focus, moves the cursor, synthesizes keyboard input, or uses the pasteboard. When the semantic path cannot perform the action it fails closed.
- **`--headed`.** A global flag (`agent-desktop --headed click @s8f3k2p9:e5`) that authorizes the action's core-owned preconditions. Ref actions that need keyboard delivery focus the exact source window; pointer actions focus that window and require a verified target point before the adapter runs. On macOS, `click`, `right-click`, `type`, `clear`, and `scroll` are physical-first; `double-click`, `triple-click`, `hover`, and `drag` are physical-only. `expand`, `collapse`, `set-value`, `select`, `toggle`, `check`, `uncheck`, `focus`, and `scroll-to` stay semantic.

`press` is explicit physical keyboard input. `hover`, `drag`, `mouse-move`, `mouse-click`, and `mouse-wheel` are explicit physical cursor input and require `--headed`. Raw coordinates carry no window identity, so they never focus an app. The held-input names (`key-down`, `key-up`, `mouse-down`, `mouse-up`) are reserved and return `ACTION_NOT_SUPPORTED` until a stateful daemon can own the hold lifetime.

`--headed` is a global flag and also applies to every `batch` entry.

### Reading the result of an action

A successful action reports what happened, not just that it ran:

| Field | Meaning |
|-------|---------|
| `data.steps` | Each mechanism attempted, in order, with `outcome` (`succeeded`/`skipped`) and `verified` |
| `data.disposition.delivery` | `delivered_verified` when the effect was observed, `delivered_unverified` when the application claimed success without an observable change |
| `data.post_state` | The target element's state after the action, when the action has one |
| `data.surfaces` | Overlays the application had open once the action settled |

Check `data.surfaces` before assuming an action finished the job. An action that
opens a sheet, menu, or alert leaves the application waiting on that overlay, and
the next command must target it:

```json
{ "ok": true, "command": "set-value",
  "data": { "action": "set-value", "surfaces": [{ "id": "focused-window", "type": "sheet" }] } }
```

Reach into that overlay with `find --surface sheet` rather than searching the
window. A `delivered_unverified` result with a surface still open usually means
the application is waiting for a confirmation the action did not deliver.

### `--wait-for` / `--wait-for-gone` (global)

Three global flags poll the accessibility tree until a compact selector matches (or, with `--wait-for-gone`, until it no longer matches), then return a snapshot envelope:

```bash
agent-desktop snapshot --app Finder -w "button:OK"
agent-desktop click @s8f3k2p9:e5 -w ":Saved!"
agent-desktop click @s8f3k2p9:e5 --wait-for-gone "progressindicator" --wait-timeout 5000
```

| Flag | Short | Default | Meaning |
|------|-------|---------|---------|
| `--wait-for <SELECTOR>` | `-w` | — | Block until an element matching `<SELECTOR>` is present |
| `--wait-for-gone <SELECTOR>` | — | — | Block until no element matches (mutually exclusive with `--wait-for`) |
| `--wait-timeout <MS>` | — | `30000` | Poll budget; on expiry exit `1` with `kind: "wait_timeout"`, `predicate: "selector"` |

**Selector grammar:** one `role:text` string split on the first `:`. Examples: `"button:Submit"` (role + text), `"button"` (role only), `":Saved!"` (text only). Matching uses the same `find` matcher (`node_matches`); text searches name, value, and description.

**Supported commands:** `snapshot` plus all 18 ref-resolving actions (`click`, `type`, `set-value`, `scroll`, `hover`, `drag`, …) — 19 commands total. Other commands (`find`, `launch`, …) return `INVALID_ARGS`. Workaround: `snapshot --app Foo -w "button:Login"`.

**Post-action waits** poll the **acted-on ref's own window** (`entry.source_window_id`, scoped to `entry.source_app`), not the frontmost window — critical in headless and multi-window apps where the terminal or a sibling window has focus. The action result is preserved under `after_action` in the returned envelope.

**Success shape:** a match returns the full snapshot envelope (`app`, `window`, `ref_count`, `snapshot_id`, `tree`) plus `elapsed_ms` and `matched_selector`. The one exception is `--wait-for-gone` when the target **app or window has itself closed**: there is no tree left to capture, so the success payload is the compact `{ "matched_selector", "gone": true, "target_absent": true, "elapsed_ms" }`. On timeout the `wait_timeout` error `details` carry `last_error` (when a poll errored) and the `snapshot_id` of the last tree built.

**Snapshot constraints:** `--root` and `--wait-for`/`--wait-for-gone` are mutually exclusive (`INVALID_ARGS`). Batch items never inherit an outer `-w` (use per-item flows or run `snapshot -w` separately).

**Timeout envelope:** exit `1`, `error.code` `TIMEOUT`, `error.details.kind` `"wait_timeout"`, `error.details.snapshot_id` holds the last built tree for inspection. Post-action timeouts also embed `error.details.after_action`.

#### Which gestures have a headless path

The command surface is platform-agnostic: every ref action builds an `Action` and calls the platform adapter, which owns the headless-vs-physical implementation. The table below is the **macOS (Phase 1) adapter's** behavior — a gesture is headless-capable there only when macOS exposes an accessibility action for it. If a future Windows (UIA) or Linux (AT-SPI) adapter exposes a headless path for `double-click`/`triple-click`, that command lights up headlessly on that platform with **no change to the command or core** — only the adapter changes (`hover`/`drag` are modeled as raw cursor gestures, so they stay physical everywhere by design).

| Command | Headless path (macOS) | Notes |
|---------|---------------|-------|
| `click`, `set-value`, `check`, `select`, `scroll`, `expand`, … | yes | semantic AX actions in strict headless mode |
| `type` | yes | uses `AXSelectedText` headlessly; `--headed` synthesizes keyboard input |
| `double-click` | no | a real two-click gesture; requires `--headed` |
| `triple-click` | no | macOS exposes no triple-click action; it is purely 3 physical clicks → `--headed` only |
| `hover` | no | hovering *is* moving the cursor over an element; no AX equivalent |
| `drag` / drop | no | dragging *is* a cursor press-move-release; no general AX drag. Native cross-app drop needs the OS dragging-session/pasteboard protocol that synthetic events cannot start (works for same-view source-tracked gestures and web/Electron mouse-DnD) |
| menu bar (`--surface menubar`) | enumerate/open | the app menu bar is readable and openable; SwiftUI `CommandMenu` items accept AXPress but do not route to their action closure (a SwiftUI limitation, like its Slider) — native AppKit menu items fire. `.contextMenu` item selection works. |

All ref-based interaction commands accept `--snapshot <snapshot_id>`. Snapshot and find output already return qualified refs (`@<snapshot_id>:eN`), which embed the exact snapshot and need no separate flag. Legacy bare `@eN` input requires `--snapshot`; when a session owns that snapshot, the command also needs the same `--session` or `AGENT_DESKTOP_SESSION` scope. Lookup never searches another session namespace.

Success responses for ref actions include a `steps` array when the activation chain recorded attempts: each entry is `{ "label": "AXPress", "outcome": "attempted" | "skipped" | "succeeded" }` in execution order, showing which activation path produced the result.

When the actionability preflight blocks an action, the error envelope carries the full report in `error.details`: `{ "actionable": false, "checks": [ { "check": "...", "status": "...", "reason": "..." } ] }`. The `check` identifiers are `visible`, `stable`, `enabled`, `supported_action`, `policy`, `editable`, and `receives_events`; statuses are `pass`, `fail`, and `unknown`. Failures split by whether waiting can help: the **transient** checks (`visible`, `stable`, `enabled`, `receives_events`) can change over time — scroll into view, settle, become enabled, occlusion clears — so they surface as `ACTION_FAILED` and are retried within `--timeout-ms`; the **terminal** checks (`supported_action`, `policy`, `editable`) cannot be healed by waiting (the element's role/action set or the interaction policy would have to change), so they fail fast with a precise code — `ACTION_NOT_SUPPORTED` (`supported_action`/`editable`) or `POLICY_DENIED` (`policy`) — instead of polling to `TIMEOUT`. The dispatch actions that activate an element (`click`, `double-click`, `right-click`, `triple-click`, `type`, `set-value`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `clear`, `focus`, `scroll`, `scroll-to`) run the applicable `visible`/`stable`/`enabled`/`supported_action`/`policy`/`editable` battery; the four click variants additionally run `receives_events`. `hover` and each ref endpoint of `drag` use the pointer resolver instead: live visibility and bounds, one scroll-into-view attempt when needed, a second equal-bounds sample, then `receives_events`. They do not require enabled/editable or an element action capability because the gesture itself is raw pointer input. Use the failing check's `reason` to pick recovery: `wait --element <ref> --predicate actionable`, a fresh snapshot, or `--headed` when a `policy` check failed and a physical gesture is intended.

**`receives_events` failures.** When a hit test at the target's center point lands on a different element, `receives_events` fails with `reason: "occluded by <role>"` and a structured `occluder` object on that check: `{ "role", "name", "bounds" }` (the element that actually received the hit, when it can be identified). The target's own bounds have not changed — something else is now on top of them. Recovery is to bring the target's window or element to the front (or dismiss whatever is covering it), then retry; blind-retrying without changing z-order will fail the same way again.

Every ref-resolving action accepts `--timeout-ms` (default `5000`), but it budgets different things. For the dispatch actions (`click`, `double-click`, `triple-click`, `right-click`, `clear`, `focus`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `scroll-to`, `type`, `set-value`, `select`, `scroll`) it is the actionability-wait budget: they poll roughly every 100ms until the target becomes actionable, then fail with `TIMEOUT` once the budget is exhausted — unless the block is a terminal check (`supported_action`/`policy`/`editable`), which fails fast on the first attempt with `ACTION_NOT_SUPPORTED`/`POLICY_DENIED` rather than waiting out the budget. For `hover` and `drag`, the same budget covers ref resolution, live visibility/bounds, stability, and `receives_events`. Transient misses, app-unresponsive reads, and occlusion are polled until recovery or `TIMEOUT`; terminal errors are returned immediately with their original code. If every poll completed traversal but one or more required native live reads failed, the command instead returns `ACTION_FAILED` with `details.kind: "live_read_incomplete"` and `retryable: false`; re-snapshot and use a fresh ref rather than treating incomplete evidence as an ordinary timeout.

**Implicit scroll-into-view.** Standard ref actions whose `Action` declares a scroll precondition attempt `AXScrollToVisible` before dispatch. The pointer resolver for `hover` and `drag` independently makes one scroll attempt when a ref endpoint is not visibly bounded, then re-resolves and fails closed if it is still not visible. Use the standalone `scroll-to` command when you need an explicit, verifiable scroll result.

## Click Actions

Click commands use semantic AX activation in strict headless mode. Pass `--headed` to prefer a physical click, or use `agent-desktop --headed mouse-click` for a raw coordinate click.

### click
```bash
agent-desktop click @s8f3k2p9:e5
agent-desktop click @e5 --snapshot <snapshot_id>
```
Primary activation. Headless tries the activation the element actually publishes — `AXPress`, `AXOpen`, or `AXConfirm` — and, for a row whose activation is selection, writes the container's selection instead. `--headed` performs a physical click first and reports `physical_synthetic` in `data.steps`. Delivery is judged by observing the application, not by the accessibility return code, so `data.steps` reports each attempt and `disposition.delivery` distinguishes `delivered_verified` from `delivered_unverified`.

### double-click
```bash
agent-desktop double-click @s8f3k2p9:e3
```
Double-click is a physical gesture and fails closed in headless mode. Pass `--headed` to perform it, or use `agent-desktop --headed mouse-click --xy X,Y --count 2` for raw coordinates.

### triple-click
```bash
agent-desktop triple-click @s8f3k2p9:e2
```
Triple-click requires cursor/focus side effects and is blocked in headless mode; pass `--headed` (`agent-desktop --headed triple-click @s8f3k2p9:e2`), or use `agent-desktop --headed mouse-click --xy X,Y --count 3` for a raw coordinate triple-click.

### right-click
```bash
agent-desktop right-click @s8f3k2p9:e5
```
Headless uses semantic context-menu actions. `--headed` performs a physical right-click first. On macOS, a semantic `AXShowMenu` can return `APP_UNRESPONSIVE` with uncertain delivery after opening a modal menu; inspect the effect and never retry blindly. Use `select` for combo boxes and menu buttons.

## Text Input

### type
```bash
agent-desktop type @s8f3k2p9:e2 "hello@example.com"
agent-desktop type @s8f3k2p9:e2 "multi line\ntext"
```
Headless `type` uses `AXSelectedText` without focusing the app or synthesizing keys. Pass `--headed` to focus the target and synthesize keyboard input. Use `set-value` when direct semantic value assignment is the intended interaction.

### set-value
```bash
agent-desktop set-value @s8f3k2p9:e2 "new value"
```
Sets the value directly via the AX value attribute. Faster than `type` but may not trigger all UI callbacks. Use for text fields, text areas, and sliders.

### clear
```bash
agent-desktop clear @s8f3k2p9:e2
```
Headless clears through `AXValue`. With `--headed`, it performs focus + Select All + Delete first.

### focus
```bash
agent-desktop focus @s8f3k2p9:e2
```
Sets keyboard focus on the element without clicking it.
This is an explicit focus-changing command. It uses accessibility focus and does not move the cursor.

## Selection & Toggle

### select
```bash
agent-desktop select @s8f3k2p9:e4 "Option B"
```
Selects an option in a list, dropdown, or combobox by display text. For menu-backed controls it opens the AX menu, presses the matching menu item, and verifies `AXValue` when the control exposes it. It returns a structured error when the matching item is missing or the exposed value does not change.

### toggle
```bash
agent-desktop toggle @s8f3k2p9:e6
```
Toggles a checkbox or switch to the opposite state.

### check
```bash
agent-desktop check @s8f3k2p9:e6
```
Sets a checkbox or switch to the checked/on state. Idempotent — does nothing if already checked.

### uncheck
```bash
agent-desktop uncheck @s8f3k2p9:e6
```
Sets a checkbox or switch to the unchecked/off state. Idempotent.

## Expand & Collapse

### expand
```bash
agent-desktop expand @s8f3k2p9:e7
```
Expands a disclosure triangle, tree item, or accordion.

### collapse
```bash
agent-desktop collapse @s8f3k2p9:e7
```
Collapses an expanded disclosure/tree item.

## Scrolling

### scroll
```bash
agent-desktop scroll @s8f3k2p9:e1 --direction down --amount 3
agent-desktop scroll @s8f3k2p9:e1 --direction up --amount 5
agent-desktop scroll @s8f3k2p9:e1 --direction left --amount 2
agent-desktop scroll @s8f3k2p9:e1 --direction right --amount 2
```

| Flag | Default | Description |
|------|---------|-------------|
| `--direction` | down | `up`, `down`, `left`, `right` |
| `--amount` | 3 | Number of scroll units |
| `--timeout-ms` | 5000 | Actionability wait budget in ms before failing with `TIMEOUT` |

Headless mode uses AX scroll actions, scroll bars, and state-setting paths. Headed mode focuses the exact ref window, resolves the target point, and sends a physical wheel gesture first. If the selected mode has no safe mechanism, the command returns a structured error.

### scroll-to
```bash
agent-desktop scroll-to @s8f3k2p9:e8
```
Scrolls the element into the visible area of its scroll container.

## Keyboard

### press
```bash
agent-desktop press return
agent-desktop press escape
agent-desktop press cmd+c
agent-desktop press cmd+shift+z
agent-desktop press shift+tab
agent-desktop press f5
agent-desktop press cmd+a --app "TextEdit"
```

| Flag | Description |
|------|-------------|
| `--app` | Target application; key delivery is PID-targeted, and `--headed` additionally focuses its exact window first |

**Key names:** `return`, `escape`, `tab`, `space`, `delete`, `up`, `down`, `left`, `right`, `f1`-`f12`
**Modifiers:** `cmd`, `ctrl`, `alt`, `shift` — combine with `+`

Dangerous shortcuts (e.g. `cmd+q`, `ctrl+cmd+q`, `cmd+alt+esc`, `cmd+shift+delete`) are refused with `POLICY_DENIED`. Normalization covers modifier order and key-name aliases (`escape`/`esc`, `backspace`/`delete`). The block is the **platform adapter's** decision, not core's — the calling agent stays in control: pass `--force` to send a flagged `press` combo anyway (`agent-desktop press cmd+q --force`). The reserved held-key names reject even when `--force` is present.

### key-down / key-up

These names are reserved but fail closed with `ACTION_NOT_SUPPORTED` in the stateless CLI. Use the atomic `press` command. A future stateful daemon may own held-key lifetimes safely.

## Mouse

### hover
```bash
agent-desktop --headed hover @s8f3k2p9:e5
agent-desktop --headed hover --xy 500,300
```
Moves cursor to element center or absolute coordinates. A positive `--duration` is rejected because a stateless process cannot guarantee cursor ownership during a dwell; run hover without it, then use `wait <ms>` for an explicit pause.
This is an explicit cursor-moving command.

With `--headed`, a ref-addressed hover must focus the target's exact window before moving the cursor and fails before delivery if focus cannot be confirmed. The response then includes `"focused": true`. Raw `--xy` hover never attempts focus because no target window identity exists.

### drag
```bash
agent-desktop --headed drag --from @s8f3k2p9:e1 --to @s8f3k2p9:e5
agent-desktop --headed drag --from-xy 100,200 --to-xy 400,500
agent-desktop --headed drag --from @s8f3k2p9:e1 --to-xy 400,500 --duration 500
agent-desktop --headed drag --from @s8f3k2p9:e1 --to @s8f3k2p9:e5 --drop-delay 800
```

| Flag | Description |
|------|-------------|
| `--from` | Source element ref |
| `--from-xy` | Source coordinates as `x,y` |
| `--to` | Destination element ref |
| `--to-xy` | Destination coordinates as `x,y` |
| `--duration` | Drag duration in milliseconds (movement from source to destination) |
| `--drop-delay` | Milliseconds to hold over the destination before releasing; default 500 |
| `--timeout-ms` | Actionability wait budget in ms before failing with `TIMEOUT`; default 5000 |

Can mix ref and coordinate sources (e.g., `--from @s8f3k2p9:e1 --to-xy 400,500`).

With `--headed`, a ref-addressed `--from` must focus the source's exact window before mouse-down and fails before delivery if focus cannot be confirmed. The destination app is never pre-focused because raising it could cover the source point. Coordinate-only drags never attempt focus. For cross-app two-ref drags, keep the destination window visible; both endpoints still undergo live visibility, stability, and hit-test checks.

macOS drop targets often need the dragged item to dwell over them before they register as the drop destination — too short and the gesture lands as a drag with no drop. The default 500ms dwell suits most targets; raise `--drop-delay` (e.g. 800–1200) for sluggish destinations like list reorders or cross-window drops. The dwell posts continuous drag events over the destination so it stays highlighted, rather than a dead pause.

### mouse-move
```bash
agent-desktop --headed mouse-move --xy 500,300
```
Moves cursor to absolute screen coordinates.

### mouse-click
```bash
agent-desktop --headed mouse-click --xy 500,300
agent-desktop --headed mouse-click --xy 500,300 --button right
agent-desktop --headed mouse-click --xy 500,300 --count 2
```

| Flag | Default | Description |
|------|---------|-------------|
| `--xy` | (required) | Coordinates as `x,y` |
| `--button` | left | `left`, `right`, `middle` |
| `--count` | 1 | Number of clicks |
| `--modifiers` | | Held modifiers: `shift`, `meta`, `ctrl`, `alt` (repeatable; `cmd`/`command` aliases are accepted); held during the click |

### mouse-down / mouse-up

These names are reserved but fail closed with `ACTION_NOT_SUPPORTED` in the stateless CLI. Use the atomic `mouse-click` or `drag` command. A future stateful daemon may own held-button lifetimes safely.

### mouse-wheel
```bash
agent-desktop --headed mouse-wheel --x 500 --y 300
agent-desktop --headed mouse-wheel --x 500 --y 300 --dy -3
agent-desktop --headed mouse-wheel --x 500 --y 300 --dx -2 --dy 0
agent-desktop --headed mouse-wheel --x 500 --y 300 --modifiers shift
```
Synthesizes a scroll-wheel event at absolute coordinates and requires `--headed`. This is distinct from `scroll <ref>`: `scroll` targets an element through AX scroll semantics, while `mouse-wheel` posts a raw wheel event at a screen point (for custom scroll surfaces or canvases with no AX scroll action). Held modifiers are applied to the event, so `--modifiers shift` produces the horizontal-scroll chord some apps expect.

| Flag | Default | Description |
|------|---------|-------------|
| `--x` | (required) | Absolute X coordinate |
| `--y` | (required) | Absolute Y coordinate |
| `--dy` | -3 | Vertical wheel lines; positive is up, negative is down |
| `--dx` | 0 | Horizontal wheel lines; positive is left, negative is right |
| `--modifiers` | | Held modifiers: `shift`, `meta`, `ctrl`, `alt` (repeatable; `cmd`/`command` aliases are accepted) |

## Choosing the Right Command

### Agent cursor presentation

Enable it once per session with `session start --cursor` or `cursor-overlay enable`. Interaction commands take no cursor flags.

- The cursor stays alive between eligible headless ref actions and travels from its previous destination.
- The action waits for it to land before it dispatches, capped at 900 ms.
- A click plays a ripple and flashes an accent outline around the element.
- Headed actions hide the overlay and use the real OS cursor. Raw pointer commands keep their headed-only behavior.

See `commands-system.md` for the style flags.

| Goal | Preferred | Alternative |
|------|-----------|-------------|
| Click a button | `click @ref` | `agent-desktop --headed mouse-click --xy X,Y` if physical interaction is intended |
| Fill a text field | `clear @ref` then `type @ref "text"` | `set-value @ref "text"` for direct replacement |
| Clear then type | `clear @ref` then `type @ref "new"` | `agent-desktop --headed mouse-click --xy X,Y --count 3` only when physical selection is intended |
| Toggle a checkbox | `check @ref` / `uncheck @ref` | `toggle @ref` if you don't know current state |
| Open context menu | `right-click @ref` | `agent-desktop --headed mouse-click --xy X,Y --button right` when physical interaction is intended |
| Select dropdown option | `select @ref "Option"` | `snapshot --surface menu` after an explicitly opened menu |
| Navigate a form | `press tab` between fields | `focus @ref` to jump directly |
| Copy text | `press cmd+c --app "App"` | `clipboard-set` to set directly |
| Scroll to find elements | `scroll @ref --direction down` | `scroll-to @ref` if you have the ref |
