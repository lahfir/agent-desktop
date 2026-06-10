---
name: agent-desktop
version: 0.1.14
tags: desktop-automation, accessibility, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  Desktop automation via native OS accessibility trees using the agent-desktop CLI.
  Use when an AI agent needs to observe, interact with, or automate desktop applications
  (click buttons, fill forms, navigate menus, read UI state, toggle checkboxes, scroll,
  drag, type text, take screenshots, manage windows, use clipboard, manage notifications).
  Covers 54 commands across observation, interaction, keyboard/mouse, app lifecycle,
  notifications (macOS), clipboard, wait, and a `skills` command that prints these
  bundled docs straight from the binary.
  Triggers on: "click button", "fill form", "open app", "read UI", "automate desktop",
  "accessibility tree", "snapshot app", "type into field", "navigate menu", "toggle checkbox",
  "take screenshot", "desktop automation", "agent-desktop", or any desktop GUI interaction task.
  Supports the macOS Phase 1 adapter, with Windows and Linux planned against
  the same core contracts.
---

# agent-desktop

CLI tool enabling AI agents to observe and control desktop applications via native OS accessibility trees.

**Core principle:** agent-desktop is NOT an AI agent. It is a tool that AI agents invoke. It outputs structured JSON with ref-based element identifiers. The observation-action loop lives in the calling agent.

## Installation

```bash
npm install -g agent-desktop
# or
bun install -g --trust agent-desktop
```

Requires macOS 12+ with Accessibility permission granted to your terminal. Screen Recording permission is also required for screenshots.

## Reference Files

Detailed documentation is split into focused reference files. Read them as needed:

| Reference | Contents |
|-----------|----------|
| `references/commands-observation.md` | snapshot, find, get, is, screenshot, list-surfaces — all flags, output examples |
| `references/commands-interaction.md` | click, type, set-value, select, toggle, scroll, drag, keyboard, mouse — choosing the right command |
| `references/commands-system.md` | launch, close, windows, clipboard, wait, batch, status, permissions, version |
| `references/workflows.md` | 12 common patterns: forms, menus, dialogs, scroll-find, drag-drop, async wait, anti-patterns |
| `references/macos.md` | macOS permissions/TCC, AX API internals, smart activation chain, surfaces, Notification Center, troubleshooting |

## The Observe-Act Loop (Progressive Skeleton Traversal)

Use **progressive skeleton traversal** as the default approach. It reduces token consumption 78-96% for dense apps by exploring the UI in two phases: a shallow skeleton overview, then targeted drill-downs into regions of interest.

```
1. SKELETON → agent-desktop snapshot --skeleton --app "App" -i --compact
   Parse the overview. Identify the region containing your target.
   Regions show children_count (e.g., "Sidebar" with children_count: 42).
   Named containers at truncation boundary have refs for drill-down.
   Keep the returned snapshot_id.

2. DRILL    → agent-desktop snapshot --root @e3 --snapshot <snapshot_id> -i --compact
   Expand the target region. Now you see its interactive elements.

3. ACT      → agent-desktop click @e12 --snapshot <snapshot_id>  (or type, select, toggle...)

4. VERIFY   → agent-desktop snapshot --root @e3 --snapshot <snapshot_id> -i --compact
   Re-drill the same region to confirm the state change.
   Scoped invalidation: only @e3's subtree refs are replaced.

5. REPEAT   → Continue drilling other regions or acting as needed.
```

**When to skip skeleton and use full snapshot instead:**
- Simple apps with few elements (Finder, Calculator, TextEdit)
- You already know the exact element name — use `find` instead
- Surface snapshots (menus, sheets, alerts) — these are already focused

**When skeleton shines:**
- Dense Electron apps (Slack, VS Code, Discord, Notion)
- Any app where full snapshot exceeds ~50 refs
- Multi-region workflows (sidebar + main content + toolbar)

## Ref System

- Refs assigned depth-first: `@e1`, `@e2`, `@e3`...
- An element gets a ref when it is addressable for an action: an interactive role (button, textfield, checkbox, link, menuitem, tab, slider, combobox, treeitem, cell, radiobutton, switch, ...) **or** any element advertising an action — so `scrollarea` (Scroll) and `disclosure` (Expand/Collapse) are ref-able and `scroll`/`expand`/`collapse` can target them
- A `SetFocus`-only affordance does not earn a ref on its own
- In skeleton mode, named/described containers at truncation boundary also get refs (drill-down targets with empty `available_actions`)
- Static text and non-actionable groups/containers remain in tree for context but have no ref
- Refs are deterministic within a snapshot but NOT stable across snapshots if UI changed
- Every snapshot returns `snapshot_id`; ref-consuming commands accept `--snapshot <snapshot_id>`, and explicit snapshot IDs do not require also passing `--session`
- `last_refmap.json` is only a latest-snapshot inspection artifact. The command path uses snapshot-scoped storage.
- After any action that changes UI, re-drill the affected region or re-snapshot
- **Scoped invalidation:** re-drilling `--root @e3` only replaces refs from @e3's previous drill — refs from other regions and the skeleton itself are preserved
- **Strict resolution:** stale refs return `STALE_REF`; duplicate plausible targets return `AMBIGUOUS_TARGET` instead of choosing arbitrarily.
- **Actionability:** ref actions check live visibility, stability, enabled state, supported action, policy, and editability before dispatch.
- **Headless vs headed:** ref actions are headless by default (AX-only, no cursor) and fail closed with `POLICY_DENIED` when only a physical gesture would work. Pass the global `--headed` flag to permit cursor movement and focus stealing so the physical click/double-click/scroll/keypress fallbacks can complete; the AX path is still tried first, so `--headed` never regresses headless-capable elements. Raw-input commands (`press`, `hover`, `drag`, `mouse-*`, `key-down`/`key-up`) are always physical and ignore the mode.
- **Sessions:** use `--session <id>` for concurrent or multi-agent runs that share a latest snapshot pointer; batch entries may override with `"session": "id"`.
- **Trace:** use `--trace <path>` for JSONL diagnostics outside stdout; `--trace-strict` fails on trace setup and pre-action writes. Post-action success traces are best-effort because the desktop mutation already happened. Trace fields whose keys contain `text`, `value`, `expected`, `name`, `description`, `message`, `label`, `query`, `secret`, `token`, or `password` are redacted to `{ "redacted": true, "chars_bucket": "..." }` at every nesting depth — do not expect raw values in trace files. Top-level `--trace` is inherited by every `batch` entry, including entries with a `session` override.

## JSON Output Contract

Every command returns a JSON envelope on stdout:

**Success:** `{ "version": "2.0", "ok": true, "command": "snapshot", "data": { ... } }`
**Error:** `{ "version": "2.0", "ok": false, "command": "click", "error": { "code": "STALE_REF", "message": "...", "suggestion": "..." } }`

The `error` object may also carry an optional `details` object (e.g. the actionability report on an actionability failure, candidate summaries on `AMBIGUOUS_TARGET`, or the last observed state on a `wait` `TIMEOUT`). Parse errors leniently — `details` and future fields are additive, so do not reject responses with unknown keys.

Exit codes: `0` success, `1` structured error, `2` argument error.

### Error Codes

| Code | Meaning | Recovery |
|------|---------|----------|
| `PERM_DENIED` | Accessibility or Screen Recording permission not granted | Grant the named permission in System Settings |
| `ELEMENT_NOT_FOUND` | Ref cannot be resolved against the live UI | Re-run snapshot, use fresh ref |
| `APP_NOT_FOUND` | App not running | Launch it first |
| `ACTION_FAILED` | AX action rejected | Try an explicit alternative command |
| `ACTION_NOT_SUPPORTED` | Element can't do this | Use different command |
| `STALE_REF` | Ref from old snapshot | Re-run snapshot |
| `AMBIGUOUS_TARGET` | Multiple elements matched the old ref identity | Re-run snapshot and choose a more specific ref |
| `SNAPSHOT_NOT_FOUND` | Snapshot ID is missing or expired | Run `snapshot` again and use the returned ID |
| `POLICY_DENIED` | A physical/headed path was blocked | Use an explicit mouse/focus/keyboard command if physical interaction is intended |
| `WINDOW_NOT_FOUND` | No matching window | Check app name, use list-windows |
| `PLATFORM_NOT_SUPPORTED` | Adapter method not implemented on this platform | Use a supported platform adapter |
| `TIMEOUT` | Wait condition not met | Increase --timeout |
| `INVALID_ARGS` | Bad arguments | Check command syntax |
| `NOTIFICATION_NOT_FOUND` | Notification index no longer exists | Re-run list-notifications |

## Command Quick Reference (54 commands)

### Observation
```
agent-desktop snapshot --skeleton --app "App" -i --compact  # Skeleton overview (preferred)
agent-desktop snapshot --root @e3 -i --compact              # Drill into region
agent-desktop snapshot --app "App" -i                       # Full tree (simple apps)
agent-desktop snapshot --app "App" --surface menu -i        # Surface snapshot
agent-desktop screenshot --app "App" out.png                # PNG screenshot
agent-desktop find --app "App" --role button                # Search elements
agent-desktop get @e1 --snapshot <snapshot_id> --property text       # Read element property
agent-desktop is @e1 --snapshot <snapshot_id> --property enabled     # Check element state
agent-desktop list-surfaces --app "App"                     # Available surfaces
```

### Interaction
```
agent-desktop click @e5 --snapshot <snapshot_id> # AX-first click, no cursor move by default
agent-desktop double-click @e3                  # AXOpen; physical double-click uses mouse-click --count 2
agent-desktop triple-click @e2                  # Physical triple-click uses mouse-click --count 3
agent-desktop right-click @e5                   # Right-click; menu returned when verified
agent-desktop type @e2 --snapshot <snapshot_id> "hello"  # Headless AX text insertion when supported
agent-desktop set-value @e2 "new value"         # Set value directly
agent-desktop clear @e2                         # Clear element value
agent-desktop focus @e2                         # Set keyboard focus
agent-desktop select @e4 "Option B"             # Select dropdown/list option
agent-desktop toggle @e6                        # Toggle checkbox/switch
agent-desktop check @e6                         # Idempotent check
agent-desktop uncheck @e6                       # Idempotent uncheck
agent-desktop expand @e7                        # Expand disclosure
agent-desktop collapse @e7                      # Collapse disclosure
agent-desktop scroll @e1 --direction down       # Scroll element
agent-desktop scroll-to @e8                     # Scroll into view
```

### Keyboard & Mouse
```
agent-desktop press cmd+c                       # Key combo
agent-desktop press return --app "App"          # Targeted key press
agent-desktop key-down shift                    # Hold key
agent-desktop key-up shift                      # Release key
agent-desktop hover @e5                         # Explicit cursor movement
agent-desktop hover --xy 500,300                # Cursor to coordinates
agent-desktop drag --from @e1 --to @e5          # Drag between elements
agent-desktop mouse-click --xy 500,300          # Click at coordinates
agent-desktop mouse-move --xy 100,200           # Move cursor
agent-desktop mouse-down --xy 100,200           # Press mouse button
agent-desktop mouse-up --xy 300,400             # Release mouse button
```

### App & Window
```
agent-desktop launch "System Settings"          # Launch and wait
agent-desktop close-app "TextEdit"              # Quit gracefully
agent-desktop close-app "TextEdit" --force      # Force kill
agent-desktop list-windows --app "Finder"       # List windows
agent-desktop list-apps                         # List running GUI apps
agent-desktop focus-window --app "Finder"       # Bring to front
agent-desktop resize-window --app "App" --width 800 --height 600
agent-desktop move-window --app "App" --x 0 --y 0
agent-desktop minimize --app "App"
agent-desktop maximize --app "App"
agent-desktop restore --app "App"
```

### Notifications
```
agent-desktop list-notifications                # List all notifications
agent-desktop list-notifications --app "Slack"  # Filter by app
agent-desktop list-notifications --text "deploy" --limit 5  # Filter by text
agent-desktop dismiss-notification 1            # Dismiss by index
agent-desktop dismiss-all-notifications         # Dismiss all
agent-desktop dismiss-all-notifications --app "Slack"  # Dismiss all from app
agent-desktop notification-action 1 "Reply" --expected-app Slack   # Click action (with NC reorder guard)
```

### Clipboard
```
agent-desktop clipboard-get                     # Read clipboard
agent-desktop clipboard-set "text"              # Write to clipboard
agent-desktop clipboard-clear                   # Clear clipboard
```

### Wait
```
agent-desktop wait 1000                         # Pause 1 second
agent-desktop wait --element @e5 --snapshot <snapshot_id> --timeout 5000 # Wait for element
agent-desktop wait --element @e5 --predicate actionable --timeout 5000 # Wait until actionable
agent-desktop wait --element @e5 --predicate value --value "Done" --timeout 5000 # Wait for value
agent-desktop wait --window "Title"             # Wait for window
agent-desktop wait --text "Done" --app "App"    # Wait for text
agent-desktop wait --menu --app "App"           # Wait for menu surface
agent-desktop wait --menu-closed --app "App"    # Wait for menu dismissal
agent-desktop wait --notification --app "App"   # Wait for new notification
```

### System
```
agent-desktop status                            # Health check
agent-desktop permissions                       # Check permission
agent-desktop permissions --request             # Trigger permission dialog
agent-desktop version --json                    # Version info
agent-desktop batch '[...]' --stop-on-error     # Batch uses the same typed command path as CLI
agent-desktop skills                            # List bundled skill docs
agent-desktop skills get desktop --full         # Load this skill + all references
```

## Key Principles for Agents

1. **Skeleton first, drill second.** Start with `--skeleton -i --compact` for dense apps. Drill into regions with `--root @ref`. Full snapshot only for simple apps.
2. **Use `-i --compact` flags.** Filters to interactive elements and collapses empty wrappers, minimizing tokens.
3. **Refs are snapshot-scoped.** Keep `snapshot_id` for deterministic multi-step use; re-drill the affected region after any UI-changing action. Scoped invalidation keeps other refs intact.
4. **Prefer refs over coordinates.** `click @e5` > `mouse-click --xy 500,300`.
5. **Use `wait` for async UI.** After launch/dialog triggers, wait for expected state.
6. **Check permissions first.** Run `permissions` on first use; screenshots also need Screen Recording.
7. **Handle errors.** Branch on `error.code` only — `error.message` and `error.suggestion` text is informational and may change between versions.
8. **Use `find` for targeted searches.** Faster than any snapshot when you know role/name.
9. **Use surfaces for overlays.** `snapshot --surface menu` for menus, `--surface sheet` for dialogs. Never `--skeleton` for surfaces — they're already focused.
10. **Batch for performance.** Multiple commands in one invocation.
11. **Headless by default.** Ref actions use semantic AX paths and block silent focus stealing, cursor movement, keyboard synthesis, and pasteboard insertion. Use explicit `focus`, `press`, `hover`, `drag`, or `mouse-*` commands only when physical/headed interaction is intended.
12. **Use sessions for parallel work.** Add `--session <id>` when multiple agents or batches can run at once.
13. **Trace hard failures.** Add `--trace /tmp/agent-desktop.jsonl` when diagnosing stale, ambiguous, or actionability failures.
