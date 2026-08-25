---
name: agent-desktop
version: 0.4.0
tags: desktop-automation, accessibility, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  Desktop automation via native OS accessibility trees using the agent-desktop CLI.
  Use when an AI agent needs to observe, interact with, or automate desktop applications
  (click buttons, fill forms, navigate menus, read UI state, toggle checkboxes, scroll,
  drag, type text, take screenshots, manage windows, use clipboard, manage notifications).
  Covers 59 command names (55 operational; four held-input names fail closed until
  daemon ownership exists) across observation, interaction, keyboard/mouse, app
  lifecycle, notifications (macOS), clipboard, wait, session lifecycle, and a
  `skills` command that bundles docs straight from the binary.
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
| `references/commands-system.md` | launch (including `--cdp` for Chromium web contents), close, windows, clipboard, wait, batch, session, status, permissions, version |
| `references/workflows.md` | 16 common patterns: forms, menus, dialogs, scroll-find, drag-drop, async wait, anti-patterns |
| `references/macos.md` | macOS permissions/TCC, AX API internals, smart activation chain, surfaces, Notification Center, troubleshooting |

## The Observe-Act Loop (Progressive Skeleton Traversal)

When you know the target's role or exact name, use `find --role ... --name ... --exact` directly. Otherwise, use **progressive skeleton traversal** for dense or unfamiliar apps: a shallow overview followed by targeted drill-downs.

```
1. SKELETON → agent-desktop snapshot --skeleton --app "App" -i --compact
   Parse the overview. Identify the region containing your target.
   Regions show children_count (e.g., "Sidebar" with children_count: 42).
   The nearest safely resolvable container has a ref for drill-down.
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
- Dense Electron apps (Slack, VS Code, Discord, Notion) that are **already running** — for one you are launching fresh, `launch --cdp` plus a CDP client (agent-browser preferred) reads the web contents faster than any skeleton walk (see principle 15)
- Any app where full snapshot exceeds ~50 refs
- Multi-region workflows (sidebar + main content + toolbar)

## Ref System

- Refs are assigned depth-first and emitted with their snapshot, for example `@s8f3k2p9:e1`, `@s8f3k2p9:e2`, `@s8f3k2p9:e3`. Legacy bare refs require an explicit `--snapshot`.
- An element gets a ref when it is addressable for an action: an interactive role (button, textfield, checkbox, link, menuitem, tab, slider, combobox, treeitem, cell, radiobutton, switch, ...) **or** any element advertising an action — so `scrollarea` (Scroll) and `disclosure` (Expand/Collapse) are ref-able and `scroll`/`expand`/`collapse` can target them
- A `SetFocus`-only affordance does not earn a ref on its own
- In skeleton mode, each truncated branch exposes the deepest safely resolvable drill target using stable text, native ID, or bounds evidence; the nearest resolvable ancestor is used when the boundary itself is anonymous
- Static text and non-actionable groups/containers remain in tree for context but have no ref
- Refs are deterministic within a snapshot but NOT stable across snapshots if UI changed
- Snapshot output uses qualified refs that embed `snapshot_id` and need no separate `--snapshot`; a session-owned ref still requires the same `--session` or `AGENT_DESKTOP_SESSION` scope because lookup never crosses namespaces
- `last_refmap.json` is only a latest-snapshot inspection artifact. The command path uses snapshot-scoped storage.
- After any action that changes UI, re-drill the affected region or re-snapshot
- **Scoped invalidation:** re-drilling a qualified root ref only replaces refs from that root's previous drill — refs from other regions and the skeleton itself are preserved
- **Strict resolution:** stale refs return `STALE_REF`; duplicate plausible targets return `AMBIGUOUS_TARGET` instead of choosing arbitrarily.
- **Actionability:** every ref-addressed action checks its applicable live visibility, stability, enabled, editability, policy, supported-action, and hit-test requirements under one bounded budget before a single dispatch. Pointer actions focus before their final geometry read, re-resolve moving endpoints, and return `TIMEOUT` with `details.kind: "actionability_timeout"` instead of sending input after the deadline.
- **Headless vs headed:** ref actions are strictly headless by default: semantic accessibility APIs only, with no focus stealing, cursor movement, or synthesized keyboard input. In headed mode, core focuses the exact ref window before dispatch; pointer actions also require a verified target point, while the adapter owns OS delivery. On macOS, `click`, `right-click`, `type`, `clear`, and `scroll` are physical-first; double/triple-click, hover, and drag are physical-only; expand/collapse and other semantic actions remain semantic. Raw `--xy` input has no window identity and never steals focus. `press` is explicit physical keyboard input; held-input commands (`key-down`, `key-up`, `mouse-down`, `mouse-up`) are reserved and fail closed in the stateless CLI.
- **Sessions and tracing:** run `session start` once per agent run to create a manifest with `trace: on` (default), then pass its returned ID with `--session` or `AGENT_DESKTOP_SESSION`. Use `session start --screenshots` when you need replay artifacts (`artifacts: full`): pre/post-action PNGs and refmap copies under the session trace directory (sensitive — treat exports like screenshots). Commands in that explicit scope record JSONL automatically to per-process segments under `~/.agent-desktop/sessions/<id>/trace/<pid>-<procTs>.jsonl` — no `--trace` on every call. Read traces back with `trace show` (bounded JSON for agents) or `trace export` (single-file HTML for humans). A session owns both its trace and its latest-snapshot namespace. Snapshot lookup never searches another namespace. **`--session <id>` alone** (no manifest from `session start`) selects only the snapshot namespace — existing callers see no surprise trace files. **`--trace <path>`** still overrides to one atomic file for CI or one-offs. Activation precedence is `--session` > `AGENT_DESKTOP_SESSION` > no session; `session start` does not activate later processes. Multi-agent shared sessions: each agent acts on qualified refs from its own snapshot — implicit latest is not a cross-agent guarantee. Run `status` to see `session_id` and `tracing`. Trace lines include `ts_ms`, monotonic per-process `seq`, and redacted sensitive fields (`text`, `value`, `expected`, `name`, `username`, `description`, `label`, `query`, `secret`, `token`, `password`, `title`, `url`, `help`, `placeholder` → `{ "redacted": true }`). `--trace-strict` fails on trace setup and pre-action writes; post-action success traces are best-effort.

## JSON Output Contract

Every command returns a JSON envelope on stdout:

**Success:** `{ "version": "2.3", "ok": true, "command": "snapshot", "data": { ... } }`
**Error:** `{ "version": "2.3", "ok": false, "command": "click", "error": { "code": "STALE_REF", "message": "...", "suggestion": "..." } }`

The `error` object may also carry an optional `details` object (e.g. the actionability report on an actionability failure, candidate summaries on `AMBIGUOUS_TARGET`, or the last observed state on a `wait` `TIMEOUT`). Parse errors leniently — `details` and future fields are additive, so do not reject responses with unknown keys.

An actionability failure on a hit-test action (`click`, `double-click`, `right-click`, `triple-click`, `hover`, `drag`) can carry a `receives_events` check with `reason: "occluded by <role>"` plus a structured `occluder: { "role", "name", "bounds" }` — another element is on top of the target. Bring the target window/element to the front (or dismiss the occluder) rather than blind-retrying; see `references/commands-interaction.md` for the full check list.

Exit codes: `0` success, `1` structured error, `2` argument error.

### Error Codes

| Code | Meaning | Recovery |
|------|---------|----------|
| `PERM_DENIED` | Accessibility or Screen Recording permission not granted | Grant the named permission in System Settings |
| `ELEMENT_NOT_FOUND` | Ref cannot be resolved against the live UI | Re-run snapshot, use fresh ref |
| `APP_NOT_FOUND` | App not running | Launch it first |
| `ACTION_FAILED` | AX action rejected | Try an explicit alternative command |
| `ACTION_NOT_SUPPORTED` | Element can't do this | Use different command |
| `STALE_REF` | Ref could not be re-identified in the live UI | Use the `snapshot_id` returned with this ref; if the UI changed or the target disappeared, re-run `snapshot` / `snapshot --skeleton` to get fresh refs |
| `AMBIGUOUS_TARGET` | Multiple elements matched the old ref identity | Re-run snapshot and choose a more specific ref |
| `SNAPSHOT_NOT_FOUND` | Snapshot ID is missing or expired | Run `snapshot` again and use the returned ID |
| `POLICY_DENIED` | A physical/headed path was blocked | Use an explicit mouse/focus/keyboard command if physical interaction is intended |
| `APP_UNRESPONSIVE` | A read-only AX liveness probe also failed after an uncertain mutation response | Inspect with a fresh snapshot and wait for the app to recover before deciding whether to retry |
| `WINDOW_NOT_FOUND` | No matching window | Check app name, use list-windows |
| `PLATFORM_NOT_SUPPORTED` | Adapter method not implemented on this platform | Use a supported platform adapter |
| `TIMEOUT` | Wait or actionability condition not met | Inspect `error.details.kind`; increase the command budget only after checking the last report, and use `AGENT_DESKTOP_CHAIN_TIMEOUT_MS` only for `chain_deadline` |
| `INVALID_ARGS` | Bad arguments | Check command syntax |
| `NOTIFICATION_NOT_FOUND` | Notification index no longer exists | Re-run list-notifications |
| `INTERNAL` | Unexpected platform/OS failure (e.g. event synthesis failed) | Read `message`/`suggestion` for cleanup state, then retry once; persistent failures indicate an environment problem |

`TIMEOUT` errors carry a `details` object whose `kind` field selects the schema. `kind: "wait_timeout"` includes `predicate`, `timeout_ms`, and `last_observed` or `last_error`, plus `ref`/`title`/`text_chars` depending on the wait mode. `kind: "chain_deadline"` includes `value_before`, `value_at_timeout`, `target`, and `mutated` (increment waits) or `wanted_expanded`/`observed_expanded` (disclosure waits). `mutated: true` — or an unknown `observed_expanded` state — means re-read the element before retrying; `mutated: false` means the state did not change and retrying directly is safe.

## Command Quick Reference (59 names, 55 operational)

### Observation
```
agent-desktop snapshot --skeleton --app "App" -i --compact  # Skeleton overview (preferred)
agent-desktop snapshot --root @s8f3k2p9:e3 -i --compact              # Drill into region
agent-desktop snapshot --app "App" -i                       # Full tree (simple apps)
agent-desktop snapshot --app "App" --surface menu -i        # Surface snapshot
agent-desktop screenshot --app "App" out.png                # PNG screenshot
agent-desktop find --app "App" --role button                # Search elements
agent-desktop find --root @s8f3k2p9:e3 --role button        # Search one region only
agent-desktop find --app "App" --surface menubar --name "Save" --first  # Search a menu
agent-desktop get @e1 --snapshot <snapshot_id> --property text       # Read element property
agent-desktop is @e1 --snapshot <snapshot_id> --property enabled     # Check element state
agent-desktop list-surfaces --app "App"                     # Available surfaces
```

### Interaction
```
agent-desktop click @e5 --snapshot <snapshot_id> # Headless semantic click
agent-desktop --headed double-click @s8f3k2p9:e3 # physical double-click
agent-desktop --headed triple-click @s8f3k2p9:e2 # physical triple-click
agent-desktop right-click @s8f3k2p9:e5          # Right-click; inspect the resulting menu/effect separately
agent-desktop type @e2 --snapshot <snapshot_id> "hello"  # Headless AX text insertion when supported
agent-desktop set-value @s8f3k2p9:e2 "new value"         # Set value directly
agent-desktop clear @s8f3k2p9:e2                         # Clear element value
agent-desktop focus @s8f3k2p9:e2                         # Set keyboard focus
agent-desktop select @s8f3k2p9:e4 "Option B"             # Select dropdown/list option
agent-desktop toggle @s8f3k2p9:e6                        # Toggle checkbox/switch
agent-desktop check @s8f3k2p9:e6                         # Idempotent check
agent-desktop uncheck @s8f3k2p9:e6                       # Idempotent uncheck
agent-desktop expand @s8f3k2p9:e7                        # Expand disclosure
agent-desktop collapse @s8f3k2p9:e7                      # Collapse disclosure
agent-desktop scroll @s8f3k2p9:e1 --direction down       # Scroll element
agent-desktop scroll-to @s8f3k2p9:e8                     # Scroll into view
```

### Keyboard & Mouse
```
agent-desktop press cmd+c                       # Key combo
agent-desktop press return --app "App"          # Targeted key press
agent-desktop --headed hover @s8f3k2p9:e5       # Explicit cursor movement
agent-desktop --headed hover --xy 500,300       # Cursor to coordinates
agent-desktop --headed drag --from @s8f3k2p9:e1 --to @s8f3k2p9:e5 # Drag between elements
agent-desktop --headed mouse-click --xy 500,300 # Click at coordinates
agent-desktop --headed mouse-move --xy 100,200  # Move cursor
```

`key-down`, `key-up`, `mouse-down`, and `mouse-up` return `ACTION_NOT_SUPPORTED` until a stateful daemon can own held-input lifetime. Use `press`, `mouse-click`, or `drag` instead.

### App & Window
```
agent-desktop launch "System Settings"          # Launch; returns once running
agent-desktop launch "TextEdit" --activate       # Also bring it forward and wait for a window
agent-desktop launch "Obsidian" --cdp            # Fresh launch + verified Chrome DevTools Protocol port for web contents
agent-desktop close-app "TextEdit"              # Quit gracefully
agent-desktop close-app "TextEdit" --force      # Force quit; SIGKILL if SIGTERM does not exit
agent-desktop list-windows --app "Finder"       # List windows
agent-desktop list-apps                         # List running GUI apps
agent-desktop focus-window --app "Finder"       # Bring to front
agent-desktop resize-window --app "App" --width 800 --height 600
agent-desktop move-window --app "App" --x 0 --y 0
agent-desktop minimize --app "App"
agent-desktop maximize --app "App"
agent-desktop restore --app "App"
```

Use `--window-id <id>` from `list-windows` instead of `--app` when an app has multiple windows.

### Notifications
```
agent-desktop --headed list-notifications                # Open center if needed, then list
agent-desktop --headed list-notifications --app "Slack"  # Filter by app
agent-desktop --headed list-notifications --text "deploy" --limit 5  # Filter by text
agent-desktop --headed dismiss-notification 1 --expected-app Slack --expected-title "Deploy complete"
agent-desktop --headed dismiss-all-notifications         # Dismiss all
agent-desktop --headed dismiss-all-notifications --app "Slack"  # Dismiss all from app
agent-desktop --headed notification-action 1 "Reply" --expected-app Slack --expected-title "Deploy complete"
```

Every notification mutation requires global `--headed`; single-notification
mutations also require an app or title fingerprint from the same listing.
Headless listing works only while Notification Center is already open.

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
agent-desktop wait --element @s8f3k2p9:e5 --predicate actionable --timeout 5000 # Wait until actionable
agent-desktop wait --element @s8f3k2p9:e5 --predicate value --value "Done" --timeout 5000 # Wait for value
agent-desktop wait --window "Title"             # Wait for window
agent-desktop wait --text "Done" --app "App"    # Wait for text
agent-desktop wait --menu --app "App"           # Wait for menu surface
agent-desktop wait --menu-closed --app "App"    # Wait for menu dismissal
agent-desktop wait --notification --app "App"   # Wait for new notification
```

### System
```
agent-desktop session start [--name LABEL] [--screenshots] [--no-trace] [--cursor]  # Creates a session; pass the returned ID explicitly
agent-desktop session end [id]                                      # Seal manifest
agent-desktop session list                                          # List session manifests
agent-desktop session gc [--older-than SECS] [--ended]              # Reclaim ended/stale sessions
agent-desktop --session <id> cursor-overlay enable [--label TEXT] [--max-words N] [--fill HEX] [--rim HEX] [--accent HEX] [--size N] [--no-ripple] [--no-highlight]
#   no flags = white cursor, blue ripple, blue element outline; `session start --cursor` does both steps at once
agent-desktop --session <id> cursor-overlay disable                 # Remove the session overlay and stop its renderer
agent-desktop trace show [--limit N] [--event PREFIX]               # Merge trace segments (default tail 500; 0 = all)
agent-desktop trace export [--out path.html] [--limit N]            # Self-contained HTML viewer (default tail 5000)
agent-desktop status                            # Health, session_id, tracing, artifacts, permissions
agent-desktop permissions                       # Check permission
agent-desktop permissions --request             # Request missing permissions in an isolated helper
agent-desktop version                           # Version info (always JSON envelope)
agent-desktop batch '[...]' --stop-on-error     # Batch uses the same typed command path as CLI
agent-desktop skills                            # List bundled skill docs
agent-desktop skills get desktop --full         # Load this skill + all references
```

## Key Principles for Agents

1. **Find known targets; skeleton unknown structure.** If role or exact name is known, use targeted `find` first. Use `--skeleton -i --compact` when the region is unknown, then drill with `--root @ref`. For fresh Chromium launches, prefer `launch --cdp` plus a CDP client.
2. **Use `-i --compact` flags.** Filters to interactive elements and collapses empty wrappers, minimizing tokens.
3. **Refs are snapshot-scoped.** Keep `snapshot_id` for deterministic multi-step use; re-drill the affected region after any UI-changing action. Scoped invalidation keeps other refs intact.
4. **Prefer refs over coordinates.** `click @s8f3k2p9:e5` > `agent-desktop --headed mouse-click --xy 500,300`.
5. **Use `wait` for async UI.** After launch/dialog triggers, wait for expected state.
6. **Check permissions first.** Run `permissions` on first use; screenshots also need Screen Recording.
7. **Handle errors.** Branch on `error.code` only — `error.message` and `error.suggestion` text is informational and may change between versions.
8. **Scope targeted searches.** Pair known names with role and `--exact`; use `--limit 2` before a mutation when uniqueness is not guaranteed. Narrow with `--root @ref` for one region or `--surface menubar` for a menu.
9. **Use surfaces for overlays.** `snapshot --surface menu` for menus, `--surface sheet` for dialogs. Never `--skeleton` for surfaces — they're already focused. Reach one item inside an overlay with `find --surface`, not a full surface snapshot.
10. **Read `data.surfaces` after acting.** An action that opens a sheet, menu, or alert reports it there. Target that overlay next instead of searching windows for what changed.
11. **Batch for performance.** Multiple commands in one invocation.
12. **Headless by default.** Ref actions use semantic AX paths and block silent focus stealing, cursor movement, keyboard synthesis, and pasteboard insertion. Use `--headed` only when exact-window focus or physical delivery is intended; raw coordinates never imply focus.
13. **Start a session once per run.** `session start` creates the manifest; pass its returned ID through `AGENT_DESKTOP_SESSION` for the run or `--session <id>` for one command. It does not activate later processes implicitly.
14. **Trace hard failures.** With an active trace-enabled session, segments are written automatically. Add `--trace /tmp/agent-desktop.jsonl` only when you need a single override file (CI, one-offs). Check `status` when unsure whether tracing is active.
15. **Relocate state with `AGENT_DESKTOP_HOME`.** When set, the env value is the state root itself — sessions, refmaps, traces, and locks all live under it, for every subcommand. Default stays `~/.agent-desktop`. The value must be an absolute path; a relative or empty value fails with `INVALID_ARGS` before any command runs. `status` reports the resolved root as `state_root`. Explicit output paths (`screenshot --out`, `--trace <path>`) are never re-rooted.
16. **Chromium-app strategy.** The accessibility path is the default, and the only option for already-running apps and for native surfaces (menus, dialogs, windows). `launch --cdp` plus a CDP client (agent-browser preferred; any CDP client works) is the opt-in fast path for a Chromium app's web contents, when a fresh launch is acceptable.
