# Interaction Commands

Commands for modifying UI state — clicking, typing, selecting, scrolling, and input synthesis.

Ref-based actions are headless by default. They try semantic accessibility operations and do not silently steal focus, move the cursor, synthesize keyboard input, or use the pasteboard. Physical/headed interaction is reserved for explicit `focus`, `press`, `hover`, `drag`, and `mouse-*` commands or an explicit FFI policy. The `type` command has an explicit focus-fallback tier for callers that opt into focus changes while still forbidding cursor movement; the default CLI path remains AX-value-first and headless.

All ref-based interaction commands accept `--snapshot <snapshot_id>`. Omit it for the active session's latest saved snapshot, or pass the `snapshot_id` returned by `snapshot` to keep scripts pinned to the exact ref map they observed. Explicit snapshot IDs do not require also passing `--session`.

Success responses for ref actions include a `steps` array when the activation chain recorded attempts: each entry is `{ "label": "AXPress", "outcome": "attempted" | "skipped" | "succeeded" }` in execution order, showing which activation path produced the result.

When the actionability preflight blocks an action, the error envelope carries the full report in `error.details`: `{ "actionable": false, "checks": [ { "name": "...", "status": "...", "reason": "..." } ] }`. Check names are `visible`, `stable`, `enabled`, `supported_action`, `policy`, and `editable`; statuses are `pass`, `fail`, and `unknown`. Use the failing check's `reason` to pick recovery: `wait --element <ref> --predicate actionable`, a fresh snapshot, or an explicit focus/physical command when intended.

## Click Actions

Click commands use semantic AX activation first. In the default headless policy, coordinate click fallback is blocked; use `mouse-click` only when physical cursor movement is intended.

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
Tries AXOpen. Physical double-click fallback is blocked by default policy; use `mouse-click --xy X,Y --count 2` when a headed physical double-click is intended.

### triple-click
```bash
agent-desktop triple-click @e2
```
Physical triple-click requires cursor/focus side effects and is blocked by default policy; use `mouse-click --xy X,Y --count 3` when a headed physical triple-click is intended.

### right-click
```bash
agent-desktop right-click @e5
```
Performs a semantic right-click/context-menu action and includes the menu tree when a menu surface can be verified. If the right-click action succeeds but menu probing fails, the command still returns the action result with `menu_probe.ok: false` so callers do not retry and double-open context menus. Combo boxes and menu buttons expose menu-opening actions for their primary dropdown; use `select` for those controls, not `right-click`. Focus-stealing and coordinate right-click fallback are blocked by default policy.

## Text Input

### type
```bash
agent-desktop type @e2 "hello@example.com"
agent-desktop type @e2 "multi line\ntext"
```
In the default headless policy, inserts text by mutating the element's AX value when the target has a settable text value. If a target cannot be updated headlessly, the command returns a structured error instead of stealing focus. Physical keyboard synthesis and pasteboard-based insertion are reserved for explicit policy paths.

When an explicit focus/physical policy is used for non-ASCII text on macOS, the adapter may briefly place the text on the clipboard to paste it. Do not use that path for secrets; prefer the default headless value path or `set-value` when the target supports it.

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

### key-down
```bash
agent-desktop key-down shift
```
Holds a key or modifier down. Must be paired with `key-up`.

### key-up
```bash
agent-desktop key-up shift
```
Releases a held key or modifier.

## Mouse

### hover
```bash
agent-desktop hover @e5
agent-desktop hover --xy 500,300
agent-desktop hover @e5 --duration 2000
```
Moves cursor to element center or absolute coordinates. Optional `--duration` holds position for N ms.
This is an explicit cursor-moving command.

### drag
```bash
agent-desktop drag --from @e1 --to @e5
agent-desktop drag --from-xy 100,200 --to-xy 400,500
agent-desktop drag --from @e1 --to-xy 400,500 --duration 500
agent-desktop drag --from @e1 --to @e5 --drop-delay 800
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

macOS drop targets often need the dragged item to dwell over them before they register as the drop destination — too short and the gesture lands as a drag with no drop. The default 500ms dwell suits most targets; raise `--drop-delay` (e.g. 800–1200) for sluggish destinations like list reorders or cross-window drops. The dwell posts continuous drag events over the destination so it stays highlighted, rather than a dead pause.

### mouse-move
```bash
agent-desktop mouse-move --xy 500,300
```
Moves cursor to absolute screen coordinates.

### mouse-click
```bash
agent-desktop mouse-click --xy 500,300
agent-desktop mouse-click --xy 500,300 --button right
agent-desktop mouse-click --xy 500,300 --count 2
```

| Flag | Default | Description |
|------|---------|-------------|
| `--xy` | (required) | Coordinates as `x,y` |
| `--button` | left | `left`, `right`, `middle` |
| `--count` | 1 | Number of clicks |

### mouse-down / mouse-up
```bash
agent-desktop mouse-down --xy 100,200
agent-desktop mouse-up --xy 300,400
```
Low-level press/release for custom drag or hold interactions.

| Flag | Default | Description |
|------|---------|-------------|
| `--xy` | (required) | Coordinates as `x,y` |
| `--button` | left | `left`, `right`, `middle` |

## Choosing the Right Command

| Goal | Preferred | Alternative |
|------|-----------|-------------|
| Click a button | `click @ref` | `mouse-click --xy` if AX fails |
| Fill a text field | `clear @ref` then `type @ref "text"` | `set-value @ref "text"` for direct replacement |
| Clear then type | `clear @ref` then `type @ref "new"` | `mouse-click --xy X,Y --count 3` only when physical selection is intended |
| Toggle a checkbox | `check @ref` / `uncheck @ref` | `toggle @ref` if you don't know current state |
| Open context menu | `right-click @ref` | `mouse-click --xy --button right` when physical interaction is intended |
| Select dropdown option | `select @ref "Option"` | `snapshot --surface menu` after an explicitly opened menu |
| Navigate a form | `press tab` between fields | `focus @ref` to jump directly |
| Copy text | `press cmd+c --app "App"` | `clipboard-set` to set directly |
| Scroll to find elements | `scroll @ref --direction down` | `scroll-to @ref` if you have the ref |
