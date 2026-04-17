# agent-desktop — Phase Roadmap

> Source of truth for the phased delivery plan. Derived from [PRD v2.0](./agent_desktop_prd_v2.pdf) and the [Skill Maintenance Addendum](./prd-addendum-skill-maintenance.md).

---

## Release Tracker

Most recent shipments against this roadmap:

| Version | Date       | What shipped |
|---------|------------|--------------|
| v0.1.13 | 2026-04-17 | FFI cdylib on 5 platforms (aarch64/x86_64 macOS + Linux, x86_64 Windows MSVC), Sigstore build-provenance attestations, FFI review hardening (#26 — 50 commits) |
| v0.1.12 | 2026-03–04 | Progressive skeleton traversal + ref-rooted drill-down (#20) |
| v0.1.11 | 2026-02–03 | Skill-install prompt fix on all success paths |
| v0.1.9  | 2026-01–02 | Scalable skill architecture + ClawHub auto-publish (#14) |
| v0.1.8  | 2026-01    | `--compact` flag to collapse single-child unnamed nodes |
| v0.1.7  | 2025-12    | Electron / web app accessibility-tree compatibility |

- Phase 1 completion: incremental across v0.1.0 – v0.1.8 (macOS MVP, 53 commands, core engine).
- Phase 1.5 completion: v0.1.13 (FFI cdylib on 5 platforms).
- Phase 2+: not yet started. See **Gap Analysis — 2026-04-17 Research** at the bottom of this document for the latest re-prioritization evidence.

---

## Phase Overview

| Phase | Name | Status | Platforms |
|-------|------|--------|-----------|
| 1 | Foundation + macOS MVP | **Completed** (v0.1.0 – v0.1.12) | macOS |
| 1.5 | FFI Distribution (C-ABI cdylib) | **Completed** (v0.1.13) | macOS, Windows, Linux |
| 2 | Windows Adapter | Planned | macOS, Windows |
| 3 | Linux Adapter | Planned | macOS, Windows, Linux |
| 4 | MCP Server Mode | Planned | All |
| 5 | Production Hardening | Planned | All |

Each phase is strictly additive. Core engine, CLI parser, JSON contract, error types, snapshot engine, and command registry are never modified — only new `PlatformAdapter` implementations, new transports, and new modes are added.

---

## Phase 1 — Foundation + macOS MVP

**Status: Completed** — shipped incrementally across v0.1.0 – v0.1.12.

Phase 1 is the load-bearing phase. It establishes every shared abstraction, every trait boundary, every output contract, every error type, the complete command trait and registry, and the full workspace structure. All subsequent phases build on top of this foundation without modifying core.

### Objectives

| ID | Objective | Success Metric |
|----|-----------|----------------|
| P1-O1 | Working macOS snapshot CLI | `snapshot --app Finder` returns valid JSON with refs for all interactive elements |
| P1-O2 | Platform adapter trait | Trait compiles with mock adapter; macOS adapter satisfies all trait methods |
| P1-O3 | Ref-based interaction | `click @e3` successfully invokes AXPress on the resolved element |
| P1-O4 | Context efficiency | Typical Finder snapshot < 500 tokens (measured via tiktoken) |
| P1-O5 | Typed JSON contract | Output envelope carries `version: "1.0"`. **Partial**: dedicated `schemas/` JSON-Schema files were never delivered — deferred to Phase 5 quality gates. |
| P1-O6 | Permission detection | Missing Accessibility permission prints specific macOS setup instructions |
| P1-O7 | Command extensibility | Adding a new command is ~4 registration points: `commands/{name}.rs` + `commands/mod.rs` + `src/cli.rs` variant + `src/dispatch.rs` match arm |
| P1-O8 | 53 working commands | All commands pass integration tests |
| P1-O9 | CI pipeline | GitHub Actions macOS runner executes full test suite on every PR |
| P1-O10 | Progressive skeleton traversal | Skeleton + drill-down workflow achieves 78%+ token savings on Electron apps |

### Workspace Structure

```
agent-desktop/
├── Cargo.toml              # workspace: members, shared deps
├── rust-toolchain.toml     # pinned Rust version
├── clippy.toml             # project-wide lint config
├── LICENSE                 # Apache-2.0 (shipped in every release tarball)
├── crates/
│   ├── core/               # agent-desktop-core (platform-agnostic)
│   │   └── src/
│   │       ├── lib.rs          # public re-exports only
│   │       ├── node.rs         # AccessibilityNode, Rect, WindowInfo
│   │       ├── adapter.rs      # PlatformAdapter trait
│   │       ├── action.rs       # Action enum, ActionResult
│   │       ├── refs.rs         # RefMap, RefEntry (persisted at ~/.agent-desktop/last_refmap.json)
│   │       ├── ref_alloc.rs    # INTERACTIVE_ROLES, allocate_refs, is_collapsible, transform_tree
│   │       ├── snapshot_ref.rs # Ref-rooted drill-down (run_from_ref)
│   │       ├── snapshot.rs     # SnapshotEngine (filter, allocate, serialize)
│   │       ├── error.rs        # ErrorCode enum (12 variants), AdapterError, AppError
│   │       ├── notification.rs # NotificationInfo, NotificationFilter, NotificationIdentity
│   │       └── commands/       # one file per command (direct match, no Command trait)
│   ├── macos/              # agent-desktop-macos (Phase 1, shipped)
│   ├── windows/            # agent-desktop-windows (stub → Phase 2)
│   ├── linux/              # agent-desktop-linux (stub → Phase 3)
│   └── ffi/                # agent-desktop-ffi (cdylib, shipped v0.1.13; see Phase 1.5)
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

The single most important abstraction. Every platform-specific operation goes through this trait. Core never imports platform crates. The trait currently exposes ~27 methods with default implementations returning `not_supported()` — see `crates/core/src/adapter.rs` for the canonical definition. Key method shapes:

```rust
pub trait PlatformAdapter: Send + Sync {
    // Core observation
    fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError>;
    fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError>;
    fn get_tree(&self, win: &WindowInfo, opts: &TreeOptions) -> Result<AccessibilityNode, AdapterError>;
    fn get_subtree(&self, handle: &NativeHandle, opts: &TreeOptions) -> Result<AccessibilityNode, AdapterError>;
    fn list_surfaces(&self, pid: i32) -> Result<Vec<SurfaceInfo>, AdapterError>;

    // Interaction
    fn execute_action(&self, handle: &NativeHandle, action: Action) -> Result<ActionResult, AdapterError>;
    fn resolve_element(&self, entry: &RefEntry) -> Result<NativeHandle, AdapterError>;
    fn release_handle(&self, handle: &NativeHandle) -> Result<(), AdapterError>;
    fn mouse_event(&self, event: MouseEvent) -> Result<(), AdapterError>;
    fn drag(&self, params: DragParams) -> Result<(), AdapterError>;
    fn press_key_for_app(&self, pid: i32, combo: KeyCombo) -> Result<(), AdapterError>;

    // Lifecycle + windowing
    fn check_permissions(&self) -> PermissionStatus;
    fn focus_window(&self, win: &WindowInfo) -> Result<(), AdapterError>;
    fn focused_window(&self) -> Result<Option<WindowInfo>, AdapterError>;
    fn launch_app(&self, id: &str, timeout_ms: u64) -> Result<WindowInfo, AdapterError>;
    fn close_app(&self, id: &str, force: bool) -> Result<(), AdapterError>;
    fn window_op(&self, win: &WindowInfo, op: WindowOp) -> Result<(), AdapterError>;

    // Capture + clipboard
    fn screenshot(&self, target: ScreenshotTarget) -> Result<ImageBuffer, AdapterError>;
    fn get_clipboard(&self) -> Result<String, AdapterError>;
    fn set_clipboard(&self, text: &str) -> Result<(), AdapterError>;
    fn clear_clipboard(&self) -> Result<(), AdapterError>;

    // Notifications (macOS shipped; Windows/Linux planned)
    fn list_notifications(&self, filter: &NotificationFilter) -> Result<Vec<NotificationInfo>, AdapterError>;
    fn dismiss_notification(&self, index: usize, app_filter: Option<&str>) -> Result<NotificationInfo, AdapterError>;
    fn dismiss_all_notifications(&self, app_filter: Option<&str>) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError>;
    fn notification_action(&self, index: usize, identity: Option<&NotificationIdentity>, action_name: &str) -> Result<ActionResult, AdapterError>;

    // Property probes
    fn get_live_value(&self, handle: &NativeHandle) -> Result<Option<String>, AdapterError>;
    fn get_element_bounds(&self, handle: &NativeHandle) -> Result<Option<Rect>, AdapterError>;
    fn wait_for_menu(&self, pid: i32, open: bool, timeout_ms: u64) -> Result<bool, AdapterError>;
}
```

### Key Supporting Types

- `Action` — `#[non_exhaustive]` enum. Current variants: Click, DoubleClick, TripleClick, RightClick, SetValue(String), SetFocus, Expand, Collapse, Select(String), Toggle, Check, Uncheck, Scroll(Direction, Amount), ScrollTo, PressKey(KeyCombo), KeyDown(KeyCombo), KeyUp(KeyCombo), TypeText(String), Clear, Hover, Drag(DragParams)
- `MouseEvent`, `DragParams`, `KeyCombo` — dedicated types (not unified under an `InputEvent` enum)
- `WindowOp` — Resize{w,h}, Move{x,y}, Minimize, Maximize, Restore, Close
- `ScreenshotTarget` — FullScreen, Window(WindowInfo), Element(NativeHandle)
- `NotificationInfo` — index, app_name, title, body, actions: Vec<String>
- `NotificationIdentity` — expected_app, expected_title (used for NC-reorder-safe `notification_action`)
- `SurfaceInfo` — kind, label, bounds (for `list-surfaces` command)
- `TreeOptions` — max_depth, include_bounds, interactive_only, compact, surface, skeleton (root is CLI-only via `SnapshotArgs.root_ref`, not plumbed into `TreeOptions`)

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
│   └── actions.rs      # Click notification action buttons (identity-verified)
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

**Progressive Skeleton Traversal:**
- `--skeleton` flag clamps depth to `min(max_depth, 3)`, annotates truncated containers with `children_count` for agent discovery
- `--root <REF>` flag starts traversal from a previously-discovered ref instead of window root
- Named or described containers at skeleton boundary receive refs as drill-down targets (with empty `available_actions`)
- Scoped invalidation: re-drilling a ref replaces only that ref's subtree refs, preserving all others
- Core modules: `ref_alloc.rs` (canonical `allocate_refs` + `RefAllocConfig`), `snapshot_ref.rs` (drill-down flow that delegates allocation to `ref_alloc`)
- macOS: `count_children()` uses raw `CFArrayGetCount` without materializing `AXElement` wrappers for performance
- RefMap write-side size check prevents >1MB files
- Token savings: 78-96% reduction for dense Electron apps (Slack skeleton: ~3.6KB vs ~17.3KB full)

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

### Commands Shipped (53)

| Category | Commands | Count |
|----------|----------|-------|
| App / Window | `launch`, `close-app`, `list-windows`, `list-apps`, `focus-window`, `resize-window`, `move-window`, `minimize`, `maximize`, `restore` | 10 |
| Observation | `snapshot`, `screenshot`, `find`, `get` (text, value, title, bounds, role, states, tree-stats), `is` (visible, enabled, checked, focused, expanded), `list-surfaces` | 6 |
| Interaction | `click`, `double-click`, `triple-click`, `right-click`, `type`, `set-value`, `clear`, `focus`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse` | 14 |
| Scroll | `scroll`, `scroll-to` | 2 |
| Keyboard | `press`, `key-down`, `key-up` | 3 |
| Mouse | `hover`, `drag`, `mouse-move`, `mouse-click`, `mouse-down`, `mouse-up` | 6 |
| Clipboard | `clipboard-get`, `clipboard-set`, `clipboard-clear` | 3 |
| Notification (macOS) | `list-notifications`, `dismiss-notification`, `dismiss-all-notifications`, `notification-action` | 4 |
| Wait | `wait` (with `--element`, `--window`, `--text`, `--menu`, `--notification` flags) | 1 |
| System | `status`, `permissions`, `version` | 3 |
| Batch | `batch` | 1 |

> System Tray / Menu Bar Extras commands are listed under "Not Yet Implemented" above — they never shipped in Phase 1.

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

The `ErrorCode` enum in `crates/core/src/error.rs` exposes exactly 12 variants:

| Code | Category | Example | Recovery Suggestion |
|------|----------|---------|---------------------|
| `PERM_DENIED` | Permission | Accessibility not granted | Open System Settings > Privacy > Accessibility and add your terminal |
| `ELEMENT_NOT_FOUND` | Ref | @e12 could not be resolved | Run 'snapshot' to refresh, then retry with updated ref |
| `APP_NOT_FOUND` | Application | --app 'Photoshop' not running | Launch the application first |
| `ACTION_FAILED` | Execution | AXPress returned error on disabled button | Element may be disabled. Check states before acting |
| `ACTION_NOT_SUPPORTED` | Execution | Expand on a button | This element does not support the requested action |
| `STALE_REF` | Ref | RefMap is from a previous snapshot | Run 'snapshot' (or `snapshot --skeleton`) to refresh |
| `WINDOW_NOT_FOUND` | Window | --window w-999 does not exist | Run 'list-windows' to see available windows |
| `PLATFORM_NOT_SUPPORTED` | Platform | Windows/Linux adapter not yet shipped | This platform ships in Phase 2/3 |
| `TIMEOUT` | Wait / Traversal | wait --element exceeded timeout | Increase --timeout or check app state |
| `INVALID_ARGS` | Input | Bad CLI argument or unknown ref format | Fix the argument per CLI help |
| `NOTIFICATION_NOT_FOUND` | Notification | Notification ID not found / NC reordered | Run 'list-notifications' to see current notifications |
| `INTERNAL` | Internal | Unexpected error or caught panic | Re-run with verbose logging |

Exit codes: `0` success, `1` structured error (JSON on stdout), `2` argument/parse error.

> Codes the earlier draft listed but that **do not exist** in the codebase: `TREE_TIMEOUT` (use `TIMEOUT`), `CLIPBOARD_EMPTY` (no special code; empty clipboard returns empty string), `NOTIFICATION_UNSUPPORTED` (use `PLATFORM_NOT_SUPPORTED`), `TRAY_NOT_FOUND` / `TRAY_UNSUPPORTED` (tray commands never shipped). Deferred-work additions (see Gap Analysis at bottom): `PERMISSION_REVOKED`, `RESOURCE_EXHAUSTED`, `AX_MESSAGING_TIMEOUT`, `AUTOMATION_PERMISSION_DENIED`.

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

Current `.github/workflows/ci.yml` on every PR:
- `fmt` job on `ubuntu-latest`: `cargo fmt --all -- --check`
- `test` job on `macos-latest`:
  - `cargo tree -p agent-desktop-core` must contain zero platform crate names (dependency isolation)
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --lib --workspace`
  - `cargo test -p agent-desktop-ffi --tests` (c_abi_harness + c_header_compile + error_lifetime integration suites)
  - `cargo build --profile ci` (fast CLI binary) + 15 MB size check
  - `cargo build --profile release-ffi -p agent-desktop-ffi` (the shipped cdylib profile)
  - FFI header drift check — diffs `crates/ffi/include/agent_desktop.h` against the build-stamped `target/ffi-header-path.txt`

### Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `clap` | 4.x | CLI parsing with derive macros |
| `serde` + `serde_json` | 1.x | JSON serialization |
| `thiserror` | 2.x | Error derive macros |
| `tracing` | 0.1+ | Structured logging |
| `tracing-subscriber` | 0.3 | env-filter log formatter |
| `rustc-hash` | 2.1 | Faster hashing for ref maps and visited sets |
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

## Phase 1.5 — FFI Distribution (C-ABI cdylib)

**Status: Completed — v0.1.13 (2026-04-17).**

Phase 1.5 ships `crates/ffi/` as a first-class distribution target. The CLI stays the primary surface; the cdylib lets Python (ctypes), Swift, Node (ffi-napi), Go (cgo), Ruby (fiddle), and C consumers call `PlatformAdapter` directly without spawning `agent-desktop` per call.

### Objectives

| ID | Objective | Metric |
|----|-----------|--------|
| P1.5-O1 | Stable C-ABI surface | `crates/ffi/include/agent_desktop.h` drift-checked in CI via a deterministic `ffi-header-path.txt` stamp |
| P1.5-O2 | 5-platform release | Tarballs for aarch64/x86_64 apple-darwin, aarch64/x86_64 unknown-linux-gnu, and x86_64 pc-windows-msvc on every tagged release |
| P1.5-O3 | Panic safety | Dedicated `release-ffi` profile overrides `panic = "abort"` → `"unwind"`; `catch_unwind` wraps every `extern "C"` boundary via `trap_panic` / `trap_panic_ptr` / `trap_panic_const_ptr` / `trap_panic_void` |
| P1.5-O4 | Main-thread safety (macOS) | `require_main_thread()` guard in every build profile; worker-thread call returns `AD_RESULT_ERR_INTERNAL` with a static `'static CStr` message |
| P1.5-O5 | Enum UB immunity | Public ABI struct fields store raw `i32`; every entry validates discriminants at the boundary via `try_from_c_enum!` |
| P1.5-O6 | Out-param zeroing before any guard | Every fallible entry zeroes `*out` before pointer / UTF-8 / main-thread checks, so a worker-thread early return never leaves a stale caller buffer |
| P1.5-O7 | Sigstore build-provenance | `actions/attest-build-provenance@v4.1.0` signs every release artifact; consumers verify with `gh attestation verify <file> --repo lahfir/agent-desktop` |
| P1.5-O8 | Skill documentation | `skills/agent-desktop-ffi/SKILL.md` + references: `build-and-link.md`, `ownership.md`, `threading.md`, `error-handling.md` |
| P1.5-O9 | README surface | "Language bindings (FFI)" section on the project README with platform→artifact table, Python dlopen snippet, and Sigstore verify one-liner |

### Crate Layout

```
crates/ffi/
├── Cargo.toml           # crate-type = ["cdylib", "rlib"]
├── cbindgen.toml        # [export].include forces emission of AdActionKind / AdDirection / AdModifier / AdMouseButton / AdMouseEventKind / AdScreenshotKind / AdSnapshotSurface / AdWindowOpKind even though the public ABI stores raw i32
├── build.rs             # runs cbindgen into $OUT_DIR, stamps target/ffi-header-path.txt, bakes install_name = @rpath/libagent_desktop_ffi.dylib on macOS
├── include/
│   └── agent_desktop.h  # committed, drift-checked against the OUT_DIR output
├── src/                 # ad_* extern "C" entrypoints, organized by domain
│   ├── types/           # 34 one-type-per-file modules (AdAction, AdRect, AdWindowList, ...)
│   ├── convert/         # string / rect / window / app / surface / notification helpers
│   ├── tree/            # BFS flat-tree layout (flatten.rs, get.rs, free.rs)
│   ├── actions/         # conversion, resolve, execute, result, native_handle
│   ├── apps/ windows/ input/ screenshot/ surfaces/ notifications/ observation/
│   ├── error.rs         # AdResult, errno-style TLS last-error (message/suggestion/platform_detail)
│   ├── ffi_try.rs       # panic boundary helpers (trap_panic_*)
│   ├── enum_validation.rs # try_from_c_enum! macro, fuzz tests
│   └── main_thread.rs   # require_main_thread() guard
├── tests/
│   ├── c_abi_harness.rs    # raw extern "C" decls, enum fuzzing, out-param zeroing, null tolerance
│   ├── c_header_compile.rs # shells out to `cc` to verify every AD_* constant is usable from C
│   └── error_lifetime.rs   # last-error pointer stability across successful follow-up calls
└── examples/
    └── panic_spike.rs   # demonstrates panic boundary on the release-ffi profile
```

### Release Artifacts

Shipped via `.github/workflows/release.yml` `build-ffi` matrix job:

| Target | Runner | Archive | Library |
|--------|--------|---------|---------|
| aarch64-apple-darwin | macos-latest | `.tar.gz` | `libagent_desktop_ffi.dylib` |
| x86_64-apple-darwin | macos-latest | `.tar.gz` | `libagent_desktop_ffi.dylib` |
| x86_64-unknown-linux-gnu | ubuntu-22.04 | `.tar.gz` | `libagent_desktop_ffi.so` |
| aarch64-unknown-linux-gnu | ubuntu-22.04-arm | `.tar.gz` | `libagent_desktop_ffi.so` |
| x86_64-pc-windows-msvc | windows-latest | `.zip` | `agent_desktop_ffi.dll` |

Each archive contains `lib/`, `include/agent_desktop.h`, `LICENSE`, and a short `README.md`. macOS tarballs have their `install_name` verified `@rpath/libagent_desktop_ffi.dylib` via `otool -D` before upload. Linux binaries use `ubuntu-22.04` (glibc 2.35) as the baseline for maximum distro coverage.

### Build Profile

```toml
[profile.release-ffi]
inherits = "release"
panic    = "unwind"   # allow catch_unwind at the extern "C" boundary
```

Regular `release` profile keeps `panic = "abort"` for the CLI binary, so a panic there aborts the process rather than cascading through the FFI layer.

### CI Hooks Added

- `cargo build --profile release-ffi -p agent-desktop-ffi` on every PR
- `cargo test -p agent-desktop-ffi --tests` runs the 3 integration suites
- FFI header drift check diffs the committed header against the OUT_DIR output discovered via `target/ffi-header-path.txt` (deterministic even with warm caches and multiple `agent-desktop-ffi-<hash>/` directories)

### New Dependencies

| Crate | Version | Scope | Purpose |
|-------|---------|-------|---------|
| `cbindgen` | = 0.27.0 (pinned) | `crates/ffi` build-dep | C header generation |
| `libc` | 0.2+ | `crates/ffi` macOS target | `pthread_main_np` for main-thread check |

### Forward Compatibility

- Pre-1.0 the ABI is explicitly unstable; consumers pin the artifact version alongside the cdylib version.
- Any new `PlatformAdapter` method that lands in Phase 2/3 must add a matching `ad_*` FFI wrapper in the same PR that adds the adapter method.
- MCP server mode (Phase 4) is a parallel transport, not an FFI consumer — it calls `PlatformAdapter` directly.

### Known Gaps (surfaced by 2026-04-17 research)

- `ad_abi_version()` export is still missing (consumers have no runtime compat check)
- CLI-flagship primitives (`snapshot` with refs + refmap, `batch`, `wait`, `version`, `status`) are not wired through FFI — consumers today cannot replay the `click @e5` idiom without shelling out to the CLI
- No `tracing::` log callback — in-process consumers lose debug output
- No `pyo3` / `maturin` wheel or `cffi` wrapper ships with the repo

These items are tracked under **Gap Analysis — 2026-04-17 Research** at the bottom of this document.

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

### Web/Electron App Compatibility

Chromium-based apps (Electron, Chrome, Edge, VS Code) expose deep, noisy accessibility trees where every HTML `<div>` becomes a UIA Group element. The macOS adapter solved this with three patterns that must be replicated identically on Windows.

**Chromium detection:**
- Detect Chromium-based windows via UIA process name or `Chrome_WidgetWin_1` window class matching
- If tree is empty or minimal for a Chromium window, warn: "This appears to be a Chromium app. Run the app with `--force-renderer-accessibility` to expose the accessibility tree"
- Include this guidance in the `platform_detail` field of the error response

**Web-aware tree traversal (depth-skip):**
- Non-semantic wrapper elements (`UIA_GroupControlTypeId` / `UIA_CustomControlTypeId`) with empty `Name` AND empty `Value` properties do NOT consume depth budget during tree traversal
- This matches the macOS pattern where `AXGroup`/`AXGenericElement` wrappers are skipped
- Without this, default `--max-depth 10` finds ~3 refs in Slack; with it, finds 100+ refs
- Implement in `crates/windows/src/tree/builder.rs` with the same `is_web_wrapper` logic

**Resolver depth:**
- Element re-identification must search up to `ABSOLUTE_MAX_DEPTH` (50), not a lower hardcoded limit
- Electron elements commonly sit at depth 25+ in the raw tree; a shallow resolver cap causes `STALE_REF` errors
- Implement in `crates/windows/src/tree/resolve.rs` matching the macOS pattern

**Surface detection for Electron:**
- When an Electron app opens a modal (file picker, dialog), UIA may report the dialog as the focused window itself rather than a child of the parent window
- Surface detection (`list-surfaces`, `--surface sheet/alert`) must check if the focused window IS the target surface, not only search its children
- Check both `ControlType` and `LocalizedControlType` / UIA patterns (analogous to macOS checking both AXRole and AXSubrole)
- Implement in `crates/windows/src/tree/surfaces.rs`

**Progressive skeleton traversal** works identically on Windows — `--skeleton` and `--root` flags are platform-agnostic, handled entirely by core. The Windows adapter only needs to implement `get_subtree()` (which delegates to the same `build_subtree()` as `get_tree()`). Token savings for Electron apps (VS Code, Slack) apply equally.

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
- Electron app snapshot (VS Code) — default depth finds 50+ refs via web-aware depth-skip
- Electron surface detection — file picker dialog detected as sheet surface
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
  - Chromium/Electron compatibility: depth-skip, resolver depth, surface detection patterns
  - `--force-renderer-accessibility` guidance for empty trees
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

### Web/Electron App Compatibility

Same Chromium/Electron compatibility patterns as Phase 2 (Windows), adapted for AT-SPI2. These patterns ensure default `--max-depth 10` works with Electron apps like Slack, VS Code, and Chrome.

**Web-aware tree traversal (depth-skip):**
- Non-semantic wrapper elements with AT-SPI roles `ROLE_PANEL`, `ROLE_SECTION`, or `ROLE_FILLER` that have empty `Name` AND empty `Value` do NOT consume depth budget during tree traversal
- This is the AT-SPI equivalent of macOS `AXGroup`/`AXGenericElement` and Windows `UIA_GroupControlTypeId` skipping
- Implement in `crates/linux/src/tree/builder.rs` with the same `is_web_wrapper` logic

**Resolver depth:**
- Element re-identification must search up to `ABSOLUTE_MAX_DEPTH` (50), not a lower hardcoded limit
- Electron elements commonly sit at depth 25+ in the raw AT-SPI tree
- Implement in `crates/linux/src/tree/resolve.rs` matching the macOS/Windows pattern

**Surface detection for Electron:**
- When an Electron app opens a modal (file picker, dialog), AT-SPI may report the dialog as the active window itself rather than a child of the parent window
- Surface detection must check if the focused window IS the target surface, not only search its children
- Check both `Role` and `RelationSet` / `RELATION_EMBEDS` for dialog detection (analogous to macOS AXRole + AXSubrole)
- Implement in `crates/linux/src/tree/surfaces.rs`

**Chromium detection:**
- Detect Chromium-based apps via process name matching (electron, chrome, chromium, code)
- If AT-SPI tree is empty for a Chromium app, warn about `--force-renderer-accessibility`
- On Linux, Chromium respects `ACCESSIBILITY_ENABLED=1` environment variable as an alternative

**Progressive skeleton traversal** works identically on Linux — `--skeleton` and `--root` flags are platform-agnostic, handled entirely by core. The Linux adapter only needs to implement `get_subtree()` (which delegates to the same async tree walker). Token savings for Electron apps apply equally.

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
- Electron app snapshot (VS Code) — default depth finds 50+ refs via web-aware depth-skip
- Electron surface detection — file picker dialog detected as sheet surface
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
| Progressive skeleton drill | All | Skeleton overview + targeted drill-down reduces token consumption 78-96% for dense apps — fewer tokens per snapshot means more budget for actions |

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
| Phase 1 | Initial README: npm + source installation, core workflow, all 53 commands, JSON output, ref system, error codes, platform support table (macOS only) |
| Phase 1.5 | Add "Language bindings (FFI)" section: platform→artifact table, 5-line Python dlopen snippet, `shasum -a 256 -c checksums.txt` + `gh attestation verify` verification, link to `skills/agent-desktop-ffi/` |
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
| Phase 1 | `macos-latest` (tests + CLI build) + `ubuntu-latest` (`fmt` job) |
| Phase 1.5 | Same as Phase 1 on PRs; release workflow fans out to `macos-latest` × 2 darwin arches + `ubuntu-22.04` + `ubuntu-22.04-arm` + `windows-latest` for the FFI matrix |
| Phase 2 | macOS + Windows (CLI tests on Windows) |
| Phase 3 | macOS + Windows + Ubuntu |
| Phase 4 | macOS + Windows + Ubuntu (+ MCP protocol tests) |
| Phase 5 | macOS + Windows + Ubuntu (+ daemon tests, package build verification) |

All runners enforce: `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `cargo tree -p agent-desktop-core` contains zero platform crate names, binary size <15MB.

### Dependency Introduction Schedule

| Dependency | Introduced In | Purpose |
|------------|---------------|---------|
| `clap` 4.x, `serde` 1.x, `thiserror` 2.x, `tracing` 0.1+, `base64` 0.22+ | Phase 1 | Core: CLI, JSON, errors, logging, encoding |
| `tracing-subscriber` 0.3, `rustc-hash` 2.1 | Phase 1 | Log formatter + fast hashing |
| `accessibility-sys` 0.1+, `core-foundation` 0.10+, `core-graphics` 0.24+ | Phase 1 | macOS AX API FFI |
| `cbindgen` = 0.27.0 (pinned), `libc` 0.2+ | Phase 1.5 | C header generation + macOS `pthread_main_np` for FFI main-thread guard |
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
| R7 | Tree traversal too slow (>5s) | Medium | Medium | Depth limiting via `--max-depth`. Focused-window-only. Cached subtrees in Phase 5 daemon. Progressive skeleton traversal (`--skeleton` + `--root`) reduces token consumption 78-96% for dense apps. |
| R8 | Ref instability confuses agents | Medium | High | Clear docs: refs are snapshot-scoped. `STALE_REF` error with recovery hint. Stable hashing in Phase 5. Progressive skeleton traversal with scoped invalidation provides a stable drill-down workflow for navigating complex UIs. |

---

## Gap Analysis — 2026-04-17 Research

Four parallel research agents (codebase-internal, external web + Apple/Microsoft/Linux docs, MCP/context7, competitive) produced an evidence-backed gap report right after the Phase 1.5 ship. This section captures the priority-ordered findings so future phases can be re-scoped against current 2026 expectations. Every item cites a file path, an Apple/MS/Linux API, or a competitor repo; nothing here is vibes-based.

### P0 — category-defining gaps

**1. MCP mode should move from Phase 4 → Phase 2 ahead of Windows/Linux.** 2026 is MCP-native: Playwright MCP, `mcp-server-macos-use`, `mcp-desktop-automation`, `computer-use-mcp`, and Microsoft Agent Framework 1.0 (April 2026) all discover tools through MCP. Shipping macOS-only via CLI in 2026 means no default integration with Claude Desktop, Cursor, VS Code Copilot, Gemini, or Microsoft Agent. `skills/agent-desktop/references/commands-*.md` already lists the full tool surface — porting to MCP is mostly a transport layer. See Playwright MCP's `e1/e2` ref shape (https://playwright.dev/mcp/snapshots) for the idiom we should match.

**2. Tree-diff on every action (`wait --event` push from `AXObserver`).** Our closest direct competitor (`macos-use`, https://github.com/mediar-ai/mcp-server-macos-use) returns the AX diff on every call — "added elements, removed elements, modified attributes" — and that single ergonomic halves an agent's re-snapshot token cost. We currently ship `wait --element` polling only. Apple ships `AXObserverCreate` + `AXObserverAddNotification` with a `CFRunLoopSource`; see `crates/macos/src/system/wait.rs` for the polling site to replace. Relevant notifications: `kAXValueChangedNotification`, `kAXFocusedUIElementChangedNotification`, `kAXUIElementDestroyedNotification`, `kAXWindowCreatedNotification`, `kAXMenuOpenedNotification`, `kAXMenuClosedNotification`, `kAXApplicationShownNotification`.

**3. Text range primitives (macOS).** Every writing / code-editor / Terminal / Notes agent hits this wall: no `AXSelectedTextRange` read or write, no `AXStringForRangeParameterizedAttribute`, no `AXBoundsForRangeParameterizedAttribute`. Land a `crates/macos/src/actions/text_ops.rs` backed by `AXValueCreate(kAXValueCFRangeType, ...)`; add new `Action::SelectRange { start, len }` / `GetSelectedText` / `InsertAtCaret`. Without this we can read a text field's value but cannot reliably position a caret inside it.

**4. FFI CLI-parity gap — no `ad_snapshot` / `ad_execute_by_ref("@e5")` / `ad_wait` / `ad_version`.** The cdylib ships `ad_get_tree` (raw, refless) and `ad_execute_action(handle, …)`, but a Python/Swift/Go consumer cannot replay the flagship CLI idiom `agent-desktop click @e5`. Every CLI action command (`crates/core/src/commands/click.rs`, `focus.rs`, `toggle.rs`) walks `RefMap::load()` — the FFI never reads the refmap. Ship either (a) `ad_execute_by_ref(adapter, "@e5", action)` or (b) an opaque `ad_refmap_load()` + `ad_refmap_resolve("@e5")`. Also export `ad_abi_version() -> u32` — today a consumer built against 0.1.13 can silently load 0.2.0 and crash at runtime.

**5. `AccessibilityNode` is lossy — no `identifier` / `subrole` / `role_description` / `placeholder` / `selected` / `checked`.** macOS exposes `kAXIdentifierAttribute`, `kAXSubroleAttribute`, `kAXRoleDescriptionAttribute`, `kAXPlaceholderValueAttribute`, `kAXSelectedAttribute`. Windows UIA `AutomationId` and Linux AT-SPI `accessible-id` map 1:1 to `identifier` — this is the cross-platform anchor that makes selectors stable across sessions. Expanding the struct in `crates/core/src/node.rs` unblocks every "find the button with data-testid X" pattern.

**6. ScreenCaptureKit replacement for `/usr/sbin/screencapture` subprocess.** `crates/macos/src/system/screenshot.rs:12-32` shells out to the system binary — on Sonoma+ it's sandbox-flaky and ~300 ms cold. `SCScreenshotManager.captureImage(contentFilter:config:)` over a `SCShareableContent.windows` target is ~10× faster, no subprocess, and respects Screen Recording TCC explicitly. Pair with `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess` so `check_permissions()` catches missing Screen Recording grant (today it only checks AX).

**7. Electron `AXEnhancedUserInterface` toggle.** Our own skill docs (`skills/agent-desktop-macos/references/electron-compat.md`) identify this, but the adapter never writes it. Chromium-backed apps (VS Code, Cursor, Slack post-Sept-2024 rewrite, Teams) drop descendants unless we flip `AXEnhancedUserInterface = YES` on the app root. Add a probe in `crates/macos/src/tree/builder.rs` that gates on known Electron bundle IDs.

**8. `AXDOMIdentifier` / `AXDOMClassList` readout on web content.** `data-testid`-style selectors — the single highest-leverage attribute for reliable Electron/Safari agents — are invisible to us today. Promote them to first-class `AccessibilityNode` fields (`dom_id`, `dom_classes`) populated in `crates/macos/src/tree/builder.rs` under an `--include-dom` flag so the default envelope stays lean.

**9. No one-command install (`brew`, `winget`).** `agent-browser` ships via npm + Homebrew + Cargo. We ship only npm. For Phase 5, land `brew install lahfir/tap/agent-desktop` and `winget install agent-desktop` — the install-friction fight is real for Framework integration.

### P1 — important gaps

**10. Sandbox + `--dry-run` + `--confirm` + append-only audit log trio.** EU AI Act Article 14 + OWASP Agentic Top-10 (2026) + every HITL framework (LangGraph, Mastra, Permit.io) converge on: destructive actions route through a policy check, with an immutable trace. `--dry-run` (resolve ref, compute would-be action, emit JSON, don't execute), `--confirm` (stderr prompt with timeout), `~/.agent-desktop/audit.jsonl`. None ship today.

**11. Missing surfaces: Toolbar, Spotlight (macOS 26), Dock, menu-bar status items.** `crates/macos/src/tree/surfaces.rs` has 7 surfaces but not these four. Safari's URL bar, every Xcode toolbar button, the Tahoe Spotlight actions pane, Dock badges (e.g. "is Slack badged?"), and menu-bar extras (Bartender, Dropbox, Rectangle) are all first-class agent targets today.

**12. Missing `Action` variants: `LongPress`, `ForceClick`, `ShowMenu`, `FileDrop`, `AXRaise`, `AXCancel`.** We call `AXShowMenu` / `AXRaise` / `AXCancel` internally as fallbacks inside the activation chain but never expose them. Force-click opens Dictionary, Xcode jump-to-def, Finder Quick Look — otherwise unreachable. File-promise drag (`NSPasteboard`) is the "drag this file into the upload box" primitive agents need constantly.

**13. Missing `ErrorCode` variants.** Agent 1 flagged: `PermissionRevoked` (TCC yanked mid-session, distinct from `PermDenied`), `ResourceExhausted` (refmap > 1 MB guard, tree size caps), `AxMessagingTimeout` (AX-specific timeout distinct from orchestration `Timeout`), `AutomationPermissionDenied` (`osascript` automation grant).

**14. `tracing::` log callback over FFI.** Zero `tracing::` lines in `crates/ffi/`. A consumer that dlopens the dylib loses every debug/info/warn the core emits. wasmtime ships `wasmtime_log_set_callback`; we should ship `ad_set_log_callback(fn(level, msg))` that installs a `tracing_subscriber` layer.

**15. No OCR / vision fallback for inaccessible UIs.** UI-TARS-2 hits 47.5% on OSWorld as pure-vision baseline; Claude Opus 4.7 hits 72.7%. Our tool returns `ACTION_FAILED` on empty AX trees, leaving Canvas apps, Flutter-desktop, games, remote desktop stuck. A tight `find --visual "label"` backed by macOS Vision framework `VNRecognizeTextRequest` (free, no Tesseract dep) closes the gap without abandoning AX-first.

**16. `pyo3` + `maturin` Python wheel OR `cffi` helper OR `uniffi` multi-language bindgen.** Today ctypes is the only documented consumer. A maturin wheel with `__enter__`/`__exit__` adapter context managers and automatic last-error → Python exception is the highest-ROI ergonomics improvement for the primary consumer language. Alternative: `uniffi` emits Python/Swift/Kotlin/Ruby from one UDL — biggest bang for buck if we want to cover four languages at once.

**17. Structured session trace with `--trace-id` + `~/.agent-desktop/traces/{uuid}.jsonl`.** Playwright 1.59 added `page.screencast`, Browserbase exports HAR+video, Amazon Bedrock AgentCore emits rrweb-style replays correlated with OpenTelemetry. Every 2026 tool ships a visual receipt. Our JSON envelope is per-command only — no session ID, no event stream, no replay artifact.

### P2 — nice-to-have parity / polish

- **Iterator helper for `AdNodeTree`** — current `(*mut AdNode, u32)` forces callers to hand-slice `child_start..child_start+child_count`. wasmtime ships an iterator macro; rustls-ffi ships `rustls_slice_*` types.
- **Static `#[repr(i32)]` discriminant assertions.** Today variants are hand-numbered in `agent_desktop.h` with no compile-time guard; a refactor reorder would silently renumber. Add `assert_eq!(AdActionKind::Click as i32, 0)` blocks.
- **`ad_get` only supports `value` / `bounds`.** CLI supports 6 properties (`text`, `value`, `title`, `bounds`, `role`, `states`). FFI is strictly weaker.
- **Pixel-precision scroll.** `crates/macos/src/input/mouse.rs::synthesize_scroll_at` uses line units; WebKit surfaces often ignore tiny deltas. Add `--pixels` flag.
- **Per-app Automation (TCC) detection.** `close_app` runs `osascript` which triggers Automation grant; today we squeeze the failure into `ActionFailed`. `AEDeterminePermissionToAutomateTarget()` gives a specific error.
- **No pkg-config `.pc` file in the release.** Blocks easy integration on Linux/BSD; `cargo-c` could generate both the `.pc` and the `rpath` fix for free.
- **Widget + Writing Tools + Live Translation + Game Mode surfaces (macOS 15+).** Niche today, routine by macOS 27.

### Recommended re-prioritization for Phase 2+

Based on the above, the 2026-Q2 order most likely to keep the project competitive:

1. **Phase 2 (new scope): MCP Server Mode** — expose the 53 commands as MCP tools, export `last_refmap.json` as an MCP resource, follow Playwright MCP's `{ref: "e5"}` shape. ~2 weeks; unblocks every framework integration.
2. **Phase 2b: AX Observer + tree-diff API + text-range primitives + ScreenCaptureKit.** The macOS modernization bundle. Each item has a concrete Apple API and lands in a known file; combined effect is a dramatic latency + token-budget win.
3. **Phase 2c: FFI parity (ad_snapshot, ad_execute_by_ref, ad_wait, ad_version, log callback) + Python wheel.** Makes in-process consumers first-class.
4. **Phase 3 (re-scoped): Windows adapter via UIA.** UI-TARS / Computer Use already ship Windows; don't fall further behind. FlaUI or raw `windows-rs` UIA bindings.
5. **Phase 4: Linux adapter via AT-SPI2.** Historically the smallest slice of the agent market; keep it planned but behind Windows.
6. **Phase 5: Sandbox + dry-run + audit + OCR fallback + brew/winget install.** Production hardening + vision fallback + distribution breadth.

### Evidence appendix — external references cited

- Playwright MCP (e1/e2 refs, `browser.bind()`, `page.screencast`): https://github.com/microsoft/playwright-mcp
- `macos-use` MCP server (tree-diff on every action): https://github.com/mediar-ai/mcp-server-macos-use
- Cua (YC X25, sandbox VMs): https://github.com/trycua/cua
- ByteDance UI-TARS-2: https://github.com/bytedance/UI-TARS-desktop
- Microsoft Agent Framework 1.0 (MCP-first, April 2026): https://opensource.microsoft.com/blog/2026/04/02/introducing-the-agent-governance-toolkit-open-source-runtime-security-for-ai-agents/
- Anthropic Computer Use: https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool
- OpenAI CUA: https://openai.com/index/computer-using-agent/
- OSWorld-Verified (April 2026): https://xlang.ai/blog/osworld-verified
- AX Observer API: https://developer.apple.com/documentation/applicationservices/1462089-axobserveraddnotification
- ScreenCaptureKit (WWDC22): https://developer.apple.com/videos/play/wwdc2022/10156/
- OWASP Agentic Top-10 (2026): https://www.authensor.com/updates/owasp-agentic-top-10-explained
- wasmtime C API (byte-vec + log-callback + version macros): https://github.com/bytecodealliance/wasmtime/tree/main/crates/c-api
- rustls-ffi (cargo-c + pkg-config): https://github.com/rustls/rustls-ffi
- uniffi (multi-language bindgen): https://github.com/mozilla/uniffi-rs
