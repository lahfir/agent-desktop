# Interaction Commands

Commands for modifying UI state — clicking, typing, selecting, scrolling, and input synthesis.

### Headless (default) vs `--headed`

Ref-based actions run in two modes, Playwright-style:

- **Headless (default).** Semantic accessibility operations only. The action never silently steals focus, moves the cursor, synthesizes keyboard input, or uses the pasteboard. When the AX path cannot perform the action it fails closed rather than reaching for OS input synthesis. (`type` is the one exception: its base tier may focus the target field — required for reliable typing — but still never moves the cursor.)
- **`--headed`.** A global flag (`agent-desktop --headed click @e5`) that upgrades every ref action to permit focus stealing **and** cursor movement, unlocking the physical click/double-click/scroll/keypress fallbacks in the action chain. The AX path is still tried first, so `--headed` never regresses elements that work headlessly — it only adds fallbacks for elements that need a real gesture (e.g. a gesture-only button with no `AXOpen`).

Raw-input commands (`press`, `hover`, `drag`, `mouse-*`, `key-down`, `key-up`) are physical by nature. Cursor-moving commands (`hover`, `drag`, `mouse-*`) require `--headed`; keyboard commands are explicit low-level input.

`--headed` is a global flag and also applies to every `batch` entry.

### `--wait-for` / `--wait-for-gone` (global)

Three global flags poll the accessibility tree until a compact selector matches (or, with `--wait-for-gone`, until it no longer matches), then return a snapshot envelope:

```bash
agent-desktop snapshot --app Finder -w "button:OK"
agent-desktop click @e5 -w ":Saved!"
agent-desktop click @e5 --wait-for-gone "progressindicator" --wait-timeout 5000
```

| Flag | Short | Default | Meaning |
|------|-------|---------|---------|
| `--wait-for <SELECTOR>` | `-w` | — | Block until an element matching `<SELECTOR>` is present |
| `--wait-for-gone <SELECTOR>` | — | — | Block until no element matches (mutually exclusive with `--wait-for`) |
| `--wait-timeout <MS>` | — | `30000` | Poll budget; on expiry exit `1` with `kind: "wait_timeout"`, `predicate: "selector"` |

**Selector grammar:** one `role:text` string split on the first `:`. Examples: `"button:Submit"` (role + text), `"button"` (role only), `":Saved!"` (text only). Matching uses the same `find` matcher (`node_matches`); text searches name, value, and description.

**Supported commands:** `snapshot` plus the 16 ref-resolving actions (`click`, `type`, `set-value`, `scroll`, …) — 17 commands total. Other commands (`find`, `launch`, …) return `INVALID_ARGS`. Workaround: `snapshot --app Foo -w "button:Login"`.

**Post-action waits** poll the **acted-on ref's own window** (`entry.source_window_id`, scoped to `entry.source_app`), not the frontmost window — critical in headless and multi-window apps where the terminal or a sibling window has focus. The action result is preserved under `after_action` in the returned envelope.

**Success shape:** a match returns the full snapshot envelope (`app`, `window`, `ref_count`, `snapshot_id`, `tree`) plus `elapsed_ms` and `matched_selector`. The one exception is `--wait-for-gone` when the target **app or window has itself closed**: there is no tree left to capture, so the success payload is the compact `{ "matched_selector", "gone": true, "target_absent": true, "elapsed_ms" }`. On timeout the `wait_timeout` error `details` carry `last_error` (when a poll errored) and the `snapshot_id` of the last tree built.

**Snapshot constraints:** `--root` and `--wait-for`/`--wait-for-gone` are mutually exclusive (`INVALID_ARGS`). Batch items never inherit an outer `-w` (use per-item flows or run `snapshot -w` separately).

**Timeout envelope:** exit `1`, `error.code` `TIMEOUT`, `error.details.kind` `"wait_timeout"`, `error.details.snapshot_id` holds the last built tree for inspection. Post-action timeouts also embed `error.details.after_action`.

#### Which gestures have a headless path

The command surface is platform-agnostic: every ref action builds an `Action` and calls the platform adapter, which owns the headless-vs-physical implementation. The table below is the **macOS (Phase 1) adapter's** behavior — a gesture is headless-capable there only when macOS exposes an accessibility action for it. If a future Windows (UIA) or Linux (AT-SPI) adapter exposes a headless path for `double-click`/`triple-click`, that command lights up headlessly on that platform with **no change to the command or core** — only the adapter changes (`hover`/`drag` are modeled as raw cursor gestures, so they stay physical everywhere by design).

| Command | Headless path (macOS) | Notes |
|---------|---------------|-------|
| `click`, `set-value`, `check`, `select`, `scroll`, `expand`, … | yes | semantic AX actions; the default and most reliable surface |
| `type` | focus fallback | CLI `type` may focus the target field but never moves the cursor; use `set-value` for pure headless value mutation when supported |
| `double-click` | partial | `AXOpen` works headless on items that advertise it (Finder/list/outline rows, table cells). Falls back to `--headed` only for gesture-only targets with no `AXOpen`. |
| `triple-click` | no | macOS exposes no triple-click action; it is purely 3 physical clicks → `--headed` only |
| `hover` | no | hovering *is* moving the cursor over an element; no AX equivalent |
| `drag` / drop | no | dragging *is* a cursor press-move-release; no general AX drag. Native cross-app drop needs the OS dragging-session/pasteboard protocol that synthetic events cannot start (works for same-view source-tracked gestures and web/Electron mouse-DnD) |
| menu bar (`--surface menubar`) | enumerate/open | the app menu bar is readable and openable; SwiftUI `CommandMenu` items accept AXPress but do not route to their action closure (a SwiftUI limitation, like its Slider) — native AppKit menu items fire. `.contextMenu` item selection works. |

All ref-based interaction commands accept `--snapshot <snapshot_id>`. Omit it for the active session's latest saved snapshot, or pass the `snapshot_id` returned by `snapshot` to keep scripts pinned to the exact ref map they observed. Explicit snapshot IDs do not require also passing `--session`. After `session start`, implicit latest resolves inside the new session; snapshots taken before the boundary need explicit `--snapshot <old-id>`.

Success responses for ref actions include a `steps` array when the activation chain recorded attempts: each entry is `{ "label": "AXPress", "outcome": "attempted" | "skipped" | "succeeded" }` in execution order, showing which activation path produced the result.

When the actionability preflight blocks an action, the error envelope carries the full report in `error.details`: `{ "actionable": false, "checks": [ { "name": "...", "status": "...", "reason": "..." } ] }`. Check names are `visible`, `stable`, `enabled`, `supported_action`, `policy`, and `editable`; statuses are `pass`, `fail`, and `unknown`. Use the failing check's `reason` to pick recovery: `wait --element <ref> --predicate actionable`, a fresh snapshot, or `--headed` when a `policy` check failed and a physical gesture is intended.

## Click Actions

Click commands use semantic AX activation first. In the default headless mode, coordinate click fallback is blocked; pass `--headed` to allow the physical click fallback, or use `agent-desktop --headed mouse-click` for a raw coordinate click.

### click
```bash
agent-desktop click @e5
agent-desktop click @e5 --snapshot <snapshot_id>
```
Primary activation. Tries verified AXPress > AXConfirm > AXOpen > AXPick > child activation > selection/value relays > custom actions > ancestor activation. Focus-stealing and coordinate fallback steps are not used by the default ref command path.

### double-click
```bash
agent-desktop double-click @e3
```
Tries AXOpen (headless). When the element advertises no `AXOpen`, the headless command fails closed with `POLICY_DENIED`; pass `--headed` to perform a real double-click (`agent-desktop --headed double-click @e3`), or use `agent-desktop --headed mouse-click --xy X,Y --count 2` for a raw coordinate double-click.

### triple-click
```bash
agent-desktop triple-click @e2
```
Triple-click requires cursor/focus side effects and is blocked in headless mode; pass `--headed` (`agent-desktop --headed triple-click @e2`), or use `agent-desktop --headed mouse-click --xy X,Y --count 3` for a raw coordinate triple-click.

### right-click
```bash
agent-desktop right-click @e5
```
Performs a semantic right-click/context-menu action and includes `menu` plus `menu_snapshot_id` when a menu surface can be verified. If the right-click action succeeds but menu probing fails, the command still returns the action result with `menu_probe.ok: false` so callers do not retry and double-open context menus. Combo boxes and menu buttons expose menu-opening actions for their primary dropdown; use `select` for those controls, not `right-click`. Focus-stealing and coordinate right-click fallback are blocked in headless mode; pass `--headed` to allow them.

## Text Input

### type
```bash
agent-desktop type @e2 "hello@example.com"
agent-desktop type @e2 "multi line\ntext"
```
`type` uses the focus-fallback policy floor: it may focus the target field because typing requires focus, but it never moves the cursor. If the field cannot be updated and the focused-insert path is unavailable, it returns a structured error. Pass `--headed` to unlock physical keyboard synthesis and pasteboard-based insertion for fields that ignore AX value writes (common in web/Electron inputs).

Under focus-fallback or `--headed`, non-ASCII text on macOS may be briefly placed on the clipboard to paste it. Do not use that path for secrets; prefer `set-value` when the target supports it.

### set-value
```bash
agent-desktop set-value @e2 "new value"
```
Sets the value directly via the AX value attribute. Faster than `type` but may not trigger all UI callbacks. Use for text fields, text areas, and sliders.

### clear
```bash
agent-desktop clear @e2
```
Clears the element's value to an empty string. Equivalent to `set-value @e2 ""`.

### focus
```bash
agent-desktop focus @e2
```
Sets keyboard focus on the element without clicking it.
This is an explicit focus-changing command. It uses accessibility focus and does not move the cursor.

## Selection & Toggle

### select
```bash
agent-desktop select @e4 "Option B"
```
Selects an option in a list, dropdown, or combobox by display text. For menu-backed controls it opens the AX menu, presses the matching menu item, and verifies `AXValue` when the control exposes it. It returns a structured error when the matching item is missing or the exposed value does not change.

### toggle
```bash
agent-desktop toggle @e6
```
Toggles a checkbox or switch to the opposite state.

### check
```bash
agent-desktop check @e6
```
Sets a checkbox or switch to the checked/on state. Idempotent — does nothing if already checked.

### uncheck
```bash
agent-desktop uncheck @e6
```
Sets a checkbox or switch to the unchecked/off state. Idempotent.

## Expand & Collapse

### expand
```bash
agent-desktop expand @e7
```
Expands a disclosure triangle, tree item, or accordion.

### collapse
```bash
agent-desktop collapse @e7
```
Collapses an expanded disclosure/tree item.

## Scrolling

### scroll
```bash
agent-desktop scroll @e1 --direction down --amount 3
agent-desktop scroll @e1 --direction up --amount 5
agent-desktop scroll @e1 --direction left --amount 2
agent-desktop scroll @e1 --direction right --amount 2
```

| Flag | Default | Description |
|------|---------|-------------|
| `--direction` | down | `up`, `down`, `left`, `right` |
| `--amount` | 3 | Number of scroll units |

Uses AX scroll actions, scroll bars, and state-setting paths. If those are unavailable, the command returns a structured error instead of stealing focus or sending wheel events.

### scroll-to
```bash
agent-desktop scroll-to @e8
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
| `--app` | Target application (focuses app before pressing) |

**Key names:** `return`, `escape`, `tab`, `space`, `delete`, `up`, `down`, `left`, `right`, `f1`-`f12`
**Modifiers:** `cmd`, `ctrl`, `alt`, `shift` — combine with `+`

Dangerous shortcuts (e.g. `cmd+q`, `ctrl+cmd+q`, `cmd+alt+esc`, `cmd+shift+delete`) are refused with `POLICY_DENIED`. Normalization covers modifier order and key-name aliases (`escape`/`esc`, `backspace`/`delete`). The block is the **platform adapter's** decision, not core's — the calling agent stays in control: pass `--force` to send a flagged combo anyway (`agent-desktop press cmd+q --force`). `--force` is available on `press`, `key-down`, and `key-up`.

### key-down
```bash
agent-desktop key-down shift
```
Holds a key or modifier down. Must be paired with `key-up`. The blocked-combo guard (same set as `press`) is enforced per invocation. **Known limitation:** because the tool is stateless per call, an agent could hold modifiers across separate `key-down` calls to assemble a blocked combo; that cross-invocation case is not guarded — a stateful guard arrives with the Phase-4 daemon.

### key-up
```bash
agent-desktop key-up shift
```
Releases a held key or modifier. The blocked-combo guard (same set as `press`) applies per invocation.

## Mouse

### hover
```bash
agent-desktop --headed hover @e5
agent-desktop --headed hover --xy 500,300
agent-desktop --headed hover @e5 --duration 2000
```
Moves cursor to element center or absolute coordinates. Optional `--duration` holds position for N ms.
This is an explicit cursor-moving command.

With `--headed`, a ref-addressed hover ensures the target app is frontmost before moving the cursor (raising it if needed, best-effort), and the response includes `"focused": true` when that frontmost state was confirmed. The field is only ever present as `true`: absence means focus was never attempted (headless default, or `--xy` input — the caller owns the target there) or the best-effort raise could not be confirmed.

### drag
```bash
agent-desktop --headed drag --from @e1 --to @e5
agent-desktop --headed drag --from-xy 100,200 --to-xy 400,500
agent-desktop --headed drag --from @e1 --to-xy 400,500 --duration 500
agent-desktop --headed drag --from @e1 --to @e5 --drop-delay 800
```

| Flag | Description |
|------|-------------|
| `--from` | Source element ref |
| `--from-xy` | Source coordinates as `x,y` |
| `--to` | Destination element ref |
| `--to-xy` | Destination coordinates as `x,y` |
| `--duration` | Drag duration in milliseconds (movement from source to destination) |
| `--drop-delay` | Milliseconds to hold over the destination before releasing; default 500 |

Can mix ref and coordinate sources (e.g., `--from @e1 --to-xy 400,500`).

With `--headed`, a ref-addressed `--from` ensures the source app is frontmost before the mouse-down (the destination app is never pre-focused — raising it could cover the source point), and the response includes `"focused": true` when that frontmost state was confirmed. The field is only ever present as `true`: absence means focus was never attempted (headless default, or coordinate-only drags) or the best-effort raise could not be confirmed. For cross-app two-ref drags, ensure the destination window is visible (not fully occluded) before dragging — only the source app is raised.

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

### mouse-down / mouse-up
```bash
agent-desktop --headed mouse-down --xy 100,200
agent-desktop --headed mouse-up --xy 300,400
```
Low-level press/release for custom drag or hold interactions.

| Flag | Default | Description |
|------|---------|-------------|
| `--xy` | (required) | Coordinates as `x,y` |
| `--button` | left | `left`, `right`, `middle` |

## Choosing the Right Command

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
