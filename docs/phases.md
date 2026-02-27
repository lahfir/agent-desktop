# agent-desktop — Phase Roadmap

> Source of truth for the phased delivery plan. Derived from [PRD v2.0](./agent_desktop_prd_v2.pdf) and the [Skill Maintenance Addendum](./prd-addendum-skill-maintenance.md).

---

## Phase Overview

| Phase | Name | Status | Platforms |
|-------|------|--------|-----------|
| 1 | Foundation + macOS MVP | **Completed** | macOS |
| 2 | Windows Adapter | Planned | macOS, Windows |
| 3 | Linux Adapter | Planned | macOS, Windows, Linux |
| 4 | MCP Server Mode | Planned | All |
| 5 | Production Hardening | Planned | All |

Each phase is strictly additive. Core engine, CLI parser, JSON contract, error types, snapshot engine, and command registry are never modified — only new `PlatformAdapter` implementations, new transports, and new modes are added.

---

## Phase 1 — Foundation + macOS MVP

**Status: Completed**

Phase 1 is the load-bearing phase. It establishes every shared abstraction, every trait boundary, every output contract, every error type, the complete command trait and registry, and the full workspace structure. All subsequent phases build on top of this foundation without modifying core.

### Objectives

| ID | Objective | Success Metric |
|----|-----------|----------------|
| P1-O1 | Working macOS snapshot CLI | `snapshot --app Finder` returns valid JSON with refs for all interactive elements |
| P1-O2 | Platform adapter trait | Trait compiles with mock adapter; macOS adapter satisfies all trait methods |
| P1-O3 | Ref-based interaction | `click @e3` successfully invokes AXPress on the resolved element |
| P1-O4 | Context efficiency | Typical Finder snapshot < 500 tokens (measured via tiktoken) |
| P1-O5 | Typed JSON contract | Output validates against JSON Schema; schema is versioned |
| P1-O6 | Permission detection | Missing Accessibility permission prints specific macOS setup instructions |
| P1-O7 | Command extensibility | Adding a new command requires exactly 1 new file + 2 registration lines |
| P1-O8 | 50 working commands | All commands pass integration tests |
| P1-O9 | CI pipeline | GitHub Actions macOS runner executes full test suite on every PR |

### Workspace Structure

```
agent-desktop/
├── Cargo.toml              # workspace: members, shared deps
├── rust-toolchain.toml     # pinned Rust version
├── clippy.toml             # project-wide lint config
├── schemas/                # JSON Schema files for output validation
│   ├── snapshot_response.json
│   ├── action_response.json
│   └── error_response.json
├── crates/
│   ├── core/               # agent-desktop-core (platform-agnostic)
│   │   └── src/
│   │       ├── lib.rs          # public re-exports only
│   │       ├── node.rs         # AccessibilityNode, Rect, WindowInfo
│   │       ├── adapter.rs      # PlatformAdapter trait
│   │       ├── action.rs       # Action enum, ActionResult, InputEvent, WindowOp
│   │       ├── refs.rs         # RefAllocator, RefMap, RefEntry
│   │       ├── snapshot.rs     # SnapshotEngine (filter, allocate, serialize)
│   │       ├── error.rs        # ErrorCode enum, AdapterError, AppError
│   │       ├── output.rs       # Response envelope, JSON formatting
│   │       ├── command.rs      # Command trait + CommandRegistry
│   │       └── commands/       # one file per command
│   ├── macos/              # agent-desktop-macos (Phase 1)
│   ├── windows/            # agent-desktop-windows (stub → Phase 2)
│   └── linux/              # agent-desktop-linux (stub → Phase 3)
├── src/                    # agent-desktop binary (entry point)
│   ├── main.rs
│   ├── cli.rs
│   ├── cli_args.rs
│   ├── dispatch.rs
│   └── batch_dispatch.rs
└── tests/
    ├── fixtures/
    └── integration/
```

### PlatformAdapter Trait

The single most important abstraction. Every platform-specific operation goes through this trait. Core never imports platform crates. 12 methods with default implementations returning `not_supported()`:

```rust
pub trait PlatformAdapter: Send + Sync {
    fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>>;
    fn list_apps(&self) -> Result<Vec<AppInfo>>;
    fn get_tree(&self, win: &WindowInfo, opts: &TreeOptions) -> Result<AccessibilityNode>;
    fn execute_action(&self, handle: &NativeHandle, action: Action) -> Result<ActionResult>;
    fn resolve_element(&self, entry: &RefEntry) -> Result<NativeHandle>;
    fn check_permissions(&self) -> PermissionStatus;
    fn focus_window(&self, win: &WindowInfo) -> Result<()>;
    fn launch_app(&self, id: &str, wait: bool) -> Result<WindowInfo>;
    fn close_app(&self, id: &str, force: bool) -> Result<()>;
    fn screenshot(&self, target: ScreenshotTarget) -> Result<ImageBuffer>;
    fn get_clipboard(&self) -> Result<String>;
    fn set_clipboard(&self, text: &str) -> Result<()>;
    fn synthesize_input(&self, input: InputEvent) -> Result<()>;
    fn manage_window(&self, win: &WindowInfo, op: WindowOp) -> Result<()>;
    fn list_notifications(&self) -> Result<Vec<NotificationInfo>>;
    fn dismiss_notification(&self, id: &str) -> Result<()>;
    fn interact_notification(&self, id: &str, action_name: &str) -> Result<ActionResult>;
    fn list_tray_items(&self) -> Result<Vec<TrayItemInfo>>;
    fn interact_tray_item(&self, id: &str, action: Action) -> Result<ActionResult>;
}
```

### Key Supporting Types

- `Action` — Click, DoubleClick, RightClick, SetValue(String), SetFocus, Expand, Collapse, Select(String), Toggle, Scroll(Direction, Amount), PressKey(KeyCombo)
- `InputEvent` — MouseMove(x,y), MouseClick(x,y,button,count), MouseDown(button), MouseUp(button), MouseWheel(dy,dx), KeyDown(key), KeyUp(key), Drag(from,to)
- `WindowOp` — Resize(w,h), Move(x,y), Minimize, Maximize, Restore, Close
- `ScreenshotTarget` — Screen(index), Window(id), Element(NativeHandle), FullScreen
- `NotificationInfo` — id, app_name, title, body, timestamp, actions: Vec\<String\>, is_persistent
- `TrayItemInfo` — id, app_name, title, tooltip, has_menu

### macOS Adapter Implementation

Located in `crates/macos/src/` following the platform crate folder structure:

```
crates/macos/src/
├── lib.rs              # mod declarations + re-exports only
├── adapter.rs          # MacOSAdapter: PlatformAdapter impl
├── tree/
│   ├── mod.rs          # re-exports
│   ├── element.rs      # AXElement struct + attribute readers
│   ├── builder.rs      # build_subtree, tree traversal
│   ├── roles.rs        # AXRole string → unified role enum mapping
│   ├── resolve.rs      # Element re-identification for ref resolution
│   └── surfaces.rs     # Surface detection (menu, sheet, alert, popover)
├── actions/
│   ├── mod.rs          # re-exports
│   ├── dispatch.rs     # perform_action match arms
│   ├── activate.rs     # Smart AX-first activation chain (15-step)
│   └── extras.rs       # select_value, ax_scroll
├── input/
│   ├── mod.rs          # re-exports
│   ├── keyboard.rs     # CGEventCreateKeyboardEvent, key synthesis, text typing
│   ├── mouse.rs        # CGEventCreateMouseEvent, mouse events
│   └── clipboard.rs    # NSPasteboard.generalPasteboard read/write
├── notifications/
│   ├── mod.rs          # re-exports
│   ├── list.rs         # List notifications via Notification Center AX tree
│   ├── dismiss.rs      # Dismiss individual or all notifications via AXPress
│   └── interact.rs     # Click notification action buttons
├── tray/
│   ├── mod.rs          # re-exports
│   ├── list.rs         # List menu bar extras via SystemUIServer AX tree
│   └── interact.rs     # Click menu bar extras, expand menus
└── system/
    ├── mod.rs          # re-exports
    ├── app_ops.rs      # launch, close, focus via NSWorkspace / AppleScript
    ├── window_ops.rs   # window resize, move, minimize, maximize, restore
    ├── key_dispatch.rs # app-targeted key press
    ├── permissions.rs  # AXIsProcessTrusted(), AXIsProcessTrustedWithOptions(prompt: true)
    ├── screenshot.rs   # CGWindowListCreateImage
    └── wait.rs         # wait utilities
```

**Tree traversal:**
- Entry: `AXUIElementCreateApplication(pid)` for app root
- Children: `kAXChildrenAttribute` recursively with ancestor-path set (not global visited set — macOS reuses AXUIElementRef pointers across sibling branches)
- Batch fetch: `AXUIElementCopyMultipleAttributeValues` for 3-5x faster attribute reads
- Role mapping: AXRole strings → unified role enum in `tree/roles.rs`
- Max depth default: 10, configurable via `--max-depth`
- Name: `kAXTitleAttribute` / `kAXDescriptionAttribute`. Value: `kAXValueAttribute`
- Bounds: `kAXPositionAttribute` + `kAXSizeAttribute` combined to Rect

**Action execution:**
- Click: `AXUIElementPerformAction(kAXPressAction)`
- SetValue: `AXUIElementSetAttributeValue(kAXValueAttribute, value)`
- SetFocus: `AXUIElementSetAttributeValue(kAXFocusedAttribute, true)`
- Expand/Collapse: Toggle `kAXExpandedAttribute`
- Select: `AXUIElementSetAttributeValue(kAXSelectedAttribute, true)` on child
- Keyboard/Mouse: `CGEventCreateKeyboardEvent` / `CGEventCreateMouseEvent` via CoreGraphics
- Clipboard: `NSPasteboard.generalPasteboard` read/write via Cocoa FFI
- Screenshot: `CGWindowListCreateImage` for window-specific or full-screen capture

**Permission detection:**
- Call `AXIsProcessTrusted()` on startup
- If false, return `PERM_DENIED` with guidance: "Open System Settings > Privacy > Accessibility and add your terminal"
- Optionally call `AXIsProcessTrustedWithOptions(prompt: true)` to trigger system dialog

**Notification management:**
- Open Notification Center via AX: target the `NotificationCenter` process (bundleId: `com.apple.notificationcenterui`)
- List notifications: traverse the Notification Center AX tree — each notification is an `AXGroup` with title, subtitle, and action buttons
- Dismiss: perform `AXPress` on the notification's close button, or `AXRemoveFromParent` if supported
- Interact: resolve action buttons within a notification group and perform `AXPress`
- Dismiss all: `AXPress` the "Clear All" button at the group level
- Do Not Disturb detection: read Focus/DND state via `NSDoNotDisturbEnabled` user defaults or `CoreFoundation` preferences

**System tray / Menu bar extras:**
- Menu bar extras (status items) live under the `SystemUIServer` process AX tree
- List items: traverse `AXMenuBarItem` children of the system menu bar
- Click: `AXPress` on the target menu bar extra element
- Expand menus: after clicking a tray item, traverse the resulting `AXMenu` as a surface
- Control Center items: accessible via the `ControlCenter` process (bundleId: `com.apple.controlcenter`)

**AXElement safety:**
- Inner field: `pub(crate)` not `pub` (prevents double-free via raw pointer extraction)
- `Clone` impl must call `CFRetain`
- `Drop` impl must call `CFRelease`

### Snapshot Engine and Ref Allocator

Platform-agnostic, lives in `agent-desktop-core`:

1. Raw tree: Call `adapter.get_tree(window, opts)`
2. Filter: Remove invisible/offscreen. Remove empty groups with no interactive descendants. Prune beyond max_depth
3. Allocate refs: Depth-first. Interactive roles get `@e1`, `@e2`, etc. Store in RefMap
4. Serialize: Omit null fields. Omit empty arrays. Omit bounds in compact mode
5. Estimate tokens: Optionally warn if exceeding threshold

RefMap persisted at `~/.agent-desktop/last_refmap.json` with `0o600` permissions, directory at `0o700`. Each snapshot replaces the refmap file entirely (atomic write via temp + rename). Action commands use optimistic re-identification: `(pid, role, name, bounds_hash)`. Return `STALE_REF` on mismatch.

### New Commands — Notification & System Tray (Post Phase 1)

> **Note:** Notification management and system tray interaction were not part of the original Phase 1 delivery. These are **new features to be implemented across all platforms** as each platform adapter is built. The macOS implementations were added as a follow-up to Phase 1. Windows (Phase 2) and Linux (Phase 3) implementations follow the same pattern.

#### Notification Commands (macOS — Completed)

| Command | Description | Flags | Status |
|---------|-------------|-------|--------|
| `list-notifications` | List current notifications with app, title, body, and available actions | `--app` (filter by app), `--text` (filter by text), `--limit` (max results) | **Completed** |
| `dismiss-notification` | Dismiss a specific notification by 1-based index | `<index>`, `--app` (filter by app) | **Completed** |
| `dismiss-all-notifications` | Clear all notifications, optionally filtered by app (single NC session, reports failures) | `--app` (filter by app) | **Completed** |
| `notification-action` | Click an action button on a specific notification | `<index> <action-name>` | **Completed** |

#### System Tray / Status Area Commands (New — Not Yet Implemented)

| Command | Description | Flags |
|---------|-------------|-------|
| `list-tray-items` | List all system tray / menu bar extra items with app name and tooltip | — |
| `click-tray-item` | Click a system tray item by ID or app name | `<tray-item-id>` |
| `open-tray-menu` | Click a tray item and snapshot its resulting menu for ref-based interaction | `<tray-item-id>` |

#### Wait Command Update (Notification — Completed, Menu — Completed)

The `wait` command has been extended with notification and menu support:
- `wait --notification` — Wait for any new notification to appear (index-diff based detection)
- `wait --notification --app Safari` — Wait for a notification from a specific app
- `wait --notification --text "Download complete"` — Wait for a notification containing specific text
- `wait --menu` / `wait --menu-closed` — Wait for context menu open/close

### Commands Shipped (57)

| Category | Commands | Count |
|----------|----------|-------|
| App / Window | `launch`, `close-app`, `list-windows`, `list-apps`, `focus-window`, `resize-window`, `move-window`, `minimize`, `maximize`, `restore` | 10 |
| Observation | `snapshot`, `screenshot`, `find`, `get` (text, value, title, bounds, role, states, tree-stats), `is` (visible, enabled, checked, focused, expanded), `list-surfaces` | 6 |
| Interaction | `click`, `double-click`, `triple-click`, `right-click`, `type`, `set-value`, `clear`, `focus`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse` | 14 |
| Scroll | `scroll`, `scroll-to` | 2 |
| Keyboard | `press`, `key-down`, `key-up` | 3 |
| Mouse | `hover`, `drag`, `mouse-move`, `mouse-click`, `mouse-down`, `mouse-up` | 6 |
| Clipboard | `clipboard-get`, `clipboard-set`, `clipboard-clear` | 3 |
| Notification | `list-notifications`, `dismiss-notification`, `dismiss-all-notifications`, `notification-action` | 4 |
| System Tray | `list-tray-items`, `click-tray-item`, `open-tray-menu` | 3 |
| Wait | `wait` (with `--element`, `--window`, `--text`, `--menu`, `--notification` flags) | 1 |
| System | `status`, `permissions`, `version` | 3 |
| Batch | `batch` | 1 |

### JSON Output Contract

All commands produce a response envelope. Schema files versioned in `schemas/`.

Success:
```json
{
  "version": "1.0",
  "ok": true,
  "command": "snapshot",
  "data": {
    "app": "Finder",
    "window": { "id": "w-4521", "title": "Documents" },
    "ref_count": 14,
    "tree": { ... }
  }
}
```

Error:
```json
{
  "version": "1.0",
  "ok": false,
  "command": "click",
  "error": {
    "code": "STALE_REF",
    "message": "RefMap is from a previous snapshot",
    "suggestion": "Run 'snapshot' to refresh, then retry with updated ref"
  }
}
```

Serialization rules: omit null/None fields (`skip_serializing_if`), omit empty arrays, omit bounds in compact mode, `ref_count` and `tree` inside `data`.

### Error Taxonomy

| Code | Category | Example | Recovery Suggestion |
|------|----------|---------|---------------------|
| `PERM_DENIED` | Permission | Accessibility not granted | Open System Settings > Privacy > Accessibility and add your terminal |
| `ELEMENT_NOT_FOUND` | Ref | @e12 not in current RefMap | Run 'snapshot' to refresh, then retry with updated ref |
| `APP_NOT_FOUND` | Application | --app 'Photoshop' not running | Launch the application first with 'launch Photoshop' |
| `ACTION_FAILED` | Execution | AXPress returned error on disabled button | Element may be disabled. Check states before acting |
| `ACTION_NOT_SUPPORTED` | Execution | Expand on a button element | This element does not support the requested action |
| `TREE_TIMEOUT` | Performance | Traversal exceeded 5s | Try --max-depth 3 or target a specific window |
| `STALE_REF` | Ref | RefMap is from a previous snapshot | UI may have changed. Run 'snapshot' again |
| `WINDOW_NOT_FOUND` | Window | --window w-999 does not exist | Run 'list-windows' to see available windows |
| `PLATFORM_UNSUPPORTED` | Platform | Linux adapter not yet shipped | This platform ships in Phase 3. Currently macOS only |
| `CLIPBOARD_EMPTY` | Clipboard | clipboard get but clipboard is empty | No text content in clipboard. Copy something first |
| `TIMEOUT` | Wait | wait --element exceeded timeout | Element did not appear within timeout. Increase --timeout or check app state |
| `NOTIFICATION_NOT_FOUND` | Notification | Notification ID not found | Notification may have been dismissed or expired. Run 'list-notifications' to see current notifications |
| `NOTIFICATION_UNSUPPORTED` | Notification | Notification daemon does not support listing | This notification daemon does not expose a history API. Consider using 'wait --notification' to catch notifications in real-time |
| `TRAY_NOT_FOUND` | System Tray | Tray item not found | Tray item may have been removed. Run 'list-tray-items' to see current items |
| `TRAY_UNSUPPORTED` | System Tray | No system tray available | System tray not available on this desktop environment. On GNOME, install the AppIndicator extension |

Exit codes: `0` success, `1` structured error (JSON on stdout), `2` argument/parse error.

### Testing

**Unit tests (core):**
- AccessibilityNode ser/de roundtrips
- Ref allocator only assigns interactive roles
- SnapshotEngine filtering
- Error serialization
- JSON schema validation
- MockAdapter: in-memory PlatformAdapter returning hardcoded trees

**Unit tests (macos):**
- Role mapping coverage
- Permission check with mocks
- Tree traversal cycle detection

**Integration tests (macOS CI):**
- Snapshot Finder, TextEdit, System Settings — non-empty trees with refs
- Click button in test app — verify action succeeded
- Type text into TextEdit via ref — verify content changed
- Clipboard get/set roundtrip
- Wait for window
- Launch + close app lifecycle
- Permission denied scenario — correct error code and guidance
- Large tree (Xcode) snapshot in under 2 seconds
- List notifications — returns non-empty list when Notification Center has entries
- Dismiss notification — verify notification removed from Notification Center AX tree
- List tray items — returns known menu bar extras (Wi-Fi, Bluetooth, Clock)
- Click tray item — verify menu bar extra menu opens

**Golden fixtures (`tests/fixtures/`):**
- Real snapshots from Finder, TextEdit, etc. checked into repo
- Regression-test serialization format changes

### CI

- GitHub Actions macOS runner on every PR
- `cargo clippy --all-targets -- -D warnings` (zero warnings)
- `cargo test --workspace`
- `cargo tree -p agent-desktop-core` must not contain platform crate names
- Binary size check: fail if release binary exceeds 15MB

### Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `clap` | 4.x | CLI parsing with derive macros |
| `serde` + `serde_json` | 1.x | JSON serialization |
| `thiserror` | 2.x | Error derive macros |
| `tracing` | 0.1+ | Structured logging |
| `base64` | 0.22+ | Screenshot encoding |
| `accessibility-sys` | 0.1+ | macOS AXUIElement FFI |
| `core-foundation` | 0.10+ | macOS CF types |
| `core-graphics` | 0.24+ | macOS CG types |

### Documentation Delivered

- [x] README with installation (npm + source), core workflow, command reference, JSON output, ref system, platform support table
- [x] PRD v2.0
- [x] Architecture diagram
- [x] Claude Code skills: `.claude/skills/agent-desktop/` (core, platform-agnostic) + `.claude/skills/agent-desktop-macos/` (macOS-specific)
- [x] Quick reference slash command: `.claude/commands/desktop.md`

---

## Phase 2 — Windows Adapter

**Status: Planned**

Phase 2 brings agent-desktop to Windows. Core engine, CLI parser, JSON contract, error types, snapshot engine, and command registry are untouched. Only the new `WindowsAdapter` implementation is added inside `crates/windows/`. The existing stub is replaced with a full implementation.

### Objectives

| ID | Objective | Metric |
|----|-----------|--------|
| P2-O1 | Windows adapter | `snapshot` on Windows returns valid tree for Explorer, Notepad, Settings |
| P2-O2 | All existing commands cross-platform | Identical JSON schema output on macOS and Windows for every command |
| P2-O3 | Windows input synthesis | `click`, `type`, `press`, all mouse commands working via UIA + SendInput |
| P2-O4 | Windows screenshot | `screenshot` produces PNG via BitBlt / PrintWindow or `xcap` crate |
| P2-O5 | Windows clipboard | `clipboard-get` / `clipboard-set` / `clipboard-clear` working via Win32 Clipboard API |
| P2-O6 | Windows CI | GitHub Actions Windows runner executes full test suite on every PR |
| P2-O7 | Windows binary release | Prebuilt `.exe` published via GitHub Releases and npm |

### Windows Adapter Implementation

Full `WindowsAdapter` in `crates/windows/src/` following the identical platform crate folder structure:

```
crates/windows/src/
├── lib.rs              # mod declarations + re-exports only
├── adapter.rs          # WindowsAdapter: PlatformAdapter impl
├── tree/
│   ├── mod.rs          # re-exports
│   ├── element.rs      # UIA element wrapper + attribute readers
│   ├── builder.rs      # IUIAutomationTreeWalker traversal with CacheRequest
│   ├── roles.rs        # UIA ControlType → unified role enum mapping
│   ├── resolve.rs      # Element re-identification for ref resolution
│   └── surfaces.rs     # Surface detection (menus, dialogs, flyouts)
├── actions/
│   ├── mod.rs          # re-exports
│   ├── dispatch.rs     # perform_action match arms via UIA patterns
│   ├── activate.rs     # Smart activation chain (InvokePattern → Toggle → coordinate)
│   └── extras.rs       # SelectionPattern, ScrollPattern helpers
├── input/
│   ├── mod.rs          # re-exports
│   ├── keyboard.rs     # SendInput keyboard synthesis
│   ├── mouse.rs        # SendInput mouse events
│   └── clipboard.rs    # OpenClipboard / GetClipboardData / SetClipboardData Win32 APIs
├── notifications/
│   ├── mod.rs          # re-exports
│   ├── list.rs         # List toast/Action Center notifications via UIA
│   ├── dismiss.rs      # Dismiss individual or all notifications
│   └── interact.rs     # Click notification action buttons
├── tray/
│   ├── mod.rs          # re-exports
│   ├── list.rs         # List system tray items via Shell_TrayWnd UIA tree
│   └── interact.rs     # Click tray items, open tray menus
└── system/
    ├── mod.rs          # re-exports
    ├── app_ops.rs      # Process launch via CreateProcess, close via TerminateProcess
    ├── window_ops.rs   # SetWindowPos, ShowWindow for resize/move/minimize/maximize/restore
    ├── key_dispatch.rs # App-targeted key press via SetForegroundWindow + SendInput
    ├── permissions.rs  # COM security check, UAC elevation detection
    ├── screenshot.rs   # BitBlt / PrintWindow or xcap crate
    └── wait.rs         # wait utilities (polling UIA element existence)
```

### Windows API Mapping

| Capability | Technology | Details |
|------------|-----------|---------|
| Tree root | `IUIAutomation.ElementFromHandle()` | Via `uiautomation` crate (v0.24+) wrapping UIA COM APIs via `windows` crate |
| Children | `IUIAutomationTreeWalker.GetFirstChild` / `GetNextSibling` | With `CacheRequest` for batch attribute retrieval (3-5x faster) |
| Role mapping | `UIA ControlType` integers | Map to unified role enum in `tree/roles.rs` — e.g. `UIA_ButtonControlTypeId` → `button` |
| Click | `InvokePattern.Invoke()` | Pattern-based, falls back to `TogglePattern.Toggle()`, then coordinate click via SendInput |
| Set text | `ValuePattern.SetValue()` | Falls back to SelectAll + SendInput keystroke sequence |
| Expand/Collapse | `ExpandCollapsePattern.Expand()` / `.Collapse()` | Native UIA pattern |
| Select | `SelectionItemPattern.Select()` | For combobox, listbox, tab items |
| Toggle | `TogglePattern.Toggle()` | For checkboxes, switches |
| Scroll | `ScrollPattern.Scroll()` / `ScrollPattern.SetScrollPercent()` | Native UIA scroll, falls back to mouse wheel |
| Keyboard | `SendInput` API | `INPUT_KEYBOARD` structs with virtual key codes and scan codes |
| Mouse | `SendInput` API | `INPUT_MOUSE` structs with `MOUSEEVENTF_*` flags |
| Clipboard | `OpenClipboard` / `GetClipboardData` / `SetClipboardData` | Win32 APIs, handle `CF_UNICODETEXT` format |
| Screenshot | `BitBlt` / `PrintWindow` | Window capture; or `xcap` crate for cross-platform consistency |
| App launch | `CreateProcess` / `ShellExecuteEx` | Launch by name or path, wait for main window |
| App close | `WM_CLOSE` / `TerminateProcess` | Graceful close first, force kill with `--force` |
| Window ops | `SetWindowPos` / `ShowWindow` | Resize, move, minimize (`SW_MINIMIZE`), maximize (`SW_MAXIMIZE`), restore (`SW_RESTORE`) |
| Permissions | COM security / UAC | Detect elevation requirements; return `PERM_DENIED` if UIA access blocked |
| Notifications | UIA + Action Center | Toast notifications accessible via UIA tree of `Windows.UI.Notifications.Manager`. List via `IUIAutomationElement` traversal of Action Center pane. Dismiss via `InvokePattern` on close button. Interact via `InvokePattern` on action buttons. Do Not Disturb (Focus Assist) state via `WNF_SHEL_QUIETHOURS_ACTIVE_PROFILE_CHANGED` or registry query |
| System tray | UIA + Shell_TrayWnd | System tray items accessible via UIA tree of `Shell_TrayWnd` class. Overflow items in `NotifyIconOverflowWindow`. List via `IUIAutomationTreeWalker` on tray area. Click via `InvokePattern` or coordinate-based `SendInput`. Expand overflow via click on chevron button |

### Notification Management (New Feature — Windows Implementation)

Windows notification management must be implemented from scratch as part of Phase 2. The macOS notification implementation (completed as a follow-up to Phase 1) serves as the reference pattern — same `PlatformAdapter` trait methods (`list_notifications`, `dismiss_notification`, `dismiss_all_notifications`, `notification_action`), same JSON output contract, same 1-based indexing.

**Implementation approach:**
- **List notifications:** Open Action Center via UIA (`Windows.UI.Notifications`). Traverse the notification list — each toast is a UIA element with `Name` (title), `FullDescription` (body), app info, and child action buttons
- **Dismiss:** `InvokePattern.Invoke()` on the notification's dismiss/close button. For "dismiss all", invoke the "Clear all" button in Action Center
- **Interact with actions:** Resolve action buttons within a toast element tree, invoke via `InvokePattern`
- **Focus Assist / Do Not Disturb:** Query via `WNF_SHEL_QUIETHOURS_ACTIVE_PROFILE_CHANGED` state notification or registry key `HKCU\Software\Microsoft\Windows\CurrentVersion\CloudStore\Store\DefaultAccount\Current\default$windows.data.notifications.quiethourssettings`
- **Edge case:** Some notifications may be transient (disappear after timeout). The `wait --notification` command should monitor for new toasts via UIA event subscription (`UIA_Notification_EventId`)

### System Tray (New Feature — Windows Implementation)

System tray interaction must be implemented from scratch as part of Phase 2.

**Implementation approach:**
- **List items:** Access the system tray via UIA tree of `Shell_TrayWnd` window class. Tray items are children of the notification area. Overflow items live in `NotifyIconOverflowWindow`
- **Click:** `InvokePattern` on tray items, falling back to coordinate-based `SendInput` for items that don't expose UIA patterns
- **Open menu:** After clicking a tray item, detect the resulting popup menu via UIA focus-changed events and expose it for ref-based interaction

### Chromium Detection

- Detect Chromium-based windows (Electron, Chrome, Edge, VS Code) via UIA process name or class name matching
- If tree is empty or minimal for a Chromium window, warn: "This appears to be a Chromium app. Run the app with `--force-renderer-accessibility` to expose the accessibility tree"
- Include this guidance in the `platform_detail` field of the error response

### Minimum OS Requirements

- Windows 10 1809+ (October 2018 update)
- UIA COM interfaces available since Windows 7, but modern patterns require 10+

### New Dependency

| Crate | Version | Purpose | License |
|-------|---------|---------|---------|
| `uiautomation` | 0.24+ | Windows UIA wrapper via `windows` crate | Apache-2.0 |

Added to `Cargo.toml` as target-gated dependency:
```toml
[target.'cfg(target_os = "windows")'.dependencies]
agent-desktop-windows = { path = "crates/windows" }
```

### Testing

**Unit tests (windows):**
- UIA ControlType → role mapping coverage for all control types
- Permission check with mocks (COM security state)
- CacheRequest attribute batching correctness
- Element resolution round-trip (pid, role, name, bounds_hash)

**Integration tests (Windows CI):**
- Snapshot Explorer — non-empty tree with refs, buttons, text fields
- Snapshot Notepad — text area with value, menu items
- Snapshot Settings — modern WinUI controls
- Click button in test app — verify action succeeded
- Type text into Notepad via ref — verify content changed
- Set-value on a text field — verify value set via UIA
- Clipboard get/set/clear roundtrip
- Wait for window title pattern
- Launch + close app lifecycle (Notepad: launch, type, close)
- Resize, move, minimize, maximize, restore window operations
- Screenshot produces valid PNG
- Large tree snapshot performance validation
- Chromium detection — verify warning when tree is empty
- List notifications — returns non-empty list when notifications exist
- Dismiss notification — verify notification removed from Action Center
- Notification action — click action button on a test toast notification
- List tray items — returns known system tray entries (volume, network, clock)
- Click tray item — verify tray menu opens

**Cross-platform validation:**
- Same snapshot of a cross-platform app (e.g., VS Code) produces structurally identical JSON on macOS and Windows
- All error codes produce identical JSON envelope format

### CI

- Add GitHub Actions Windows runner alongside existing macOS runner
- Both runners execute: `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`
- `cargo tree -p agent-desktop-core` continues to contain zero platform crate names
- Binary size check: Windows `.exe` must be under 15MB

### Release

- [ ] Prebuilt Windows `.exe` binary published to GitHub Releases via `cargo-dist`
- [ ] npm package updated to include Windows binary (platform-specific download)
- [ ] GitHub Release notes document Windows support and installation

### Skill Update

Per [Skill Maintenance Addendum](./prd-addendum-skill-maintenance.md):

- [ ] Create `.claude/skills/agent-desktop-windows/SKILL.md`:
  - UIA permission model and UAC handling
  - Windows-specific behaviors (UIA patterns, WinUI3 quirks, COM initialization)
  - Chromium detection and `--force-renderer-accessibility` guidance
  - Windows error codes and `platform_detail` examples (HRESULT codes)
  - Troubleshooting guide (empty trees, COM errors, elevation failures)
- [ ] Update core `SKILL.md`:
  - Add Windows platform skill to skill graph table
  - Update platform support section
- [ ] Update `workflows.md`:
  - Add cross-platform patterns noting Windows-specific differences
  - Add Windows-specific workflow examples (e.g., navigating UWP apps)

### README Update

- [ ] Update Platform Support table: Windows column → **Yes**
- [ ] Add Windows installation instructions:
  - npm (same command, auto-detects platform)
  - Direct `.exe` download from GitHub Releases
  - From source: `cargo build --release` on Windows (note: requires MSVC toolchain)
- [ ] Add Windows permissions section:
  - UIA works without special permissions for most apps
  - UAC elevation may be required for elevated processes
  - Chromium apps need `--force-renderer-accessibility`
- [ ] Update "From source" section with Windows build requirements (Rust + MSVC)

---

## Phase 3 — Linux Adapter

**Status: Planned**

Phase 3 brings agent-desktop to Linux, completing the three-platform story. Same additive approach — only the `LinuxAdapter` implementation is added inside `crates/linux/`. The existing stub is replaced with a full implementation.

### Objectives

| ID | Objective | Metric |
|----|-----------|--------|
| P3-O1 | Linux adapter | `snapshot` on Ubuntu GNOME returns valid tree for Files, Terminal, Settings |
| P3-O2 | All commands cross-platform | Identical JSON schema output on all 3 platforms for every command |
| P3-O3 | Linux input synthesis | `click`, `type`, `press`, all mouse commands via AT-SPI actions + xdotool/ydotool |
| P3-O4 | Linux screenshot | `screenshot` produces PNG via PipeWire ScreenCast (Wayland) / XGetImage (X11) |
| P3-O5 | Linux clipboard | `clipboard-get` / `clipboard-set` / `clipboard-clear` via `wl-clipboard` (Wayland) / `xclip` (X11) |
| P3-O6 | Cross-platform CI | GitHub Actions matrix: macOS + Windows + Ubuntu |
| P3-O7 | Linux binary release | Prebuilt binary published via GitHub Releases and npm |

### Linux Adapter Implementation

Full `LinuxAdapter` in `crates/linux/src/` following the identical platform crate folder structure:

```
crates/linux/src/
├── lib.rs              # mod declarations + re-exports only
├── adapter.rs          # LinuxAdapter: PlatformAdapter impl
├── tree/
│   ├── mod.rs          # re-exports
│   ├── element.rs      # AT-SPI Accessible wrapper + attribute readers
│   ├── builder.rs      # D-Bus tree traversal via GetChildren
│   ├── roles.rs        # AT-SPI Role enum → unified role enum mapping
│   ├── resolve.rs      # Element re-identification for ref resolution
│   └── surfaces.rs     # Surface detection (menus, dialogs, popovers)
├── actions/
│   ├── mod.rs          # re-exports
│   ├── dispatch.rs     # perform_action via AT-SPI Action interface
│   ├── activate.rs     # Smart activation chain (DoAction → coordinate fallback)
│   └── extras.rs       # Text.InsertText, Selection helpers
├── input/
│   ├── mod.rs          # re-exports
│   ├── keyboard.rs     # xdotool (X11) / ydotool (Wayland) keyboard synthesis
│   ├── mouse.rs        # xdotool (X11) / ydotool (Wayland) mouse events
│   └── clipboard.rs    # wl-clipboard (Wayland) / xclip (X11) clipboard ops
├── notifications/
│   ├── mod.rs          # re-exports
│   ├── list.rs         # List notifications via D-Bus org.freedesktop.Notifications or daemon-specific API
│   ├── dismiss.rs      # Dismiss/close notifications via CloseNotification D-Bus method
│   └── interact.rs     # Invoke notification actions via ActionInvoked D-Bus signal
├── tray/
│   ├── mod.rs          # re-exports
│   ├── list.rs         # List tray items via StatusNotifierItem D-Bus interface or AT-SPI
│   └── interact.rs     # Activate/context-menu tray items via D-Bus methods
└── system/
    ├── mod.rs          # re-exports
    ├── app_ops.rs      # App launch via xdg-open / process spawn, close via SIGTERM/SIGKILL
    ├── window_ops.rs   # xdotool / wmctrl for resize/move/minimize/maximize/restore
    ├── key_dispatch.rs # App-targeted key press via window focus + input synthesis
    ├── permissions.rs  # AT-SPI2 bus availability check, DBUS_SESSION_BUS_ADDRESS detection
    ├── screenshot.rs   # PipeWire ScreenCast portal (Wayland) / XGetImage (X11) / xcap crate
    └── wait.rs         # wait utilities (polling AT-SPI element existence)
```

### Linux API Mapping

| Capability | Technology | Details |
|------------|-----------|---------|
| Tree root | `atspi Accessible` on bus | Via `atspi` crate (v0.28+) + `zbus` (5.x) — pure Rust, no libatspi/GLib dependency |
| Children | `org.a11y.atspi.Accessible.GetChildren` | Async D-Bus calls to AT-SPI2 registry daemon |
| Role mapping | AT-SPI `Role` enum | Map to unified role enum in `tree/roles.rs` — e.g. `Role::PushButton` → `button` |
| Click | `org.a11y.atspi.Action.DoAction(0)` | AT-SPI actions preferred over coordinate-based input |
| Set text | `org.a11y.atspi.Text.InsertText` | AT-SPI text interface; falls back to clipboard paste |
| Expand/Collapse | `Action.DoAction("expand")` / `Action.DoAction("collapse")` | Action name-based dispatch |
| Select | `org.a11y.atspi.Selection.SelectChild` | For combobox, listbox, tab items |
| Toggle | `Action.DoAction("toggle")` or `Action.DoAction("click")` | For checkboxes, switches |
| Scroll | Coordinate-based scroll events via xdotool/ydotool | AT-SPI has no native scroll pattern |
| Keyboard | `xdotool key` (X11) / `ydotool key` (Wayland) | Shelling out for input synthesis |
| Mouse | `xdotool mousemove/click` (X11) / `ydotool mousemove/click` (Wayland) | Display server detected at runtime |
| Clipboard | `wl-copy` / `wl-paste` (Wayland) / `xclip` (X11) | Shelling out; display server detected at runtime |
| Screenshot | PipeWire ScreenCast portal (Wayland) / `XGetImage` (X11) | Or `xcap` crate for consistency |
| App launch | `xdg-open` / direct process spawn | Launch by .desktop file or command name |
| App close | `SIGTERM` / `SIGKILL` | Graceful close first, force with `--force` |
| Window ops | `xdotool` / `wmctrl` | Window resize, move, minimize, maximize, restore |
| Permissions | AT-SPI2 bus availability | Check for `org.a11y.Bus` on D-Bus session bus. Return `PLATFORM_UNSUPPORTED` with enable instructions if missing |
| Notifications | D-Bus `org.freedesktop.Notifications` | List via `GetServerInformation` + monitoring `Notify` signals. History varies by daemon: GNOME uses `org.gnome.Shell.Notifications`, KDE uses `org.freedesktop.Notifications` with `GetNotifications`. Dismiss via `CloseNotification(id)`. Interact via `ActionInvoked` signal. Do Not Disturb: GNOME `org.gnome.desktop.notifications.show-banners`, KDE `org.kde.notificationmanager` |
| System tray | D-Bus `org.kde.StatusNotifierWatcher` | SNI (StatusNotifierItem) protocol for modern tray items. Legacy XEmbed tray items via AT-SPI tree of the tray window. List via `RegisteredStatusNotifierItems` property. Activate via `Activate(x, y)` method. Context menu via `ContextMenu(x, y)` method. Fallback: coordinate-based click for XEmbed items |

### Notification Management (New Feature — Linux Implementation)

Linux notification management must be implemented from scratch as part of Phase 3. The macOS implementation (completed) and Windows implementation (Phase 2) serve as reference patterns — same trait methods, same JSON output contract, same 1-based indexing.

**Implementation approach:**
- **List notifications:** The standard `org.freedesktop.Notifications` D-Bus interface does NOT provide a "list current notifications" method. Approach varies by desktop environment:
  - GNOME: `org.gnome.Shell` exposes `org.gnome.Shell.Notifications` interface with `GetNotifications()` method (returns array of notification dicts)
  - KDE Plasma: `org.freedesktop.Notifications` with `GetNotifications()` extension, or `org.kde.notificationmanager` D-Bus interface
  - Other DEs: Monitor `Notify` D-Bus signals to maintain an in-memory notification history within the daemon session
- **Dismiss:** `org.freedesktop.Notifications.CloseNotification(id)` D-Bus method call. Works across all notification daemons
- **Interact with actions:** Listen for user-triggered actions or programmatically invoke via `ActionInvoked` signal. Note: the D-Bus spec does not define a method to programmatically trigger actions — coordinate-based click on the notification popup via AT-SPI may be needed as a fallback
- **Do Not Disturb:**
  - GNOME: `gsettings get org.gnome.desktop.notifications show-banners` (boolean)
  - KDE: `org.kde.notificationmanager` D-Bus interface, `inhibited` property
- **Edge case:** Notification daemon varies by DE — detect via `GetServerInformation()` D-Bus method. Return `PLATFORM_UNSUPPORTED` with daemon-specific guidance if the notification interface is unreachable

### System Tray (New Feature — Linux Implementation)

System tray interaction must be implemented from scratch as part of Phase 3.

**Implementation approach:**
- **Modern tray (SNI):** Most modern Linux apps use the `StatusNotifierItem` (SNI) D-Bus protocol. Discover items via `org.kde.StatusNotifierWatcher.RegisteredStatusNotifierItems` property
- **Legacy tray (XEmbed):** Older apps use XEmbed protocol. Access via AT-SPI tree of the tray window, or coordinate-based interaction
- **List items:** Query `StatusNotifierWatcher` for registered items. Each item exposes `Title`, `IconName`, `ToolTip`, `Menu` (D-Bus menu path) properties
- **Activate:** Call `Activate(x, y)` method on the `StatusNotifierItem` D-Bus interface
- **Context menu:** Call `ContextMenu(x, y)` method, or read the `Menu` property to get the `com.canonical.dbusmenu` path and traverse the menu tree
- **Edge case:** GNOME does not natively support SNI (requires `AppIndicator` extension). Detect and report via error suggestion if no tray is available

### Display Server Detection

Runtime detection required for input, clipboard, and screenshot since Linux runs either X11 or Wayland:

- Check `$WAYLAND_DISPLAY` environment variable — if set, use Wayland path
- Check `$DISPLAY` environment variable — if set and no Wayland, use X11 path
- If neither, return `PLATFORM_UNSUPPORTED` with guidance to check display server configuration
- Input tools: verify `xdotool` (X11) or `ydotool` (Wayland) is installed; error with install instructions if missing
- Clipboard tools: verify `xclip` (X11) or `wl-clipboard` (Wayland) is installed; error with install instructions if missing

### AT-SPI2 Bus Detection

- Check for `org.a11y.Bus` presence on the D-Bus session bus
- If bus is not running, return `PLATFORM_UNSUPPORTED` with instructions:
  - GNOME: "AT-SPI2 should be enabled by default. Check `gsettings get org.gnome.desktop.interface toolkit-accessibility`"
  - Other DEs: "Install `at-spi2-core` and ensure `at-spi-bus-launcher` is running"
  - Flatpak/Snap: "Ensure the app has `--talk-name=org.a11y.Bus` permission"

### Minimum OS Requirements

- Ubuntu 22.04+ / Fedora 38+
- GNOME 42+ (primary target), KDE Plasma 5.24+ (secondary)
- `at-spi2-core` package installed (default on GNOME)
- X11: `xdotool` installed. Wayland: `ydotool` installed

### Key Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Wayland a11y gaps | Focus on GNOME (best AT-SPI2 support). Prefer AT-SPI actions over coordinate input. Document known gaps clearly in skill and README. |
| AT-SPI2 bus not running | Detect on first command. Return clear enable instructions specific to the detected distro/DE. |
| Display server fragmentation | Runtime detection (X11 vs Wayland). Separate code paths for input/clipboard/screenshot. Test both. |
| Rust a11y crate maintenance stalls | Pin `atspi` and `zbus` versions. `atspi` crate backed by Odilia accessibility project. Maintain patches if upstream stalls. |
| Input tool availability | Check for xdotool/ydotool on first use. Provide package manager install commands in error suggestion. |

### New Dependencies

| Crate | Version | Purpose | License |
|-------|---------|---------|---------|
| `atspi` | 0.28+ | Linux AT-SPI2 client | MIT/Apache-2.0 |
| `zbus` | 5.x | D-Bus connection | MIT/Apache-2.0 |
| `tokio` | 1.x | Async runtime (required by atspi/zbus for async D-Bus) | MIT |

Added to `Cargo.toml` as target-gated dependency:
```toml
[target.'cfg(target_os = "linux")'.dependencies]
agent-desktop-linux = { path = "crates/linux" }
```

Note: `tokio` is introduced here for the first time. Phases 1-2 are fully synchronous. The Linux adapter requires async D-Bus calls via zbus.

### Testing

**Unit tests (linux):**
- AT-SPI Role → role mapping coverage for all role types
- Bus availability detection (mock D-Bus responses)
- Display server detection logic (Wayland vs X11 env vars)
- Element resolution round-trip (pid, role, name, bounds_hash)

**Integration tests (Ubuntu CI):**
- Snapshot GNOME Files — non-empty tree with refs, buttons, text fields
- Snapshot GNOME Terminal — text area, menu items
- Snapshot GNOME Settings — modern GTK4 controls
- Click button in test app — verify action succeeded
- Type text into GNOME Text Editor via ref — verify content changed
- Clipboard get/set/clear roundtrip (test both X11 and Wayland if CI supports)
- Wait for window title pattern
- Launch + close app lifecycle
- Resize, move, minimize, maximize, restore window operations
- Screenshot produces valid PNG
- AT-SPI2 bus not running — correct error code and guidance
- List notifications — returns non-empty list when notifications exist (GNOME)
- Dismiss notification — verify notification dismissed via D-Bus `CloseNotification`
- List tray items — returns known SNI items (if running under KDE or with AppIndicator extension)
- Click tray item — verify tray menu opens via `Activate` D-Bus method
- Notification daemon detection — correct `GetServerInformation` result

**Cross-platform validation:**
- Same snapshot of a cross-platform app (e.g., VS Code) produces structurally identical JSON on all 3 platforms
- All error codes produce identical JSON envelope format on all 3 platforms
- Notification commands return identical JSON envelope structure across all 3 platforms (list, dismiss, action)
- Tray commands return identical JSON envelope structure across all 3 platforms

### CI

- GitHub Actions matrix: macOS + Windows + Ubuntu (all three on every PR)
- All runners execute: `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`
- `cargo tree -p agent-desktop-core` continues to contain zero platform crate names
- Binary size check: all platform binaries must be under 15MB

### Release

- [ ] Prebuilt Linux binary published to GitHub Releases via `cargo-dist`
- [ ] npm package updated to include Linux binary (platform-specific download)
- [ ] GitHub Release notes document Linux support, requirements, and installation

### Skill Update

Per [Skill Maintenance Addendum](./prd-addendum-skill-maintenance.md):

- [ ] Create `.claude/skills/agent-desktop-linux/SKILL.md`:
  - AT-SPI2/D-Bus setup and bus detection
  - Wayland vs X11 differences (input via xdotool/ydotool, clipboard via wl-clipboard/xclip, screenshot via PipeWire/XGetImage)
  - Required system tools: `xdotool` or `ydotool`, `xclip` or `wl-clipboard`
  - Linux error codes and `platform_detail` examples (D-Bus errors, bus not found)
  - Troubleshooting guide (bus not running, empty trees, missing tools, Flatpak/Snap permissions)
- [ ] Update core `SKILL.md`:
  - Add Linux platform skill to skill graph table
  - Update platform support section to show all 3 platforms
- [ ] Update `workflows.md`:
  - Add cross-platform patterns noting Linux-specific differences
  - Add Linux-specific workflow examples (e.g., GNOME app automation)
  - Document display server detection behavior

### README Update

- [ ] Update Platform Support table: Linux column → **Yes**
- [ ] Add Linux installation instructions:
  - npm (same command, auto-detects platform)
  - Direct binary download from GitHub Releases
  - From source: `cargo build --release` on Linux (note: requires `pkg-config`, `libdbus-1-dev`)
- [ ] Add Linux permissions section:
  - AT-SPI2 bus must be running (default on GNOME, may need enabling on other DEs)
  - Required tools: `xdotool` (X11) or `ydotool` (Wayland) for input synthesis
  - Required tools: `xclip` (X11) or `wl-clipboard` (Wayland) for clipboard
  - How to check: `busctl --user list | grep a11y`
- [ ] Update minimum OS versions: Ubuntu 22.04+ / Fedora 38+
- [ ] Update "From source" section with Linux build requirements

---

## Phase 4 — MCP Server Mode

**Status: Planned**

Phase 4 adds a new I/O layer. Core engine and all three platform adapters are unchanged. The MCP server wraps existing command logic in JSON-RPC tool definitions, enabling agent-desktop to work as an MCP server for Claude Desktop, Cursor, and other MCP-compatible hosts.

### Objectives

| ID | Objective | Metric |
|----|-----------|--------|
| P4-O1 | MCP server mode via `--mcp` | Responds to MCP `initialize` handshake, reports capabilities |
| P4-O2 | All commands as MCP tools | `tools/list` returns all tools with JSON Schema specs |
| P4-O3 | Claude Desktop validated | Claude Desktop invokes tools to control desktop apps end-to-end on all platforms |
| P4-O4 | Tool annotations | `readOnlyHint`, `destructiveHint`, `idempotentHint` on every tool |

### Entry Point

The binary crate's `main.rs` detects mode:
- If invoked with `--mcp` or stdin is a pipe: enter MCP server mode
- Otherwise: parse CLI arguments, execute command, print JSON to stdout

This is the invariant: every MCP tool maps 1:1 to a CLI command. `agent-desktop snapshot --app Finder` is identical to invoking the MCP `desktop_snapshot` tool. Testing, debugging, and documentation are never fragmented.

### New Crate: `agent-desktop-mcp`

```
crates/mcp/src/
├── lib.rs              # mod declarations + re-exports
├── server.rs           # MCP server bootstrap, initialize handler, capabilities reporting
├── tools.rs            # Tool definitions with #[tool] macro, parameter JSON Schemas
└── transport.rs        # Stdio transport (primary), optional HTTP+SSE
```

### MCP Tool Surface

Each MCP tool maps 1:1 to a CLI command. Tool names are prefixed with `desktop_` to avoid collision with other MCP servers.

| MCP Tool | CLI Equivalent | readOnly | destructive |
|----------|---------------|----------|-------------|
| `desktop_snapshot` | `snapshot` | true | false |
| `desktop_click` | `click <ref>` | false | false |
| `desktop_type_text` | `type <ref> <text>` | false | false |
| `desktop_set_value` | `set-value <ref> <text>` | false | false |
| `desktop_press_key` | `press <keys>` | false | false |
| `desktop_find` | `find <query>` | true | false |
| `desktop_list_windows` | `list-windows` | true | false |
| `desktop_focus_window` | `focus-window` | false | false |
| `desktop_launch_app` | `launch <app>` | false | false |
| `desktop_close_app` | `close-app <app>` | false | true |
| `desktop_screenshot` | `screenshot` | true | false |
| `desktop_scroll` | `scroll <dir>` | false | false |
| `desktop_drag` | `drag <from> <to>` | false | false |
| `desktop_select` | `select <ref> <val>` | false | false |
| `desktop_toggle` | `toggle <ref>` | false | false |
| `desktop_clipboard_get` | `clipboard get` | true | false |
| `desktop_clipboard_set` | `clipboard set <text>` | false | false |
| `desktop_wait` | `wait` | true | false |
| `desktop_get` | `get <prop> <ref>` | true | false |
| `desktop_is` | `is <state> <ref>` | true | false |
| `desktop_list_notifications` | `list-notifications` | true | false |
| `desktop_dismiss_notification` | `dismiss-notification <id>` | false | true |
| `desktop_dismiss_all_notifications` | `dismiss-all-notifications` | false | true |
| `desktop_notification_action` | `notification-action <id> <action>` | false | false |
| `desktop_list_tray_items` | `list-tray-items` | true | false |
| `desktop_click_tray_item` | `click-tray-item <id>` | false | false |
| `desktop_open_tray_menu` | `open-tray-menu <id>` | false | false |

### Transport

- **Stdio (primary):** MCP host spawns `agent-desktop --mcp` as child process. JSON-RPC over stdin/stdout. This is the only required transport.
- **HTTP+SSE (optional, stretch goal):** For remote scenarios. Additive, non-blocking for core milestone.
- **Session:** On MCP `initialize`, detect platform, check accessibility permissions, report capabilities. RefMap is session-scoped (held in memory, not persisted to disk like CLI mode).

### Initialize Handler

On receiving MCP `initialize`:
1. Detect platform (macOS / Windows / Linux)
2. Check accessibility permissions (`check_permissions()`)
3. Report capabilities: list of available tools, platform, permission status
4. If permissions not granted, include guidance in capabilities response

### New Dependencies

| Crate | Version | Purpose | License |
|-------|---------|---------|---------|
| `rmcp` | 0.15.0+ | Official MCP Rust SDK — `#[tool]` macro, JSON-RPC handling, transport | MIT/Apache-2.0 |
| `schemars` | 0.8+ | JSON Schema generation for tool parameter definitions | MIT/Apache-2.0 |
| `tokio` | 1.x | Async runtime (required by rmcp for MCP server event loop) | MIT |

Note: If `tokio` was already introduced in Phase 3 (Linux), it is already available. Otherwise, it is introduced here.

### Binary Crate Changes

- `src/main.rs` — Add `--mcp` flag detection, route to MCP server mode
- `Cargo.toml` — Add `agent-desktop-mcp` dependency (non-platform-gated, available on all platforms)
- No changes to `dispatch.rs`, `cli.rs`, or any command files — MCP tools call the same `execute()` functions

### Testing

**Unit tests (mcp):**
- Tool definition schema validation — every tool's JSON Schema is valid
- Tool invocation round-trip — call tool, verify response matches CLI output
- Initialize handler — correct capabilities, platform detection, permission status

**Integration tests:**
- Full MCP protocol compliance — initialize, tools/list, tool invocation, error responses
- Claude Desktop end-to-end: launch app → snapshot → click button → verify action
- Cursor end-to-end: same workflow
- Session isolation: RefMap is session-scoped, not shared across sessions
- Protocol edge cases: malformed requests, unknown tools, invalid parameters

**Cross-platform:**
- MCP server works identically on macOS, Windows, and Linux
- Same tool invocations produce same JSON structure on all platforms

### MCP Config Examples

Provide ready-to-use config snippets for:

**Claude Desktop (`claude_desktop_config.json`):**
```json
{
  "mcpServers": {
    "agent-desktop": {
      "command": "agent-desktop",
      "args": ["--mcp"]
    }
  }
}
```

**Cursor (`.cursor/mcp.json`):**
```json
{
  "mcpServers": {
    "agent-desktop": {
      "command": "agent-desktop",
      "args": ["--mcp"]
    }
  }
}
```

### Skill Update

Per [Skill Maintenance Addendum](./prd-addendum-skill-maintenance.md):

- [ ] Create `.claude/skills/agent-desktop-mcp/SKILL.md`:
  - MCP tool surface documentation (all tools, parameters, annotations)
  - Transport configuration (stdio setup, optional SSE)
  - Session management (RefMap scoping, initialize flow)
  - Tool-to-CLI mapping reference
  - MCP-specific error handling
- [ ] Update core `SKILL.md`:
  - Add MCP mode section
  - Add MCP skill to skill graph table
- [ ] Update `workflows.md`:
  - Add MCP workflow patterns (tool invocation from Claude Desktop, Cursor)
  - Add session lifecycle patterns

### README Update

- [ ] Add "MCP Server" section:
  - How to start: `agent-desktop --mcp`
  - What it does: wraps all CLI commands as MCP tools
  - Session behavior: RefMap scoped per session
- [ ] Add Claude Desktop configuration snippet
- [ ] Add Cursor configuration snippet
- [ ] Document `--mcp` flag in CLI reference
- [ ] Add note: every MCP tool maps 1:1 to a CLI command

---

## Phase 5 — Production Hardening

**Status: Planned**

Phase 5 transforms agent-desktop from functional to enterprise-grade. Persistent daemon process, session isolation for concurrent agents, comprehensive quality gates, and distribution via native package managers.

### Objectives

| ID | Objective | Metric |
|----|-----------|--------|
| P5-O1 | Persistent daemon | Warm snapshot completes in <50ms (vs 200ms+ cold start) |
| P5-O2 | Session isolation | Two agents hold independent RefMaps without interference |
| P5-O3 | Enterprise quality gates | All gates in quality gates table pass |
| P5-O4 | Package manager distribution | Available via brew (macOS), winget/scoop (Windows), snap/apt (Linux) |

### Daemon Architecture

The daemon is a long-running process that maintains state between CLI/MCP invocations, dramatically reducing startup latency.

**Auto-start:**
- CLI detects if daemon is running by checking for socket file (`~/.agent-desktop/daemon.sock` on Unix, named pipe on Windows)
- If not running, spawns daemon as background process
- Daemon listens on the socket for incoming commands

**Auto-stop:**
- Daemon exits after configurable idle timeout (default 5 minutes)
- No active sessions = idle timer starts
- Any new connection resets the idle timer

**Session multiplexing:**
- Each CLI invocation or MCP session gets a unique session ID
- Each session has its own RefMap (held in memory by daemon, not on disk)
- Sessions are isolated: agent A's refs never collide with agent B's refs
- Session destroyed on disconnect or explicit `session kill`

**Health check:**
- `agent-desktop status` returns: daemon PID, uptime, active session count, platform, permission status

### New Commands

| Command | Description |
|---------|-------------|
| `session list` | List active daemon sessions with IDs, creation time, last activity |
| `session kill <id>` | Terminate a specific daemon session, release its RefMap |

### CLI-to-Daemon Migration

When daemon is running:
1. CLI command parses arguments as usual
2. Instead of directly calling the adapter, CLI connects to daemon socket
3. Sends serialized command to daemon
4. Daemon executes command in the caller's session context
5. Returns JSON response to CLI
6. CLI prints response to stdout

When daemon is not running, CLI falls back to direct execution (same as Phases 1-4). Daemon is purely an optimization, never a requirement.

### Enterprise Quality Gates

| Gate | Requirement |
|------|-------------|
| Security | No arbitrary code execution. No privilege escalation. All actions allowlisted via Action enum. No network access (daemon communicates only via local socket). |
| Performance | Cold start <200ms. Warm snapshot <50ms via daemon. Tree traversal timeout 5s default, configurable. |
| Reliability | Zero panics in non-test code. Graceful daemon recovery on crash. Stale socket cleanup on startup. |
| Observability | Structured logging via `tracing` crate. `--verbose` flag for debug output. Timing metrics per operation logged at debug level. |
| Compatibility | Tested against target app matrix: Finder, TextEdit, Xcode, VS Code, Chrome (macOS); Explorer, Notepad, Settings, VS Code (Windows); Nautilus, Terminal, Firefox (Linux). |
| Distribution | Single binary per platform. No runtime dependencies. Reproducible builds. SHA256 checksums for every release artifact. |
| Documentation | README, CLI reference, MCP reference, per-platform setup guides, troubleshooting. |

### Performance Optimizations

| Optimization | Platform | Details |
|-------------|----------|---------|
| CacheRequest batching | Windows | Batch UIA attribute fetches via CacheRequest — reduces COM round-trips |
| Async tree walking | Linux | Parallel D-Bus calls for tree traversal — concurrent child fetching |
| Cached subtrees | All (daemon) | Reuse unchanged subtrees between snapshots in same session — skip re-traversal of stable UI regions |
| Warm adapter | All (daemon) | Adapter stays initialized between commands — skip COM init (Win), D-Bus connect (Linux), AX bootstrap (macOS) |

### Package Manager Distribution

| Platform | Package Manager | Format | Install Command |
|----------|----------------|--------|----------------|
| macOS | Homebrew | Formula | `brew install agent-desktop` |
| Windows | winget | Manifest | `winget install agent-desktop` |
| Windows | scoop | Manifest | `scoop install agent-desktop` |
| Linux | snap | Snap package | `snap install agent-desktop` |
| Linux | apt | .deb package | `apt install agent-desktop` |

Each package manager distribution includes:
- Prebuilt binary for the target platform
- SHA256 checksum verification
- Automatic PATH setup
- Uninstall support

### Testing

**Daemon tests:**
- Daemon starts on first CLI command when not running
- Daemon stops after idle timeout with no active sessions
- Multiple concurrent sessions have isolated RefMaps
- Session list returns correct session metadata
- Session kill terminates session and releases resources
- Stale socket cleaned up on daemon restart
- Daemon crash recovery — CLI falls back to direct execution
- Warm snapshot performance: <50ms after initial cold start

**Quality gate tests:**
- Security: verify Action enum is exhaustive, no shell injection vectors
- Performance: benchmark cold start (<200ms) and warm snapshot (<50ms)
- Reliability: stress test with concurrent sessions, verify zero panics
- Compatibility: snapshot + click workflow on each app in target matrix

**Package tests:**
- brew formula installs and runs on macOS
- winget/scoop manifest installs and runs on Windows
- snap package installs and runs on Ubuntu
- All packages produce correct `version` output
- All packages handle permissions correctly on their platform

### Skill Update

Per [Skill Maintenance Addendum](./prd-addendum-skill-maintenance.md):

- [ ] Update `commands-system.md`:
  - Add `session list` command documentation
  - Add `session kill <id>` command documentation
  - Update `status` command to document daemon-specific fields (PID, uptime, sessions)
- [ ] Update `workflows.md`:
  - Add daemon lifecycle patterns (auto-start, idle timeout, health checks)
  - Add concurrent agent patterns (session isolation, multi-agent coordination)
  - Add performance optimization patterns (warm snapshot, cached subtrees)
- [ ] Update platform skills:
  - Document enterprise quality gates in each platform skill
  - Add daemon-specific troubleshooting (stale socket, port conflicts)

### README Update

- [ ] Add "Daemon Mode" section:
  - How it works: auto-start, auto-stop, session isolation
  - Configuration: idle timeout, socket location
  - Health check: `agent-desktop status`
- [ ] Add package manager installation methods:
  - `brew install agent-desktop` (macOS)
  - `winget install agent-desktop` (Windows)
  - `snap install agent-desktop` (Linux)
- [ ] Add "Performance" section:
  - Cold start vs warm snapshot benchmarks
  - Daemon mode benefits
- [ ] Update installation section with all distribution channels (npm, brew, winget, scoop, snap, apt, source)
- [ ] Final polish:
  - Complete CLI reference for all commands including `session list` and `session kill`
  - Comprehensive troubleshooting guide covering all platforms
  - Per-platform setup guides linked from main README

---

## Cross-Phase Requirements

### README Update Schedule

The README is updated at the end of each phase to reflect the current state:

| Phase | README Changes |
|-------|---------------|
| Phase 1 | Initial README: npm + source installation, core workflow, all 50 commands, JSON output, ref system, error codes, platform support table (macOS only) |
| Phase 2 | Add Windows: `.exe` installation, Windows permissions, update platform table, Windows build instructions |
| Phase 3 | Add Linux: binary installation, AT-SPI2 setup, update platform table, Linux build instructions, minimum OS versions |
| Phase 4 | Add MCP Server: `--mcp` usage, Claude Desktop config, Cursor config, tool-to-CLI mapping |
| Phase 5 | Add daemon mode, package managers (brew/winget/snap), performance benchmarks, final troubleshooting guide |

### Skill Maintenance Rules

Per the [Skill Maintenance Addendum](./prd-addendum-skill-maintenance.md):

1. **Every new command** must be added to the appropriate `commands-*.md` file
2. **Every new platform** gets its own skill directory under `.claude/skills/agent-desktop-{platform}/`
3. **Every new mode** (MCP, daemon) gets its own skill file
4. **Breaking changes** to JSON output or CLI flags must update all affected skill files
5. **Skill files are reviewed** as part of the PR checklist for any command-surface change

### CI Matrix Evolution

| Phase | CI Runners |
|-------|-----------|
| Phase 1 | macOS |
| Phase 2 | macOS + Windows |
| Phase 3 | macOS + Windows + Ubuntu |
| Phase 4 | macOS + Windows + Ubuntu (+ MCP protocol tests) |
| Phase 5 | macOS + Windows + Ubuntu (+ daemon tests, package build verification) |

All runners enforce: `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `cargo tree -p agent-desktop-core` contains zero platform crate names, binary size <15MB.

### Dependency Introduction Schedule

| Dependency | Introduced In | Purpose |
|------------|---------------|---------|
| `clap` 4.x, `serde` 1.x, `thiserror` 2.x, `tracing` 0.1+, `base64` 0.22+ | Phase 1 | Core: CLI, JSON, errors, logging, encoding |
| `accessibility-sys` 0.1+, `core-foundation` 0.10+, `core-graphics` 0.24+ | Phase 1 | macOS AX API FFI |
| `uiautomation` 0.24+ | Phase 2 | Windows UIA wrapper |
| `atspi` 0.28+ + `zbus` 5.x | Phase 3 | Linux AT-SPI2 client via D-Bus |
| `tokio` 1.x | Phase 3 | Async runtime (required by atspi/zbus) |
| `rmcp` 0.15.0+ | Phase 4 | Official MCP Rust SDK |
| `schemars` 0.8+ | Phase 4 | JSON Schema generation for MCP tool parameters |

### Platform API Quick Reference

| Capability | macOS | Windows | Linux |
|------------|-------|---------|-------|
| Tree root | `AXUIElementCreateApp(pid)` | `IUIAutomation.ElementFromHandle()` | `atspi Accessible` on bus |
| Children | `kAXChildrenAttribute` | `TreeWalker.GetFirstChild` | `GetChildren` D-Bus |
| Click | `AXPress` | `InvokePattern.Invoke()` | `Action.DoAction(0)` |
| Set text | `AXValue = val` | `ValuePattern.SetValue()` | `Text.InsertText` |
| Keyboard | `CGEventCreateKeyboard` | `SendInput` | `xdotool` / `ydotool` |
| Clipboard | `NSPasteboard` | Win32 Clipboard API | `wl-clipboard` / `xclip` |
| Screenshot | `CGWindowListCreateImage` | `BitBlt` / `PrintWindow` | `PipeWire` / `XGetImage` |
| Permissions | `AXIsProcessTrusted()` | COM security / UAC | Bus availability |
| Notifications | Notification Center AX tree (`com.apple.notificationcenterui`) | UIA tree of Action Center / Toast Manager | D-Bus `org.freedesktop.Notifications` + daemon-specific history |
| System tray | `SystemUIServer` AX tree + `ControlCenter` AX tree | UIA tree of `Shell_TrayWnd` + overflow window | D-Bus `StatusNotifierWatcher` + XEmbed fallback |

---

## Risk Register

| ID | Risk | Likelihood | Impact | Mitigation |
|----|------|------------|--------|------------|
| R1 | macOS TCC friction deters adoption | High | High | Clear first-run guidance. Detect before any op. One-command setup: `permissions --request`. |
| R2 | Electron/Chrome no a11y tree by default | High | Medium | Detect Chromium windows. Print `--force-renderer-accessibility` guidance in error response. |
| R3 | Custom-rendered UIs invisible to a11y | Medium | High | Phase 5 stretch: vision fallback. Short-term: document limitation in README and skills. |
| R4 | Wayland a11y gaps | Medium | Medium | Focus on GNOME (best AT-SPI2 support). Prefer AT-SPI actions over coordinate input. Document gaps. |
| R5 | Rust a11y crate maintenance stalls | Low | High | Pin versions, maintain patches. `atspi` backed by Odilia project. Fork-ready. |
| R6 | MCP spec changes break compat | Low | Medium | Pin `rmcp` version. Monitor spec under Linux Foundation governance. |
| R7 | Tree traversal too slow (>5s) | Medium | Medium | Depth limiting via `--max-depth`. Focused-window-only. Cached subtrees in Phase 5 daemon. |
| R8 | Ref instability confuses agents | Medium | High | Clear docs: refs are snapshot-scoped. `STALE_REF` error with recovery hint. Stable hashing in Phase 5. |
