# Common Automation Workflows

Patterns for using agent-desktop effectively in multi-step desktop automation tasks.

Snapshot output uses qualified refs such as `@s8f3k2p9:e3`. Examples that pair
a legacy bare ref with `--snapshot <snapshot_id>` intentionally demonstrate the
still-supported compatibility form; never use a bare ref without that flag.

## First-Time Setup

Before any automation, verify permissions.

On macOS (TCC):

```bash
agent-desktop permissions
# If PERM_DENIED:
agent-desktop permissions --request
# Then: System Settings > Privacy & Security > Accessibility > enable your terminal
```

For screenshots, also grant Screen Recording. `permissions` reports `accessibility`, `screen_recording`, and `automation` separately.

On Windows there is no permission dialog: UI Automation reads of same-integrity targets need no grant, and `permissions` probes UIA live (`automation` reports `not_required`). Input into an elevated target requires running from an equally elevated terminal, because the OS blocks cross-integrity input synthesis — see the `agent-desktop-windows` skill's permissions-and-elevation reference.

## Pattern: Session-Scoped Tracing (Default for Multi-Step Runs)

Start one session per agent run, then explicitly select its ID so tracing and the latest-snapshot namespace follow consistently — no `--trace` on every command.

```bash
# 1. Start once — creates manifest (trace: on) and trace/ directory
agent-desktop session start --name "invoice-bot"
# Note session_id from data.session_id, then export it for later processes
export AGENT_DESKTOP_SESSION=<session_id>

# 2. Observe-act loop — segments land under sessions/<id>/trace/<pid>-*.jsonl
agent-desktop snapshot --app "Preview" -i --compact
agent-desktop click @e3 --snapshot <snapshot_id>
agent-desktop status   # confirms session_id + tracing: true

# 3. End and reclaim when finished
agent-desktop session end "$AGENT_DESKTOP_SESSION"
agent-desktop session gc
```

**Concurrent independent agents:** set `AGENT_DESKTOP_SESSION=<id>` in each process. Each agent should use qualified refs from its own snapshot when sharing a session ID.

**Namespace without tracing:** `session start --no-trace` or bare `--session legacy-id` (no manifest) — snapshots namespaced, no JSONL files.

**Override file trace:** `--trace /tmp/run.jsonl` still forces a single file regardless of session manifest.

## Pattern: Progressive Skeleton Traversal (Default for Dense Apps)

The recommended approach for Electron apps (Slack, VS Code, Discord) and any app with 50+ interactive elements. Reduces token consumption 78-96%.

```bash
# 1. Get skeleton overview — shallow 3-level map with children_count hints
agent-desktop snapshot --skeleton --app "Slack" -i --compact
# Keep snapshot_id = <snapshot_id>
# Output shows regions like:
#   @e1 = group "Workspaces" (children_count: 4)
#   @e2 = group "Channels" (children_count: 42)
#   @e3 = group "Messages" (children_count: 156)
#   @e4 = button "New Message"    ← interactive elements at top levels still get refs

# 2. Identify the region you need and drill into it
agent-desktop snapshot --root @e2 --snapshot <snapshot_id> -i --compact
# Now you see all 42 children inside "Channels" with full refs

# 3. Act on an element found in the drill-down
agent-desktop click @e18 --snapshot <snapshot_id>  # Click "general" channel

# 4. Re-drill the same or a different region to verify / continue
agent-desktop snapshot --root @e3 --snapshot <snapshot_id> -i --compact
# Scoped invalidation: only @e3's previous refs are replaced
# @e2's drill-down refs and the skeleton refs are preserved

# 5. Drill into another region as needed — refs accumulate
agent-desktop snapshot --root @e1 --snapshot <snapshot_id> -i --compact
# Now you have refs from skeleton + @e2 drill + @e3 drill + @e1 drill
```

**Key behaviors:**
- `--skeleton` clamps depth to min(max_depth, 3) automatically
- Named/described containers at the boundary get refs as drill-down targets
- `--root @ref` merges new refs into the existing refmap
- Re-drilling the same root replaces only that root's subtree refs
- Interactive elements (buttons, textfields) within skeleton depth still get normal refs

## Pattern: Task in a Chromium App (Two Tools, Hand in Hand)

Task: open Slack and send a message to a person. Slack is Chromium-based, so this pattern hands the web-content step to a CDP client — any framework that speaks CDP works, agent-browser preferred — while agent-desktop keeps every native surface.

```bash
# 1. Launch normally first — no --cdp yet
agent-desktop launch "Slack"
# Response includes: "renderer": "chromium", "suggestion": "For web-content work: close-app, ..."
# The suggestion is a fact to read, not an order — the accessibility path still works here.

# 2. Decide the task needs web-content work (composing and sending a message
#    lives in Slack's web contents, which are dense through the accessibility tree).
#    A fresh launch is required for --cdp, so close the app first.
agent-desktop close-app "Slack"
agent-desktop wait --event app-terminated --app "Slack" --timeout 10000

# 3. Relaunch with a verified CDP port
agent-desktop launch "Slack" --cdp
# Response: data.cdp.port = 9231

# 4. Hand off to a CDP client — agent-browser preferred, any CDP client works
command -v agent-browser
# If present:
agent-browser connect 9231
# Then its normal workflow: snapshot, find the person, click, type the message, send.
# agent-browser skills get electron   # Electron-specific guidance if needed
# If absent, connect with Playwright, Puppeteer, chrome-remote-interface, or another CDP client instead.

# 5. Anything native happens on agent-desktop meanwhile, over the same launch —
#    a menu bar item, a file-attach dialog, or a delivered notification:
agent-desktop snapshot --app "Slack" --surface menubar -i
agent-desktop --headed list-notifications --app "Slack"

# 6. Cleanup: close-app ends the port along with the app when the task is done
agent-desktop close-app "Slack"
```

**Fallback when no CDP client is installed:** ask the user to run `npm install -g agent-browser`, or skip `--cdp` and keep working through agent-desktop's accessibility path — `snapshot --skeleton --app "Slack" -i --compact` and drill into the message composer the same way as any other dense app. Accessibility always works; `--cdp` is the opt-in fast path when a fresh launch is acceptable and a CDP client is available.

## Pattern: Fill a Form

```bash
# For simple apps, full snapshot is fine
agent-desktop snapshot --app "System Settings" -i
# Keep snapshot_id = <snapshot_id>

# For dense apps, use skeleton first to find the form region, then drill
# agent-desktop snapshot --skeleton --app "System Settings" -i --compact
# agent-desktop snapshot --root @e5 --snapshot <snapshot_id> -i --compact

# Found: @e3 = "Computer Name" textfield, @e5 = "Local Hostname" textfield

# Clear and fill each field
agent-desktop clear @e3 --snapshot <snapshot_id>
agent-desktop type @e3 --snapshot <snapshot_id> "My MacBook Pro"
agent-desktop clear @e5 --snapshot <snapshot_id>
agent-desktop type @e5 --snapshot <snapshot_id> "my-macbook-pro"

# Click the save/apply button
agent-desktop click @e8 --snapshot <snapshot_id>

# Verify success — re-snapshot or re-drill
agent-desktop snapshot --app "System Settings" -i
```

## Pattern: Navigate Menus

```bash
# 1. Click the menu item
agent-desktop snapshot --app "TextEdit" --surface menubar -i
# Found: @e1 = "File" menuitem

agent-desktop click @e1 --snapshot <snapshot_id>
agent-desktop wait --menu --app "TextEdit"
agent-desktop snapshot --app "TextEdit" --surface menu -i
# Found: @e5 = "Save As..." menuitem

agent-desktop click @e5 --snapshot <snapshot_id>

# 2. Wait for the dialog, then snapshot the SHEET surface (not the full window)
agent-desktop wait --window "Save"
agent-desktop snapshot --app "TextEdit" --surface sheet -i
```

## Pattern: Right-Click Context Menu

```bash
# 1. Right-click the target. On macOS, APP_UNRESPONSIVE can mean AXShowMenu entered modal tracking after delivery; inspect the effect before retrying.
agent-desktop right-click @s8f3k2p9:e3

# 2. Use the returned menu tree, or snapshot the menu surface if you need a fresh read.
agent-desktop snapshot --app "Finder" --surface menu -i

# 3. Click the desired menu item
agent-desktop click @e7 --snapshot <snapshot_id>

# 4. Wait for menu to close
agent-desktop wait --menu-closed --app "Finder" --timeout 2000
```

## Pattern: Handle a Dialog

```bash
# After triggering a dialog (save, alert, confirmation):
agent-desktop wait --window "Save As" --timeout 5000

# Snapshot the SURFACE, not the full window — only overlay refs matter
agent-desktop snapshot --app "TextEdit" --surface sheet -i
# For alerts: --surface alert | For popovers: --surface popover

# Fill dialog fields
agent-desktop type @e2 --snapshot <snapshot_id> "my-document.txt"

# Click OK/Save
agent-desktop click @e5 --snapshot <snapshot_id>

# After dialog closes, snapshot the window again for fresh refs
agent-desktop snapshot --app "TextEdit" -i
```

## Pattern: Scroll and Find

When the target element isn't visible and you need to scroll to find it:

```bash
# 1. Use skeleton to find the scrollable region
agent-desktop snapshot --skeleton --app "App" -i --compact
# Found: @e2 = group "Content" (children_count: 200)

# 2. Drill into the region to get a scroll area ref
agent-desktop snapshot --root @e2 --snapshot <snapshot_id> -i --compact
# Found: @e8 = scroll area

# 3. Scroll and search in a loop
agent-desktop scroll @e8 --snapshot <snapshot_id> --direction down --amount 5
agent-desktop find --app "App" --name "Target Item"
# If no matches, scroll again
agent-desktop scroll @e8 --snapshot <snapshot_id> --direction down --amount 5
agent-desktop find --app "App" --name "Target Item"
# Found: @e14 = "Target Item"
agent-desktop click @e14 --snapshot <snapshot_id>
```

## Pattern: Tab Through Fields

```bash
# For sequential form filling without needing refs for each field:
agent-desktop focus @s8f3k2p9:e1          # Explicit focus change
agent-desktop type @s8f3k2p9:e1 "value1"
agent-desktop press tab
# Now in next field — type directly since focus moved
agent-desktop press tab          # Skip a field
agent-desktop type @s8f3k2p9:e3 "value3"  # Or snapshot again to get new refs
```

## Pattern: Copy Text from Element

```bash
# Option A: Read directly via accessibility
agent-desktop get @s8f3k2p9:e5 --property value

# Option B: Copy via keyboard
agent-desktop focus @s8f3k2p9:e5
agent-desktop press cmd+a
agent-desktop press cmd+c
agent-desktop clipboard-get
```

## Pattern: Drag and Drop

```bash
# Between elements (by ref)
agent-desktop --headed drag --from @s8f3k2p9:e3 --to @s8f3k2p9:e8

# Between coordinates
agent-desktop --headed drag --from-xy 100,200 --to-xy 500,400

# Mixed: element to coordinates
agent-desktop --headed drag --from @s8f3k2p9:e3 --to-xy 500,400 --duration 500
```

## Pattern: Wait for Async UI

```bash
# After triggering a long operation:
agent-desktop click @e5 --snapshot <snapshot_id>  # "Download" button

# Wait for completion text
agent-desktop wait --text "Download complete" --app "App" --timeout 30000

# Or wait for a specific element to appear
agent-desktop wait --element @e10 --snapshot <snapshot_id> --timeout 10000
```

## Pattern: Launch, Automate, Close

```bash
# Full lifecycle
agent-desktop launch "Calculator"
# Read data.window: present means the app already drew one. If it is absent and
# you need a window, launch --activate or wait --event window-opened.
# Simple app → full snapshot is fine
agent-desktop snapshot --app "Calculator" -i

# Dense app → skeleton first
# agent-desktop launch "Slack"
# agent-desktop snapshot --skeleton --app "Slack" -i --compact
# agent-desktop snapshot --root @e2 --snapshot <snapshot_id> -i --compact

# ... perform automation ...

agent-desktop close-app "Calculator"
```

## Pattern: Launch, Automate, Close (Windows)

Windows-specific notes: `launch` resolves an absolute path or a bare name
found under System32 or the Windows directory — not display names; dangerous
shortcuts are refused without `--force`; notifications do not exist on the
Windows adapter.

```bash
# 1. Launch by system-directory bare name and wait for a window
agent-desktop launch "notepad.exe"
agent-desktop wait --event window-opened --app "notepad" --timeout 10000

# 2. Observe, then act through refs - semantic and headless
agent-desktop snapshot --skeleton --app "notepad" -i --compact
agent-desktop set-value @s8f3k2p9:e2 --snapshot <snapshot_id> "typed through UIA"

# 3. Physical input is explicit: focus is headed-required, then press works
agent-desktop --headed focus @s8f3k2p9:e2 --snapshot <snapshot_id>
agent-desktop press ctrl+s --app "notepad"

# 4. Close cleanly instead of alt+f4 (blocked without --force)
agent-desktop press alt+f4            # POLICY_DENIED; add --force only deliberately
agent-desktop close-app "notepad"     # success only after verified exit
```

## Pattern: Multi-Window Workflow

```bash
# List windows to find the right one
agent-desktop list-windows --app "Finder"
# Returns: [{id: "w-1234", title: "Documents"}, {id: "w-5678", title: "Downloads"}]

# Focus a specific window
agent-desktop focus-window --window-id "w-5678"

# Snapshot that specific window
agent-desktop snapshot --app "Finder" --window-id "w-5678" -i
```

## Pattern: Check Before Act (Idempotent)

```bash
# Check if already in desired state
agent-desktop is @s8f3k2p9:e6 --property checked
# If result is false, then check it
agent-desktop check @s8f3k2p9:e6

# Or use check/uncheck directly (they're idempotent)
agent-desktop check @s8f3k2p9:e6    # No-op if already checked
agent-desktop uncheck @s8f3k2p9:e6  # No-op if already unchecked
```

## Pattern: Batch Operations

```bash
# Run multiple commands sequentially in one process; this is not a transaction
agent-desktop batch '[
  {"command":"click","args":{"ref_id":"@e1","snapshot":"<snapshot_id>"}},
  {"command":"wait","args":{"ms":200}},
  {"command":"type","args":{"ref_id":"@e2","snapshot":"<snapshot_id>","text":"hello"}},
  {"command":"press","args":{"combo":"return"}}
]' --stop-on-error
```

## Anti-Patterns to Avoid

1. **Full snapshot on dense apps.** Use `--skeleton` + `--root` for Electron apps (Slack, VS Code, Discord). Full snapshot wastes 4-25x more tokens.
2. **Acting without observing.** Never click a ref without a recent snapshot or drill-down.
3. **Hardcoding refs.** Refs change between snapshots. Always use fresh refs.
4. **Ignoring wait.** After launch, dialog triggers, or menu clicks — always wait before snapshotting.
5. **Using coordinates when refs exist.** AX-based actions are more reliable than coordinate clicks.
6. **Not checking permissions.** Always verify accessibility permission before starting automation.
7. **Assuming UI stability.** Re-drill the affected region after every action that could change the UI.
8. **Snapshotting the full window when an overlay is open.** Use `--surface sheet/alert/popover/menu` instead. Never `--skeleton` for surfaces — they're already focused.
9. **Re-snapshotting everything after one action.** Use scoped re-drill (`--root @ref`) to refresh only the affected region. Other refs stay valid.
10. **Assuming headed and headless have the same side effects.** Headless ref actions block implicit focus/cursor input. Headed ref actions intentionally focus the exact source window when required, and headed pointer actions may move the cursor; raw coordinates never infer focus.
11. **Hand-rolling raw CDP for a Chromium app.** After `launch --cdp`, connect a real CDP client — `agent-browser connect <port>` (preferred), or Playwright, Puppeteer, `chrome-remote-interface`, and similar — never a hand-written WebSocket client, `Runtime.evaluate` DOM edits, or app-internal APIs (e.g. an app's own JS globals). Those bypass real input, verify nothing, and are app-specific; they break silently on the next app update. If no CDP client is available, ask the user to install agent-browser or stay on accessibility commands.
