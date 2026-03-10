# Common Automation Workflows

Patterns for using agent-desktop effectively in multi-step desktop automation tasks.

## First-Time Setup

Before any automation, verify permissions:

```bash
agent-desktop permissions
# If PERM_DENIED:
agent-desktop permissions --request
# Then: System Settings > Privacy & Security > Accessibility > enable your terminal
```

## Pattern: Progressive Skeleton Traversal (Default for Dense Apps)

The recommended approach for Electron apps (Slack, VS Code, Discord) and any app with 50+ interactive elements. Reduces token consumption 78-96%.

```bash
# 1. Get skeleton overview — shallow 3-level map with children_count hints
agent-desktop snapshot --skeleton --app "Slack" -i --compact
# Output shows regions like:
#   @e1 = group "Workspaces" (children_count: 4)
#   @e2 = group "Channels" (children_count: 42)
#   @e3 = group "Messages" (children_count: 156)
#   @e4 = button "New Message"    ← interactive elements at top levels still get refs

# 2. Identify the region you need and drill into it
agent-desktop snapshot --root @e2 -i --compact
# Now you see all 42 children inside "Channels" with full refs

# 3. Act on an element found in the drill-down
agent-desktop click @e18  # Click "general" channel

# 4. Re-drill the same or a different region to verify / continue
agent-desktop snapshot --root @e3 -i --compact
# Scoped invalidation: only @e3's previous refs are replaced
# @e2's drill-down refs and the skeleton refs are preserved

# 5. Drill into another region as needed — refs accumulate
agent-desktop snapshot --root @e1 -i --compact
# Now you have refs from skeleton + @e2 drill + @e3 drill + @e1 drill
```

**Key behaviors:**
- `--skeleton` clamps depth to min(max_depth, 3) automatically
- Named/described containers at the boundary get refs as drill-down targets
- `--root @ref` merges new refs into the existing refmap
- Re-drilling the same root replaces only that root's subtree refs
- Interactive elements (buttons, textfields) within skeleton depth still get normal refs

## Pattern: Fill a Form

```bash
# For simple apps, full snapshot is fine
agent-desktop snapshot --app "System Settings" -i

# For dense apps, use skeleton first to find the form region, then drill
# agent-desktop snapshot --skeleton --app "System Settings" -i --compact
# agent-desktop snapshot --root @e5 -i --compact

# Found: @e3 = "Computer Name" textfield, @e5 = "Local Hostname" textfield

# Clear and fill each field
agent-desktop clear @e3
agent-desktop type @e3 "My MacBook Pro"
agent-desktop clear @e5
agent-desktop type @e5 "my-macbook-pro"

# Click the save/apply button
agent-desktop click @e8

# Verify success — re-snapshot or re-drill
agent-desktop snapshot --app "System Settings" -i
```

## Pattern: Navigate Menus

```bash
# 1. Click the menu item
agent-desktop snapshot --app "TextEdit" --surface menubar -i
# Found: @e1 = "File" menuitem

agent-desktop click @e1
agent-desktop wait --menu --app "TextEdit"
agent-desktop snapshot --app "TextEdit" --surface menu -i
# Found: @e5 = "Save As..." menuitem

agent-desktop click @e5

# 2. Wait for the dialog, then snapshot the SHEET surface (not the full window)
agent-desktop wait --window "Save"
agent-desktop snapshot --app "TextEdit" --surface sheet -i
```

## Pattern: Right-Click Context Menu

```bash
# 1. Right-click the target element
agent-desktop right-click @e3

# 2. Wait for context menu to appear
agent-desktop wait --menu --app "Finder" --timeout 3000

# 3. Snapshot the menu surface
agent-desktop snapshot --app "Finder" --surface menu -i

# 4. Click the desired menu item
agent-desktop click @e7

# 5. Wait for menu to close
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
agent-desktop type @e2 "my-document.txt"

# Click OK/Save
agent-desktop click @e5

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
agent-desktop snapshot --root @e2 -i --compact
# Found: @e8 = scroll area

# 3. Scroll and search in a loop
agent-desktop scroll @e8 --direction down --amount 5
agent-desktop find --app "App" --name "Target Item"
# If no matches, scroll again
agent-desktop scroll @e8 --direction down --amount 5
agent-desktop find --app "App" --name "Target Item"
# Found: @e14 = "Target Item"
agent-desktop click @e14
```

## Pattern: Tab Through Fields

```bash
# For sequential form filling without needing refs for each field:
agent-desktop click @e1          # Focus first field
agent-desktop type @e1 "value1"
agent-desktop press tab
# Now in next field — type directly since focus moved
agent-desktop press tab          # Skip a field
agent-desktop type @e3 "value3"  # Or snapshot again to get new refs
```

## Pattern: Copy Text from Element

```bash
# Option A: Read directly via accessibility
agent-desktop get @e5 --property value

# Option B: Copy via keyboard
agent-desktop click @e5
agent-desktop press cmd+a
agent-desktop press cmd+c
agent-desktop clipboard-get
```

## Pattern: Drag and Drop

```bash
# Between elements (by ref)
agent-desktop drag --from @e3 --to @e8

# Between coordinates
agent-desktop drag --from-xy 100,200 --to-xy 500,400

# Mixed: element to coordinates
agent-desktop drag --from @e3 --to-xy 500,400 --duration 500
```

## Pattern: Wait for Async UI

```bash
# After triggering a long operation:
agent-desktop click @e5  # "Download" button

# Wait for completion text
agent-desktop wait --text "Download complete" --app "App" --timeout 30000

# Or wait for a specific element to appear
agent-desktop wait --element @e10 --timeout 10000
```

## Pattern: Launch, Automate, Close

```bash
# Full lifecycle
agent-desktop launch "Calculator"
# Simple app → full snapshot is fine
agent-desktop snapshot --app "Calculator" -i

# Dense app → skeleton first
# agent-desktop launch "Slack"
# agent-desktop snapshot --skeleton --app "Slack" -i --compact
# agent-desktop snapshot --root @e2 -i --compact

# ... perform automation ...

agent-desktop close-app "Calculator"
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
agent-desktop is @e6 --property checked
# If result is false, then check it
agent-desktop check @e6

# Or use check/uncheck directly (they're idempotent)
agent-desktop check @e6    # No-op if already checked
agent-desktop uncheck @e6  # No-op if already unchecked
```

## Pattern: Batch Operations

```bash
# Run multiple commands atomically
agent-desktop batch '[
  {"command":"click","args":{"ref_id":"@e1"}},
  {"command":"wait","args":{"ms":200}},
  {"command":"type","args":{"ref_id":"@e2","text":"hello"}},
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
