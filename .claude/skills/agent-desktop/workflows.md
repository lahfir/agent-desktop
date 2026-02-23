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

## Pattern: Fill a Form

```bash
# 1. Snapshot the form
agent-desktop snapshot --app "System Settings" -i

# 2. Parse output, identify text fields by name
# Found: @e3 = "Computer Name" textfield, @e5 = "Local Hostname" textfield

# 3. Clear and fill each field
agent-desktop clear @e3
agent-desktop type @e3 "My MacBook Pro"
agent-desktop clear @e5
agent-desktop type @e5 "my-macbook-pro"

# 4. Click the save/apply button
agent-desktop click @e8

# 5. Verify success
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

# 2. Wait for the dialog
agent-desktop wait --window "Save"
agent-desktop snapshot --app "TextEdit" -i
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
agent-desktop snapshot --app "TextEdit" -i

# Fill dialog fields
agent-desktop type @e2 "my-document.txt"

# Click OK/Save
agent-desktop click @e5
```

## Pattern: Scroll and Find

When the target element isn't visible and you need to scroll to find it:

```bash
# 1. Find the scroll area
agent-desktop snapshot --app "App" -i
# Found: @e1 = scroll area

# 2. Scroll and search in a loop
agent-desktop scroll @e1 --direction down --amount 5
agent-desktop find --app "App" --name "Target Item"
# If no matches, scroll again
agent-desktop scroll @e1 --direction down --amount 5
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
agent-desktop snapshot --app "Calculator" -i

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

1. **Acting without observing.** Never click a ref without a recent snapshot.
2. **Hardcoding refs.** Refs change between snapshots. Always use fresh refs.
3. **Ignoring wait.** After launch, dialog triggers, or menu clicks — always wait before snapshotting.
4. **Using coordinates when refs exist.** AX-based actions are more reliable than coordinate clicks.
5. **Not checking permissions.** Always verify accessibility permission before starting automation.
6. **Deep snapshots of large apps.** Use `--max-depth 5` and `-i` for Xcode, VS Code, etc.
7. **Assuming UI stability.** Re-snapshot after every action that could change the UI.
