---
title: "feat: scalable skill architecture with ClawHub auto-publishing"
type: feat
status: completed
date: 2026-03-02
origin: docs/brainstorms/2026-03-02-clawhub-skill-publishing-brainstorm.md
---

# feat: scalable skill architecture with ClawHub auto-publishing

## Overview

Restructure the `skills/` directory into a nested hierarchy (core → platform → app references), sync stale skill content from `.agents/` to `skills/`, add ClawHub metadata to all SKILL.md files, create a symlink script for local development, and wire CI auto-publishing to ClawHub on every release.

(see brainstorm: docs/brainstorms/2026-03-02-clawhub-skill-publishing-brainstorm.md)

## Problem Statement

1. **Skills are stale** — `skills/agent-desktop/` has 50 commands documented, but `.agents/skills/agent-desktop/` has 54 (notifications added Feb 27). Git-tracked version lags.
2. **macOS skill not publishable** — lives in `.claude/skills/agent-desktop-macos/` (gitignored), not in `skills/`
3. **No publishing pipeline** — ClawHub integration doesn't exist. Manual publish only.
4. **No scaffolding** — adding a new platform or app-specific skill has no clear pattern or automation.

## Proposed Solution

### Directory Structure (Target)

```
skills/
├── agent-desktop/                  # Core skill (platform-agnostic)
│   ├── SKILL.md                    # Commands, observe-act loop, ref system, JSON contract
│   └── references/
│       ├── commands-observation.md
│       ├── commands-interaction.md
│       ├── commands-system.md
│       └── workflows.md
│
├── agent-desktop-macos/            # macOS platform skill
│   ├── SKILL.md                    # TCC, AX API, smart activation chain, surfaces, NC
│   └── references/
│       └── notifications.md        # NC lifecycle, dismiss strategies (extracted from SKILL.md)
│
scripts/
└── link-skills.sh                  # Symlinks skills/ → .claude/skills/ for local dev
```

### CI Pipeline Addition

New `publish-skills` job in `.github/workflows/release.yml` after `publish-npm`.

## Acceptance Criteria

- [x] `skills/agent-desktop/` synced to match `.agents/skills/agent-desktop/` (54 commands, notifications section)
- [x] `skills/agent-desktop-macos/` created from `.claude/skills/agent-desktop-macos/` (git-tracked)
- [x] `references/macos.md` removed from core skill (moved to platform skill)
- [x] Core skill SKILL.md reference table updated (no macos.md row)
- [x] ClawHub metadata added to all SKILL.md frontmatters (`version`, `tags`, `requirements`)
- [x] `scripts/link-skills.sh` created and working
- [x] `publish-skills` job added to `.github/workflows/release.yml`
- [x] npm postinstall prompts user to install Claude Code skills (with platform auto-detection)
- [x] All SKILL.md files reviewed using `/skill-creator` skill for best practices
- [x] All existing tests still pass
- [x] Clippy clean, fmt clean

## Build Guidance

**Use the `/skill-creator` skill** when writing or updating any SKILL.md file. It provides best practices for frontmatter structure, trigger keywords, reference file organization, and description quality. Invoke it before finalizing each skill to ensure the content meets Claude Code skill standards.

## Implementation

### Phase 1: Sync & Restructure Skills Directory

#### 1.1 Sync core skill from `.agents/` to `skills/`

The `.agents/skills/agent-desktop/` directory is the most up-to-date version (54 commands, includes notifications). Copy it over the stale `skills/agent-desktop/`.

**Files to sync:**

| Source (`.agents/skills/agent-desktop/`) | Destination (`skills/agent-desktop/`) |
|---|---|
| `SKILL.md` | `SKILL.md` (overwrite — adds notifications section) |
| `references/commands-observation.md` | `references/commands-observation.md` |
| `references/commands-interaction.md` | `references/commands-interaction.md` |
| `references/commands-system.md` | `references/commands-system.md` (adds notification commands) |
| `references/workflows.md` | `references/workflows.md` |

**After sync, delete:** `skills/agent-desktop/references/macos.md` — this content moves to the platform skill.

**Update:** `skills/agent-desktop/SKILL.md` reference table — remove the `macos.md` row.

#### 1.2 Move macOS skill to `skills/`

```bash
mkdir -p skills/agent-desktop-macos/references/
cp .claude/skills/agent-desktop-macos/SKILL.md skills/agent-desktop-macos/SKILL.md
```

If the macOS SKILL.md contains a Notification Center section that's large enough to be a reference, extract it to `skills/agent-desktop-macos/references/notifications.md` and reference it from the SKILL.md table. Otherwise keep it inline.

### Phase 2: Add ClawHub Metadata

#### 2.1 Core skill frontmatter

```yaml
# skills/agent-desktop/SKILL.md
---
name: agent-desktop
version: 0.1.8
tags: desktop-automation, accessibility, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  Desktop automation via native OS accessibility trees...
---
```

#### 2.2 macOS skill frontmatter

```yaml
# skills/agent-desktop-macos/SKILL.md
---
name: agent-desktop-macos
version: 0.1.8
tags: desktop-automation, macos, accessibility, ax-api, tcc-permissions
requirements:
  - agent-desktop
description: >
  macOS platform details for agent-desktop...
---
```

### Phase 3: Create Scaffolding Scripts

#### 3.1 `scripts/link-skills.sh`

```bash
#!/usr/bin/env bash
# Links skills/ directories to .claude/skills/ for local Claude Code use.
# Run after clone or when adding new skills.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CLAUDE_SKILLS="$REPO_ROOT/.claude/skills"

mkdir -p "$CLAUDE_SKILLS"

for skill_dir in "$REPO_ROOT"/skills/*/; do
  name=$(basename "$skill_dir")
  target="../../skills/$name"
  link="$CLAUDE_SKILLS/$name"

  if [ -L "$link" ]; then
    rm "$link"
  fi

  ln -s "$target" "$link"
  echo "Linked: .claude/skills/$name → skills/$name"
done
```

### Phase 4: Post-Install Skill Prompt

#### 4.1 Extend `npm/scripts/postinstall.js`

After the binary download succeeds, detect the platform and prompt the user to install the agent-desktop Claude Code skills. This runs in the terminal so we can use stdin.

```javascript
// npm/scripts/postinstall.js — append after binary install success

function promptSkillInstall() {
  const os = require('os');
  const { execSync } = require('child_process');

  // Check if Claude Code CLI is available
  try {
    execSync('claude --version', { stdio: 'ignore' });
  } catch {
    log('Tip: Install Claude Code skills for agent-desktop with:');
    log('  claude /plugin marketplace add lahfir/agent-desktop');
    return;
  }

  // Detect platform for the right skill
  const plat = os.platform();
  const platformSkill = {
    darwin: 'agent-desktop-macos',
    win32: 'agent-desktop-windows',
    linux: 'agent-desktop-linux',
  }[plat];

  log('');
  log('Claude Code skills available for agent-desktop!');
  log('Install with:');
  log('  claude /plugin marketplace add lahfir/agent-desktop');
  if (platformSkill) {
    log(`  claude /plugin install ${platformSkill}@lahfir-agent-desktop`);
  }
  log('');
}

promptSkillInstall();
```

**Design decision:** Print install instructions rather than auto-running `claude` commands. Postinstall scripts should not modify the user's Claude Code config without explicit consent. The user copies and runs the commands themselves.

#### 4.2 Platform detection mapping

| `os.platform()` | Core skill | Platform skill |
|---|---|---|
| `darwin` | `agent-desktop` | `agent-desktop-macos` |
| `win32` | `agent-desktop` | `agent-desktop-windows` |
| `linux` | `agent-desktop` | `agent-desktop-linux` |

### Phase 5: CI Auto-Publishing

#### 5.1 Add `publish-skills` job to `release.yml`

Add after the `publish-npm` job:

```yaml
# .github/workflows/release.yml
  publish-skills:
    needs: [release-please]
    if: needs.release-please.outputs.release_created == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: "22"

      - name: Install ClawHub CLI
        run: npm i -g clawhub

      - name: Publish all skills to ClawHub
        run: |
          clawhub sync \
            --root skills/ \
            --all \
            --bump patch \
            --changelog "Release ${{ needs.release-please.outputs.tag_name }}"
        env:
          CLAWHUB_TOKEN: ${{ secrets.CLAWHUB_TOKEN }}
```

#### 5.2 Add `CLAWHUB_TOKEN` to GitHub repo secrets

Manual step: generate token at clawhub.ai, add to repo settings → Secrets → Actions.

## Files Changed

| File | Change |
|------|--------|
| `skills/agent-desktop/SKILL.md` | Sync from `.agents/`, add ClawHub metadata, remove macos.md reference |
| `skills/agent-desktop/references/commands-system.md` | Sync from `.agents/` (adds notification commands) |
| `skills/agent-desktop/references/macos.md` | **Delete** (moved to platform skill) |
| `skills/agent-desktop-macos/SKILL.md` | **New** — moved from `.claude/skills/`, add ClawHub metadata |
| `skills/agent-desktop-macos/references/notifications.md` | **New** — NC details extracted if SKILL.md is too large |
| `scripts/link-skills.sh` | **New** — symlink automation |
| `npm/scripts/postinstall.js` | Add skill install prompt after binary download |
| `.github/workflows/release.yml` | Add `publish-skills` job |

## Ongoing Convention: Skills Updated With Every Feature

This plan establishes a **permanent convention**: every new feature, command, or platform change must update the corresponding skill files. This is enforced via:

1. **Memory rule** — added to `MEMORY.md` under "Skill Maintenance (MANDATORY)" so it's loaded into every conversation context. Claude will automatically update skills as part of feature work.
2. **PRD addendum** — `docs/prd-addendum-skill-maintenance.md` has detailed per-phase rules.
3. **`/skill-creator` skill** — used when writing or updating any SKILL.md to ensure quality.

| Change Type | Skill Update Required |
|---|---|
| New CLI command | Add to `skills/agent-desktop/references/commands-*.md` |
| New platform adapter | Create `skills/agent-desktop-{platform}/SKILL.md` + `references/` |
| App-specific quirk discovered | Add `skills/agent-desktop-{platform}/references/{app}.md` |
| Changed CLI flags or JSON output | Update all affected skill files |
| New workflow pattern | Add to `skills/agent-desktop/references/workflows.md` |

## Not Changing

- `crates/` — no Rust code changes
- `src/` — no binary changes
- `release-please-config.json` — skill versions are managed by ClawHub CLI `--bump`, not release-please
- `.gitignore` — `skills/` is already tracked, `.claude/` is already ignored

## Dependencies & Risks

- **CLAWHUB_TOKEN secret required** — CI job will fail without it. Must be configured before first automated release.
- **ClawHub automated review** — published skills go through syntax validation and permission scanning (<5 min). If rejected, need to fix and re-publish.
- **First publish should be manual** — run `clawhub sync --root skills/ --all --dry-run` locally to verify before relying on CI.

## Sources

- **Origin brainstorm:** [docs/brainstorms/2026-03-02-clawhub-skill-publishing-brainstorm.md](../brainstorms/2026-03-02-clawhub-skill-publishing-brainstorm.md) — key decisions: nested hierarchy, CI auto-publish, platform > apps scoping
- Existing CI pattern: `.github/workflows/release.yml` — 4-job pipeline (release-please → build → publish-github → publish-npm)
- Skill maintenance rules: `docs/prd-addendum-skill-maintenance.md`
- ClawHub CLI: [docs.openclaw.ai/tools/clawhub](https://docs.openclaw.ai/tools/clawhub)
