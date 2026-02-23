# agent-desktop Quick Reference

Use this command when you need to automate desktop applications with agent-desktop.

## Core Loop

```bash
# 1. OBSERVE
agent-desktop snapshot --app "$APP" -i

# 2. Find target element by parsing JSON output (look for ref like @e5)

# 3. ACT
agent-desktop click @e5    # or type, select, toggle, etc.

# 4. VERIFY
agent-desktop snapshot --app "$APP" -i
```

## Common Tasks

**Click a button:**
```bash
agent-desktop snapshot --app "App" -i
agent-desktop click @e5
```

**Fill a text field:**
```bash
agent-desktop clear @e2
agent-desktop type @e2 "new text"
```

**Toggle a checkbox:**
```bash
agent-desktop check @e3      # idempotent on
agent-desktop uncheck @e3    # idempotent off
```

**Open context menu:**
```bash
agent-desktop right-click @e5
agent-desktop wait --menu --app "App"
agent-desktop snapshot --app "App" --surface menu -i
agent-desktop click @e7
```

**Navigate menus:**
```bash
agent-desktop snapshot --app "App" --surface menubar -i
agent-desktop click @e1        # File menu
agent-desktop wait --menu --app "App"
agent-desktop snapshot --app "App" --surface menu -i
agent-desktop click @e5        # Menu item
```

**Wait for UI:**
```bash
agent-desktop wait --text "Done" --app "App" --timeout 5000
agent-desktop wait --window "Save" --timeout 5000
```

## First-Time Setup

```bash
agent-desktop permissions --request
# Grant in System Settings > Privacy & Security > Accessibility
```

## Detailed Skills

For comprehensive reference, see:
- `.claude/skills/agent-desktop/SKILL.md` — Core concepts and full command index
- `.claude/skills/agent-desktop/commands-observation.md` — snapshot, find, get, is, screenshot
- `.claude/skills/agent-desktop/commands-interaction.md` — click, type, select, toggle, scroll, drag
- `.claude/skills/agent-desktop/commands-system.md` — launch, close, windows, clipboard, wait, batch
- `.claude/skills/agent-desktop/workflows.md` — Common automation patterns
- `.claude/skills/agent-desktop-macos/SKILL.md` — macOS permissions, troubleshooting, AX details
