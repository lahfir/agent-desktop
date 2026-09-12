---
title: Visual debug artifacts for snapshots and clicks
date: 2026-09-09
category: developer-experience
module: src/visual_debug
problem_type: developer_experience
component: tooling
severity: medium
applies_when:
  - "Demonstrating snapshot targets or investigating a click visually"
  - "Maintaining screenshot alignment and viewer filters"
tags: [visual-debug, skeleton, screenshots, role-filters, macos]
---

# Visual debug artifacts for snapshots and clicks

## Context

JSON is useful for agents but awkward for a visual demo. The opt-in HTML viewer shows captured window pixels with labeled element bounds. Its sidebar groups nodes by role instead of displaying one long list.

## Guidance

- Use `--debug --screenshot PATH.html` with window-surface `snapshot` or ref-based `click`. The destination must be new; screenshots and labels are sensitive.
- Keep role groups collapsed initially. Checkbox filters select roles without expanding their lists; multiple selections combine, and no selection shows all. Preserve original element numbers and refs when grouping.
- Capture the exact source window, not whichever window is focused. Use shadow-free window captures and check geometry before drawing bounds; macOS implements this in `crates/macos/src/system/screenshot.rs`.
- Keep before/after click images separate. The before target is resolved before dispatch, not necessarily at the final delivery point. Do not reuse its bounds on the after image or treat screenshots as proof of delivery.
- Preserve the original command result if late artifact capture or writing fails. Report a warning instead of encouraging a duplicate click. `src/visual_debug/mod.rs` owns this boundary.
- Escape app-controlled data before embedding it in HTML and render labels with `textContent`. Keep the viewer self-contained with no network requests.

## Why This Matters

The debug view must help explain an action without changing its meaning, capturing the wrong window, or making an uncertain result look verified.

## When to Apply

Use this when changing `src/visual_debug/` or the window-frame screenshot adapter. Rebuild the binary and generate a new artifact after viewer changes; existing HTML files are static.

## Examples

See [Visual debug usage](../../../skills/agent-desktop/references/commands-observation.md#visual-debug) for capture commands and viewer controls.

```bash
cargo test -p agent-desktop --bin agent-desktop visual_debug
node --test src/visual_debug/viewer_filter.test.js
```

Also check role-only filtering, combined roles, reset, group expansion, individual selection, and narrow-screen layout in a browser. Live TextEdit captures verified Retina alignment and a Bold click; a separate accessibility read confirmed the checked state.
