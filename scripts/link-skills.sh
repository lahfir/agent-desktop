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
