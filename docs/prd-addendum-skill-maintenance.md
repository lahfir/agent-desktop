# PRD Addendum: Skill Maintenance Across Phases

**Date:** 2026-02-23
**Applies to:** agent-desktop PRD v2.0, all phases

## Overview

agent-desktop ships Claude Code skills (`.claude/skills/` and `.claude/commands/`) that teach AI coding agents how to use agent-desktop effectively. These skills must be kept in sync with the tool's capabilities across all phases.

## Skill Graph Structure

```
.claude/
├── commands/
│   └── desktop.md                          # Quick reference slash command
└── skills/
    ├── agent-desktop/                      # Core skill (platform-agnostic)
    │   ├── SKILL.md                        # Index: concepts, JSON contract, command overview
    │   ├── commands-observation.md         # snapshot, find, get, is, screenshot
    │   ├── commands-interaction.md         # click, type, select, toggle, scroll, drag
    │   ├── commands-system.md             # launch, close, windows, clipboard, wait, batch
    │   └── workflows.md                   # Common automation patterns
    └── agent-desktop-macos/               # macOS platform skill
        └── SKILL.md                       # Permissions, AX API, troubleshooting
```

## Phase-Specific Skill Updates

### Phase 1 (Foundation + macOS MVP) — CURRENT
- All skill files created covering 50 commands
- macOS platform skill with AX API details, permissions, troubleshooting
- Core workflows: form filling, menu navigation, context menus, scrolling, dialogs

### Phase 2 (Cross-Platform Expansion)
When Windows and Linux adapters ship:
- [ ] Create `.claude/skills/agent-desktop-windows/SKILL.md`
  - UIA permission model
  - Windows-specific behaviors (UAC, WinUI3 quirks)
  - Troubleshooting guide
- [ ] Create `.claude/skills/agent-desktop-linux/SKILL.md`
  - AT-SPI2/D-Bus setup
  - Wayland vs X11 differences
  - Troubleshooting guide
- [ ] Update core `SKILL.md` skill graph table to include new platform skills
- [ ] Update `workflows.md` with cross-platform patterns

### Phase 3 (MCP Server Mode)
When `--mcp` flag ships:
- [ ] Create `.claude/skills/agent-desktop-mcp/SKILL.md`
  - MCP tool surface documentation
  - Transport configuration (stdio, SSE)
  - Session management
  - Tool-to-CLI mapping reference
- [ ] Update core `SKILL.md` with MCP mode section
- [ ] Add MCP workflow patterns to `workflows.md`

### Phase 4 (Production Hardening)
When daemon mode and sessions ship:
- [ ] Update `commands-system.md` with session commands
- [ ] Add daemon lifecycle patterns to `workflows.md`
- [ ] Document enterprise quality gates in platform skills

## Maintenance Rules

1. **Every new command** must be added to the appropriate commands-*.md file
2. **Every new platform** gets its own skill directory under `.claude/skills/agent-desktop-{platform}/`
3. **Every new mode** (MCP, daemon) gets its own skill file
4. **Breaking changes** to JSON output or CLI flags must update all affected skill files
5. **Skill files are reviewed** as part of the PR checklist for any command-surface change
