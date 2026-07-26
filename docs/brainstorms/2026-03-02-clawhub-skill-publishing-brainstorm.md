# ClawHub Skill Publishing & Scalable Skill Architecture Brainstorm

**Date:** 2026-03-02
**Status:** Decided
**Trigger:** agent-desktop has a mature skill ready for public distribution. Need a scalable architecture for multi-platform, multi-app skill publishing to ClawHub.

## Problem Statement

The `agent-desktop` skill exists locally in `skills/agent-desktop/` with a SKILL.md and 5 reference files. A separate `agent-desktop-macos` skill lives in `.claude/skills/` (not git-tracked). As we add Windows/Linux platforms and app-specific knowledge (Slack, Notion, VS Code quirks), we need:

1. A **scalable directory structure** under `skills/` that supports platform and app-specific sub-skills
2. **Automated CI publishing** to ClawHub on every release (no manual steps)
3. A **scaffolding pattern** so adding a new platform or app skill is trivial

## Data

| Metric | Value |
|--------|-------|
| Current skill files | 1 SKILL.md + 5 reference docs + 1 platform SKILL.md |
| Total content | ~42KB across all files |
| Commands documented | 54 |
| ClawHub skills total | 5,700+ |
| Desktop automation skills on ClawHub | Very few (niche) |
| ClawHub CLI batch command | `clawhub sync --root skills/ --all` |

## What We're Building

A **nested skill hierarchy** where platform skills contain app-specific references, with CI-driven auto-publishing to ClawHub on every release.

### Directory Structure

```
skills/
├── agent-desktop/                  # Core skill (platform-agnostic)
│   ├── SKILL.md                    # Commands, observe-act loop, ref system, JSON contract
│   └── references/
│       ├── commands-observation.md
│       ├── commands-interaction.md
│       ├── commands-system.md
│       └── workflows.md            # 12 common automation patterns
│
├── agent-desktop-macos/            # macOS platform skill
│   ├── SKILL.md                    # TCC permissions, AX API, smart activation chain, surfaces
│   └── references/
│       ├── slack.md                # Slack on macOS: Electron quirks, clipboard paste pattern
│       ├── notion.md               # Notion on macOS: block navigation, slash commands
│       ├── vscode.md               # VS Code on macOS: command palette, editor interaction
│       └── notifications.md        # NC lifecycle, dismiss strategies, stacked notifications
│
├── agent-desktop-windows/          # Windows platform skill (Phase 2)
│   ├── SKILL.md                    # UIA setup, permission model, COM automation
│   └── references/
│       ├── slack.md                # Slack on Windows
│       ├── teams.md                # Teams on Windows (UIA-native)
│       └── vscode.md               # VS Code on Windows
│
└── agent-desktop-linux/            # Linux platform skill (Phase 3)
    ├── SKILL.md                    # AT-SPI setup, D-Bus, Wayland vs X11
    └── references/
        ├── slack.md                # Slack on Linux
        └── vscode.md               # VS Code on Linux
```

### Key design principles

1. **App quirks are platform-scoped** — Slack on macOS (AX + Electron depth-skip) differs from Slack on Windows (UIA + Chromium detection). App references live inside the platform skill, not in a separate top-level skill.

2. **Core skill stays platform-agnostic** — `agent-desktop/` documents commands, JSON contract, ref system. Zero platform detail. The `references/macos.md` that currently lives here moves to `agent-desktop-macos/SKILL.md`.

3. **Each `skills/*/` folder is independently publishable** — ClawHub `clawhub sync --root skills/ --all` discovers and publishes each subfolder as a separate skill.

4. **Git-tracked source of truth** — everything under `skills/` is committed. The `.claude/skills/` and `.agents/skills/` symlinks are derived (gitignored).

## Why This Approach

1. **Matches how the project actually scales** — each platform adapter (Phase 2, 3) brings platform-specific AX behavior AND app-specific quirks. The skill hierarchy mirrors the crate hierarchy.

2. **App knowledge compounds** — every time we test Slack/Notion/VS Code, we document quirks in the platform's `references/` folder. Future agents benefit automatically.

3. **`clawhub sync` handles multi-skill publishing** — one CI step publishes all skills in `skills/`. No per-skill config. Adding a new skill = adding a new folder.

4. **Fewer top-level skills** — users install `agent-desktop` (core) + `agent-desktop-macos` (their platform). App knowledge comes free inside the platform skill. No need to install `agent-desktop-slack` separately.

## Approaches Considered

### 1. Flat app-specific skills (agent-desktop-slack, agent-desktop-notion, etc.)
- One top-level skill per app, published separately
- **Rejected:** App behavior is platform-dependent. Slack on macOS vs Windows differs fundamentally. Flat app skills would either duplicate platform info or be incomplete.

### 2. Nested hierarchy — platform > apps (CHOSEN)
- Platform skills contain app-specific references
- App knowledge scoped to platform where it's actually useful
- Fewer skills to install, more content per skill

### 3. Monolithic single skill with everything
- One massive `agent-desktop` skill with all platform + app info
- **Rejected:** Too much context loaded at once. Users on macOS don't need Windows/Linux info in context.

## CI Automation

Add a `publish-skills` job to `.github/workflows/release.yml`:

```yaml
publish-skills:
  needs: [release-please]
  if: needs.release-please.outputs.release_created == 'true'
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: actions/setup-node@v4
      with: { node-version: '22' }
    - run: npm i -g clawhub
    - run: |
        clawhub sync \
          --root skills/ \
          --all \
          --bump patch \
          --changelog "${{ needs.release-please.outputs.tag_name }}"
      env:
        CLAWHUB_TOKEN: ${{ secrets.CLAWHUB_TOKEN }}
```

This runs automatically when release-please creates a new release. All skills under `skills/` are published in one step.

### Symlink Automation

Add a post-checkout script or Makefile target to wire `.claude/skills/`:

```bash
# scripts/link-skills.sh
for skill_dir in skills/*/; do
  name=$(basename "$skill_dir")
  mkdir -p .claude/skills
  ln -sfn "../../$skill_dir" ".claude/skills/$name"
done
```

## Key Decisions

- **Nested hierarchy (platform > apps)** — app references scoped to platform
- **CI auto-publish via `clawhub sync`** on every release — zero manual steps
- **Move `references/macos.md` from core skill to `agent-desktop-macos/`** — core stays platform-agnostic
- **Move `agent-desktop-macos` from `.claude/skills/` to `skills/`** — git-tracked, publishable
- **Version synced to npm/Cargo version** via release-please
- **`CLAWHUB_TOKEN` as GitHub secret** for CI authentication
- **Symlink script** to wire `skills/` → `.claude/skills/` for local development

## Scaffolding: Adding a New App Reference

To add Slack macOS knowledge:

1. Create `skills/agent-desktop-macos/references/slack.md`
2. Document: Electron quirks, clipboard paste pattern, block structure, known AX issues
3. Reference it from `skills/agent-desktop-macos/SKILL.md` reference table
4. Commit → next release auto-publishes to ClawHub

To add a new platform (e.g., Windows):

1. Create `skills/agent-desktop-windows/SKILL.md` with platform-specific frontmatter
2. Create `skills/agent-desktop-windows/references/` for app-specific docs
3. Run `scripts/link-skills.sh` to wire local symlinks
4. Commit → next release auto-publishes

## Migration Steps (from current state)

1. Move `.claude/skills/agent-desktop-macos/SKILL.md` → `skills/agent-desktop-macos/SKILL.md`
2. Move `skills/agent-desktop/references/macos.md` → merge into `skills/agent-desktop-macos/SKILL.md`
3. Remove `macos.md` from core skill's reference table in SKILL.md
4. Add ClawHub metadata (`version`, `tags`, `requirements`) to all SKILL.md frontmatters
5. Create `scripts/link-skills.sh` for local symlink management
6. Add `publish-skills` job to `release.yml`
7. Store `CLAWHUB_TOKEN` in GitHub repo secrets
8. First publish: manual `clawhub sync --root skills/ --all` to verify
9. After verification: CI handles all future publishes

## Open Questions

None — architecture is clear.

## Sources

- [ClawHub Docs](https://docs.openclaw.ai/tools/clawhub) — publish CLI, sync command, token management
- [ClawHub GitHub](https://github.com/openclaw/clawhub) — skill directory structure
- [Anthropic Skills Repo](https://github.com/anthropics/skills) — SKILL.md format standard
- [ClawHub Publisher Skill](https://playbooks.com/skills/openclaw/skills/openclaw-clawhub-publisher) — batch publish, CI integration
