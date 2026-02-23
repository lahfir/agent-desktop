# Interaction Commands

Commands for modifying UI state — clicking, typing, selecting, scrolling, and input synthesis.

## Click Actions

All click commands use a smart activation chain (AX-first) that tries accessibility actions before falling back to coordinate-based clicks.

### click
```bash
agent-desktop click @e5
```
Primary activation. Tries AXPress > AXConfirm > AXOpen > AXPick > child activation > focus+activate > coordinate click.

### double-click
```bash
agent-desktop double-click @e3
```
Tries AXOpen first, then two smart activations with 50ms gap, then CGEvent double-click.

### triple-click
```bash
agent-desktop triple-click @e2
```
Three smart activations with 30ms gaps, then CGEvent triple-click. Useful for select-all in text fields.

### right-click
```bash
agent-desktop right-click @e5
```
Opens context menu. Tries AXShowMenu > focus+AXShowMenu > parent/child AXShowMenu > coordinate right-click. Use `wait --menu` after to capture the menu, then `snapshot --surface menu` to read it.

## Text Input

### type
```bash
agent-desktop type @e2 "hello@example.com"
agent-desktop type @e2 "multi line\ntext"
```
Focuses the element then types each character via keyboard synthesis. Handles special characters.

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

## Selection & Toggle

### select
```bash
agent-desktop select @e4 "Option B"
```
Selects an option in a list, dropdown, or combobox by its display text.

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

### drag
```bash
agent-desktop drag --from @e1 --to @e5
agent-desktop drag --from-xy 100,200 --to-xy 400,500
agent-desktop drag --from @e1 --to-xy 400,500 --duration 500
```

| Flag | Description |
|------|-------------|
| `--from` | Source element ref |
| `--from-xy` | Source coordinates as `x,y` |
| `--to` | Destination element ref |
| `--to-xy` | Destination coordinates as `x,y` |
| `--duration` | Drag duration in milliseconds |

Can mix ref and coordinate sources (e.g., `--from @e1 --to-xy 400,500`).

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
| Fill a text field | `type @ref "text"` | `set-value @ref "text"` for direct set |
| Clear then type | `clear @ref` then `type @ref "new"` | `triple-click @ref` then `type @ref "new"` |
| Toggle a checkbox | `check @ref` / `uncheck @ref` | `toggle @ref` if you don't know current state |
| Open context menu | `right-click @ref` then `wait --menu` | `mouse-click --xy --button right` |
| Select dropdown option | `select @ref "Option"` | `click @ref` then `find` the option |
| Navigate a form | `press tab` between fields | `focus @ref` to jump directly |
| Copy text | `press cmd+c --app "App"` | `clipboard-set` to set directly |
| Scroll to find elements | `scroll @ref --direction down` | `scroll-to @ref` if you have the ref |
