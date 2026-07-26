# Electron Tree Compaction Brainstorm

**Date:** 2026-03-01
**Status:** Decided
**Trigger:** Electron depth-skip fix (commit a19c1b5) made Slack snapshots work but raised concern about context bloat for AI agents.

## Problem Statement

After enabling Electron/web app support via depth-skip for non-semantic `AXGroup` wrappers, Slack snapshots with `-i` return **268 nodes / ~6,700 tokens** — 4x the cost of Finder's 96 nodes / ~1,625 tokens. 40% of Slack's nodes are structural wrappers carrying no useful info.

The question: should we build a smart architecture (e.g., a standalone Electron-to-AX bridge) to reduce this, or is there a simpler solution?

## Data

| Mode | Nodes | Refs | Tokens | Notes |
|------|-------|------|--------|-------|
| Slack `-i` (current) | 268 | 162 | ~6,700 | 40% structural noise |
| Slack `-i` + compact | 204 | 162 | ~5,475 | 18% savings |
| Slack full (no `-i`) | 528 | 162 | ~12,000 | 69% structural noise |
| Finder `-i` | 96 | 73 | ~1,625 | 24% structural (baseline) |

Snapshot speed: Slack ~220ms, Finder ~260ms. **Speed is not an issue.**

## What We're Building

**Implement `--compact` flag** (already wired in CLI as a no-op) to collapse single-child non-interactive pass-through nodes during tree serialization.

**Collapse rule:** If a node has no `ref_id`, no `name`, no `value`, and exactly one child — skip it and hoist the child. If it has a `name` but the child doesn't, promote the name to the child and skip.

This is a tree serialization optimization, not an Electron-specific feature. Benefits all apps.

## Why This Approach

1. **The flag already exists** — `--compact` is wired through CLI, `TreeOptions`, and the snapshot pipeline. Just needs implementation.
2. **18% token savings for free** — 64 nodes removed from Slack with zero information loss.
3. **No separate package needed** — The Electron bridge idea (CDP-based, standalone crate) is YAGNI. The actual token cost is dominated by real content (162 interactive elements with descriptive names), not structural noise.
4. **Composable with `--surface`** — Agents that need even smaller snapshots should scope to a surface (sheet, menu, alert) rather than us over-pruning the full tree.

## Approaches Considered

### 1. Implement `--compact` with chain collapsing (CHOSEN)
- Collapse unnamed/valueless single-child non-interactive nodes
- 15-18% token reduction, zero information loss, trivial to implement
- Works for all apps, not Electron-specific

### 2. Standalone Electron-to-AX bridge package
- Separate crate that connects via CDP, extracts real DOM structure, maps to AX tree
- Would require `--remote-debugging-port` (invasive), becomes agent-browser territory
- **Rejected:** YAGNI. Pure AX approach already works. Adds dependency and complexity.

### 3. Flat interactive-only output (`--flat`)
- Output just refs as `[{ref, role, name, value, path}]` — no tree
- Minimal tokens (~2,000 for Slack) but agents lose spatial/hierarchy context
- **Deferred:** Could add later if compact isn't enough. Not needed now.

## Key Decisions

- **Chain collapsing preserves tree structure** — agents still see hierarchy, just without empty wrapper noise
- **Not Electron-specific** — universal tree optimization via `--compact` flag
- **No separate bridge package** — unnecessary given 220ms speed and ~5,475 post-compact tokens
- **Agents control scope via `--surface`** — surface-scoped snapshots are the right lever for smaller output, not tree pruning

## Open Questions

None — approach is clear and implementation is straightforward.
