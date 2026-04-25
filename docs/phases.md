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
- Phase 2: planned. Full scope defined in `docs/plans/2026-04-18-001-feat-phase2-windows-crossplatform-plan.md` (superseding `docs/brainstorms/2026-02-25-windows-adapter-phase2-brainstorm.md` and `docs/plans/2026-02-25-feat-windows-adapter-phase2-plan.md`). Research-driven refinements to the brainstorm are captured in the plan's §Headless-First Invariant, §Key Technical Decisions, and §Review-Driven Refinements sections.
- Phase 3+: planned. See Phase 2 plan for trait method defaults that Phase 3 backfills.

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

## Command Surface Architecture (DRY invariant)

Every command in agent-desktop lives **exactly once** in `crates/core/src/commands/` and flows automatically to every transport (CLI, FFI, MCP) and every platform (macOS, Windows, Linux). No transport has per-platform code; no platform has per-transport code. The only place a new command branches is into the `PlatformAdapter` trait method calls it makes — and even those are written once per adapter in each platform crate, never per transport.

### Layering

```
                          ┌─────────────────────────────────┐
                          │   crates/core/src/commands/     │
                          │   one file per command,         │
                          │   operates on &dyn PlatformAdapter │
                          └─────────┬───────────┬───────────┘
                                    │           │
                 ┌──────────────────┼───────────┼──────────────────┐
                 │                  │           │                  │
                 ▼                  ▼           ▼                  ▼
    ┌─────────────────┐  ┌───────────────┐  ┌────────────┐  ┌─────────────────┐
    │   src/cli.rs    │  │ crates/ffi/   │  │ crates/mcp/│  │   (future)      │
    │   (clap derive) │  │ ad_* extern C │  │ #[tool]    │  │   HTTP / gRPC   │
    └────────┬────────┘  └───────┬───────┘  └──────┬─────┘  └────────┬────────┘
             │                   │                 │                 │
             └───────────────────┴─────────────────┴─────────────────┘
                                       │
                                       ▼
                          ┌─────────────────────────────────┐
                          │   PlatformAdapter trait         │
                          │   (defined in crates/core)      │
                          └─────────┬──────────┬────────────┘
                                    │          │
                 ┌──────────────────┼──────────┼──────────────────┐
                 ▼                  ▼          ▼                  ▼
          crates/macos/     crates/windows/   crates/linux/   (future platforms)
```

### What each layer contains

| Layer | Scope | Per-command cost | Per-platform cost |
|-------|-------|------------------|-------------------|
| `crates/core/src/commands/<name>.rs` | Args struct + `execute(args, adapter)` function, platform-agnostic | **1 file** | 0 |
| `crates/core/src/adapter.rs` (trait) | One method per distinct low-level operation (e.g. `watch_element`, `set_text_selection`) | 0 for most commands; ≤1 trait method when the command needs a new primitive | 0 |
| `crates/macos/`, `crates/windows/`, `crates/linux/` | Trait method implementations, one file per operation domain | 0 | **1 implementation per new trait method, per adapter** (real per-platform work) |
| `src/cli.rs` (clap enum) + `src/dispatch.rs` (match arm) | CLI transport | 2 lines (enum variant + match arm) | 0 |
| `crates/ffi/src/<domain>/<name>.rs` | One `ad_*` extern "C" wrapper per command | ~30 lines of marshaling | 0 |
| `crates/mcp/src/tools.rs` (registry) | One `#[tool]`-annotated wrapper per command | 1 annotation on the core `execute` function | 0 |

**Concretely:** adding `text select-range` in Phase 2 means:

1. Write `crates/core/src/commands/text_select_range.rs` with `TextSelectRangeArgs` + `execute(args, adapter)`. Calls `adapter.set_text_selection(handle, range)`.
2. Add one method `set_text_selection` to `PlatformAdapter` with a `not_supported` default.
3. Implement it in `crates/macos/src/actions/text_ops.rs` via `kAXSelectedTextRangeAttribute`.
4. Implement it in `crates/windows/src/actions/text_ops.rs` via `TextPattern.SetSelection`.
5. Implement it in `crates/linux/src/actions/text_ops.rs` via `org.a11y.atspi.Text.AddSelection` (Phase 3).
6. CLI: add 1 variant to `cli.rs` enum + 1 arm to `dispatch.rs`.
7. FFI: add `crates/ffi/src/observation/text_select_range.rs` with `ad_text_select_range(adapter, ref, start, length)` — ~30 lines of marshaling.
8. MCP: no file change. The `#[mcp_tool]` registry sees the new command automatically via the inventory submit below.

### Shared command registry pattern

Every command's `execute` function is annotated once; the registry is populated at link time via `inventory::submit!` (or `linkme`). Transports iterate the registry — they do not hand-maintain a list:

```rust
// crates/core/src/commands/click.rs  — the SINGLE source of truth for Click

use schemars::JsonSchema;

#[derive(Debug, clap::Parser, JsonSchema, serde::Deserialize)]
pub struct ClickArgs {
    /// The ref to click, e.g. @e5
    #[arg(value_name = "REF")]
    pub ref_id: String,
}

pub fn execute(args: ClickArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let entry = RefMap::load()?.resolve(&args.ref_id)?;
    let handle = adapter.resolve_element(&entry)?;
    let result = adapter.execute_action(&handle, Action::Click)?;
    Ok(json!({ "action": result.action }))
}

// Registered once, visible to every transport.
inventory::submit! {
    CommandDescriptor {
        cli_name: "click",
        mcp_name: "desktop_click",
        description: "Press the element identified by @ref (AXPress / UIA InvokePattern / AT-SPI Action.DoAction(0))",
        args_schema: || schema_for!(ClickArgs),
        invoke: invoke_typed::<ClickArgs>(execute),
        annotations: ToolAnnotations {
            read_only: false,
            destructive: false,
            idempotent: false,
            ..Default::default()
        },
    }
}
```

- `crates/core/src/command_registry.rs` defines `CommandDescriptor` and the `invoke_typed` generic helper (parse JSON → Args → execute → JSON back).
- `src/dispatch.rs` walks `inventory::iter::<CommandDescriptor>` and matches on `cli_name` — replaces the hand-maintained match when Phase 2 lands the registry (today's dispatch is the bootstrap form).
- `crates/ffi/src/...` walks the same registry to generate `ad_<name>` wrappers via a `build.rs` codegen step (Phase 2 work) — so adding a CLI command *also* emits the FFI entry automatically. Bespoke marshaling lives in per-type `convert/` helpers, not per-command.
- `crates/mcp/src/server.rs` walks the same registry and registers each as an `rmcp` tool with the auto-generated JSON Schema. **Zero per-platform code in the MCP crate. Zero per-command MCP code beyond the one-line `inventory::submit!`.**

### Why this works

- **Rust has no runtime reflection**, but the `inventory` / `linkme` crates provide compile-time plugin registration with zero runtime overhead.
- **`schemars` derives JSON Schema** from the same Args struct that clap derives CLI parsing from — one type, two bindings for free, a third for MCP.
- **`rmcp` (the official MCP Rust SDK) accepts `JsonSchema`-derived types directly** via its `#[tool]` macro, so MCP registration is a trivial pass-through.
- **`PlatformAdapter` is dyn-compatible** and passed as `&dyn PlatformAdapter` to every `execute` function; the binary crate's `build_adapter()` is the one-and-only place a concrete adapter is chosen per `#[cfg(target_os)]`.

### What this means for the phases below

- **Phase 2 ships the registry migration** as part of the core-extension work. After Phase 2, adding a new command is additive in exactly the places listed in the table above — never per-transport-per-platform.
- **Phase 3's Linux adapter** implements the new trait methods from Phase 2 but writes zero command files, zero CLI dispatch, zero FFI wrappers, zero MCP tool code. Pure platform impl.
- **Phase 4 (MCP)** is a new crate + stdio/HTTP transport + registry walker. It does not enumerate 53 tools by hand. The tool table in Phase 4 is a snapshot of what the registry produces, not a manual list.

Every objective in Phases 2–5 below assumes this invariant. If any task description implies per-transport or per-platform command-surface duplication, it's a wording bug — the actual implementation follows the registry pattern.

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

These items are tracked in the Phase 2 plan (`docs/plans/2026-04-18-001-feat-phase2-windows-crossplatform-plan.md`) — specifically Unit 2 (registry migration via `build.rs` filesystem enumeration, NOT inventory/linkme), Unit 2.5 (`ad_set_log_callback` with redaction), and Unit 2.6 (`ad_execute_by_ref` + descriptor confirms).

---

## Phase 2 — Windows Adapter + Cross-Platform Feature Parity

**Status: Planned** — authoritative plan: `docs/plans/2026-04-18-001-feat-phase2-windows-crossplatform-plan.md`. This section remains the high-level objective catalogue; the plan owns implementation-unit detail, research-driven refinements, and the progressive-commit / swarm strategy.

### Core invariants (research-driven — from Phase 2 plan §Headless-First Invariant)

1. **Headless-first.** Every command — existing and Phase 2 — must work without foreground activation, visible GUI, focus steal, or physical cursor movement (except for explicit mouse commands). This is enforced as an integration-test invariant: target window is NOT focused at test entry; `list-windows --focused-only` returns the same window before/after; cursor position unchanged for non-mouse commands.
2. **Skeleton traversal is platform-agnostic.** The novel progressive skeleton pattern (depth-3 clamp + `children_count` annotation + drill-down via `--root @ref` + scoped invalidation via `RefMap::remove_by_root_ref`) lives entirely in `crates/core/src/snapshot_ref.rs`. Windows adapter contributes ~50 LOC glue: `ControlViewWalker` (NOT `RawViewWalker` or `ContentViewWalker`) + `FindAll(TreeScope_Children, TrueCondition)` for `children_count` + fresh `UICacheRequest` per drill-down.
3. **Asymmetric event threading.** `watch_element` uses main-thread `AXObserver` on macOS (research-confirmed: Apple DTS says all AX is main-thread-only; AXSwift / Hammerspoon / Phoenix all do this); worker-thread MTA `IUIAutomation` event handler on Windows (Microsoft 2025 threading doc: UIA supports cross-thread event delivery).
4. **No `inventory` / `linkme` command registry.** Research confirmed neither survives link-GC reliably across ld64, ld-prime, GNU ld, lld, MSVC for cdylib consumers. Phase 2 uses `build.rs` filesystem enumeration of `crates/core/src/commands/*.rs` — deterministic, cdylib-safe, zero linker magic. The "one command per file" CLAUDE.md rule becomes the codegen contract.
5. **v0.1.14 prep release.** Ships `#[non_exhaustive]` on `ErrorCode` + `ad_abi_version()` + `ad_init(expected_major)` + `AD_RESULT_UNKNOWN` sentinel before any ABI break, giving consumers time to adapt. v0.2.0 then ships only unavoidable breaks (`PermissionStatus` tri-state, MSRV 1.82, new variants).
6. **`DeliverFiles` replaces `FileDrop`.** Headless-first forbids `NSDraggingSession` on macOS; the new action uses a 4-tier fallback (URL scheme → `NSWorkspace.open` with `activates: false` → pasteboard + `Cmd-V` → AppleScript). Windows keeps `IDataObject + DoDragDrop` (OLE drag is headless on Windows).

### Windows Engineering Invariants (from Phase 2 plan Unit 3)

1. `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` at startup.
2. `CoInitializeEx(NULL, COINIT_MULTITHREADED)` on main thread (UIA prefers MTA).
3. Never cache `IUIAutomationElement` across apartments.
4. UIA-first, SendInput-fallback (UIA patterns are focus-independent; `SendInput` is focus-dependent + UIPI-blocked for elevated targets).
5. `PostMessage WM_KEYDOWN` is DEAD for Chromium/UWP/games — not a viable alternative.
6. UIPI elevation detection via `GetTokenInformation(TokenIntegrityLevel)`. Ship `uiAccess=true` as optional signed release, not default.
7. `RemoveAutomationEventHandler` with post-remove-barrier pattern (Arc<Handler> outlives final callback dispatch).
8. HRESULT format in `platform_detail`: `COM HRESULT 0x80070005 (E_ACCESSDENIED: Access is denied)`.
9. `PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)` for legacy screenshot (mitigates DWM black frames). `windows-capture` (modern) handles composition correctly.
10. `ElementFromHandle(hwnd)` is headless-safe on any visible/minimized window — the foundation of observation headlessness.
11. `Windows.Graphics.Capture` requires DWM (Windows 10 1903+) in an interactive session; returns `PlatformNotSupported` in Session 0 / Server Core.
12. Session isolation: cannot drive windows in other user sessions.



Phase 2 brings agent-desktop to Windows. It is also the phase that closes the cross-platform feature-parity gaps surfaced after the v0.1.13 FFI ship — shipping Windows meaningfully requires new core abstractions (stable identifiers, event subscriptions, text-range primitives) that Windows UIA exposes natively and the macOS adapter currently does not surface. Every new trait method added here is implemented on both platforms in the same PR pair: Windows ships the native version, macOS backfills using the equivalent AX API. Linux (Phase 3) mirrors both against AT-SPI2.

Core engine, CLI parser, JSON contract invariants, and command-registration pattern are preserved. What Phase 2 legitimately changes: `AccessibilityNode` field set, `Action` enum variants, `ErrorCode` variants, `PlatformAdapter` trait size. Every change is additive (`#[non_exhaustive]` already guards the enums) and every macOS backfill lands atomically with the Windows implementation so the two platforms never drift.

Per the [Command Surface Architecture](#command-surface-architecture-dry-invariant) invariant, every new command added in Phase 2 (`watch`, `text select-range`, `text get-selection`, `text insert-at-caret`, etc.) lives in **exactly one file** under `crates/core/src/commands/` and auto-registers into the CLI, FFI, and MCP transports via `inventory::submit!`. The per-platform work is the three `PlatformAdapter` method implementations (one each in `crates/macos/`, `crates/windows/`, `crates/linux/`) — nothing repeats across transports.

P2-O16 (FFI parity expansion) also migrates the FFI wrappers from hand-written to codegen: a `build.rs` step in `crates/ffi/` walks the registry and emits one `ad_<name>` extern "C" function per `CommandDescriptor`, using the per-type marshaling helpers in `crates/ffi/src/convert/`. After this migration, the FFI crate holds marshaling primitives, not command wrappers. The `crates/mcp/` crate follows the same walk-the-registry pattern with `rmcp`'s `#[tool]` shape — so Phase 4 can ship its MCP server without hand-maintaining the tool list.

### Objectives

Core + Windows parity (original scope):

| ID | Objective | Metric |
|----|-----------|--------|
| P2-O1 | Windows adapter | `snapshot` on Windows returns valid tree for Explorer, Notepad, Settings |
| P2-O2 | All existing commands cross-platform | Identical JSON schema output on macOS and Windows for every command |
| P2-O3 | Windows input synthesis | `click`, `type`, `press`, all mouse commands working via UIA + SendInput |
| P2-O4 | Windows screenshot | `screenshot` produces PNG via `Windows.Graphics.Capture` API |
| P2-O5 | Windows clipboard | `clipboard-get` / `clipboard-set` / `clipboard-clear` working via Win32 Clipboard API |
| P2-O6 | Windows CI | GitHub Actions Windows runner executes full test suite on every PR |
| P2-O7 | Windows binary release | Prebuilt `.exe` published via GitHub Releases and npm; Phase 1.5 FFI cdylib for Windows already ships |

Cross-platform core extensions (new, landed alongside Windows):

| ID | Objective | Metric |
|----|-----------|--------|
| P2-O8 | `AccessibilityNode` stable-selector fields | Every node carries `identifier`, `subrole`, `role_description`, `placeholder`, `dom_id`, `dom_classes` (all `Option<String>` / `Vec<String>` with `skip_serializing_if`). Populated by Windows UIA `AutomationId` / `LocalizedControlType` / `HelpText`, by macOS `kAXIdentifierAttribute` / `kAXSubroleAttribute` / `kAXRoleDescriptionAttribute` / `kAXPlaceholderValueAttribute` / `kAXDOMIdentifierAttribute` / `kAXDOMClassListAttribute`. Snapshot regression fixtures show stable IDs across re-drills |
| P2-O9 | `Action` enum expansion for 2026 agent workloads | New variants: `LongPress { duration_ms }`, `ForceClick`, `ShowMenu`, `DeliverFiles(Vec<PathBuf>)` (renamed from `FileDrop` — the original name implied `NSDraggingSession` which is not headless-compatible on macOS; see Phase 2 plan §Headless-First Invariant and Unit 12), `WindowRaise`, `Cancel`, `SelectRange { start, len }`, `InsertAtCaret(String)`. `watch_element` is an adapter method, **not** an `Action` variant (plan §KD8 + origin brainstorm §D8). Each has a macOS AX API mapping (all AX calls on main thread per plan §KD9), a Windows UIA pattern mapping, and a new CLI subcommand. The `#[non_exhaustive]` attribute keeps this SemVer-safe; C-ABI consumers use `default:` / wildcard fallthrough plus the exported `AD_RESULT_UNKNOWN = -99` sentinel (plan §KD17) |
| P2-O10 | `ErrorCode` expansion | Add `PermissionRevoked` (distinct from `PermDenied` — TCC yanked mid-session), `ResourceExhausted` (refmap >1 MB, tree node-count cap), `AxMessagingTimeout` (AX-specific timeout separate from orchestration `Timeout`), `AutomationPermissionDenied` (macOS `osascript` grant). Tri-state permission probe at startup distinguishes "never granted" from "revoked" |
| P2-O11 | Event-subscription primitive (push, not poll) | New trait method `watch_element(handle, events: &[EventKind], timeout_ms: u64) -> Result<Vec<ElementEvent>>`. macOS: `AXObserverCreate` + `AXObserverAddNotification` + `CFRunLoopSource` (no more polling in `system/wait.rs`). Windows: `IUIAutomation.AddAutomationEventHandler` + `AddFocusChangedEventHandler` + `AddPropertyChangedEventHandler`. New `wait --event value-changed --ref @e5 --timeout 3000` CLI flag. Linux mirrors in Phase 3 via AT-SPI2 D-Bus signals |
| P2-O12 | Text range primitives | Read caret, read selection, select a range by offsets, read text at range, insert at caret. macOS: `kAXSelectedTextRangeAttribute` (settable), `AXStringForRangeParameterizedAttribute`, `AXBoundsForRangeParameterizedAttribute`, `AXRangeForLineParameterizedAttribute`, `AXValueCreate(kAXValueCFRangeType, …)`. Windows: `TextPattern.GetSelection`, `TextPattern.DocumentRange`, `TextRange.Select`, `TextRange.Move`, `TextRange.GetText`, `TextRange.GetBoundingRectangles`. Commands: `text get-selection`, `text select-range <ref> <start> <len>`, `text insert-at-caret <ref> <string>`, `text at-offset <ref> <start> <len>` |
| P2-O13 | Modern per-window screenshot APIs | macOS: replace `/usr/sbin/screencapture` subprocess with `SCScreenshotManager.captureImage(contentFilter:config:)` filtered to a specific `CGWindowID` from `SCShareableContent.windows`. Windows: `Windows.Graphics.Capture` via `GraphicsCaptureItem.CreateFromWindowHandle(HWND)` + `Direct3D11CaptureFramePool`. No subprocess, honours platform capture permission, ~10× faster for per-window captures |
| P2-O14 | Toolbar and missing surfaces | Both platforms add `SnapshotSurface::Toolbar`. macOS additionally adds `Spotlight` (pid of `/System/Library/CoreServices/Spotlight.app`), `Dock` (pid of `/System/Library/CoreServices/Dock.app`), and `MenuBarExtras` (enumerates `SystemUIServer`, `ControlCenter`, and per-app `AXExtrasMenuBar`). Windows adds `SystemTray` (as structured surface, not just tray commands) |
| P2-O15 | Electron / WebView2 deep-tree toggles | macOS: `build_subtree` writes `AXEnhancedUserInterface = YES` on app root for known Electron bundle IDs (VS Code, Cursor, Slack post-Sept-2024, Teams, Discord, Figma Desktop, Notion). Windows: detect Edge WebView2 via UIA `ClassName = "Chrome_WidgetWin_1"` and the equivalent flag; apply same web-wrapper depth-skip. Both: new `--force-electron-a11y` CLI override |
| P2-O16 | FFI registry migration + parity expansion | Migrate `crates/ffi/` from hand-written `ad_*` wrappers to a `build.rs` codegen step that walks the compile-time `CommandDescriptor` registry and emits one wrapper per command. After this, adding a CLI command automatically produces the FFI entry (plus its JSON Schema via `schemars` and its MCP tool in Phase 4). Marshaling helpers stay in `crates/ffi/src/convert/` — these are per-type, not per-command. In the same migration: backfill `ad_snapshot` (full refmap pipeline), `ad_execute_by_ref(adapter, "@e5", action, out)`, `ad_wait(…)`, `ad_version`, `ad_abi_version() -> u32` with `AD_ABI_VERSION_MAJOR` cbindgen `[defines]` export, `ad_status`, `ad_set_log_callback(fn(level, msg))` installing a `tracing_subscriber` layer so dlopen consumers see debug output |
| P2-O17 | Screen Recording / Automation permission detection (macOS backfill) | `check_permissions()` returns a richer `PermissionStatus` with a tri-state for AX, Screen Recording (`CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess`), and Automation (`AEDeterminePermissionToAutomateTarget`). Failures surface as `PermDenied` / `AutomationPermissionDenied` with concrete System Settings paths |

### Cross-Platform Trait Extensions

All methods land as `#[non_exhaustive]` additions in `crates/core/src/adapter.rs` with default implementations returning `AdapterError::not_supported(method)`. Windows implements them natively. macOS backfills in the same PR pair. Linux (Phase 3) adds the AT-SPI2 implementations.

```rust
impl PlatformAdapter for … {
    // P2-O11 — event subscription
    fn watch_element(
        &self,
        handle: &NativeHandle,
        events: &[EventKind],
        timeout: Duration,
    ) -> Result<Vec<ElementEvent>, AdapterError> { /* default: not_supported */ }

    // P2-O12 — text ranges
    fn get_text_selection(&self, handle: &NativeHandle) -> Result<TextSelection, AdapterError>;
    fn set_text_selection(&self, handle: &NativeHandle, range: TextRange) -> Result<(), AdapterError>;
    fn get_text_at(&self, handle: &NativeHandle, range: TextRange) -> Result<String, AdapterError>;
    fn insert_text_at_caret(&self, handle: &NativeHandle, text: &str) -> Result<(), AdapterError>;

    // P2-O13 — modern screenshot
    // (screenshot() gains a new `ScreenshotBackend::Modern` variant; platforms pick the
    //  native modern API; a `Legacy` fallback preserves the Phase 1 subprocess path.)

    // P2-O14 — new surfaces
    fn list_surfaces(&self, pid: i32) -> Result<Vec<SurfaceInfo>, AdapterError> // extended kinds
}
```

New supporting types (land in `crates/core/src/`):

- `EventKind` — `FocusChanged`, `ValueChanged`, `SelectionChanged`, `ChildrenChanged`, `WindowOpened`, `WindowClosed`, `MenuOpened`, `MenuClosed`, `NotificationPosted`, `ElementDestroyed`
- `ElementEvent` — `{ kind, handle_ref_id: Option<String>, timestamp, attr_snapshot: Option<AccessibilityNode> }`
- `TextRange` — `{ start: u32, length: u32 }` (UTF-16 code units to match both AX CFRange and UIA TextRange conventions)
- `TextSelection` — `{ range: TextRange, caret_offset: u32, lines_in_view: Vec<TextRange> }`
- `ScreenshotBackend` — `Modern` (ScreenCaptureKit / Windows.Graphics.Capture / PipeWire) or `Legacy` (preserves Phase 1 subprocess path as fallback for restricted environments)
- `PermissionStatus` extends to `{ accessibility: TriState, screen_recording: TriState, automation: TriState }` where `TriState = Granted | Denied { suggestion } | NotDetermined`

### Cross-platform capability map (P2-O8 through O17)

| Capability | macOS API | Windows API | Linux API (Phase 3) |
|------------|-----------|-------------|----------------------|
| Stable `identifier` | `kAXIdentifierAttribute` | UIA `AutomationId` | AT-SPI2 `accessible-id` + GTK `gtk-id` |
| `subrole` | `kAXSubroleAttribute` | UIA `LocalizedControlType` + pattern-based heuristic | AT-SPI2 `role-name` + `state-set` |
| `role_description` | `kAXRoleDescriptionAttribute` | UIA `LocalizedControlType` | AT-SPI2 `role-description` |
| `placeholder` | `kAXPlaceholderValueAttribute` | UIA `HelpText` + `IsTextEditPatternAvailable` placeholder | AT-SPI2 `description` + HTML `placeholder` via `object-attributes` |
| `dom_id` / `dom_classes` | `kAXDOMIdentifierAttribute` / `kAXDOMClassListAttribute` | Edge WebView2 UIA `HtmlId` / `HtmlClass` properties | AT-SPI2 `object-attributes` HTML keys |
| Event subscription | `AXObserverCreate` + `AXObserverAddNotification` on `CFRunLoop` | `IUIAutomation.AddAutomationEventHandler` + `AddFocusChangedEventHandler` + `AddPropertyChangedEventHandler` | AT-SPI2 D-Bus signals via `zbus::StreamFactory` |
| Text range read | `AXStringForRangeParameterizedAttribute` + `AXSelectedTextRangeAttribute` | `TextPattern.GetSelection`, `TextPattern.DocumentRange.GetText` | AT-SPI2 `Text.GetText(start, end)` + `Text.GetCaretOffset` |
| Text range write | `AXSelectedTextRange = AXValueCreate(kAXValueCFRangeType, …)` | `TextRange.Select` + `TextRange.Move` | AT-SPI2 `EditableText.InsertText` + `Text.SetCaretOffset` |
| Modern per-window screenshot | `SCScreenshotManager.captureImage(contentFilter:config:)` | `GraphicsCaptureItem.CreateFromWindowHandle` + `Direct3D11CaptureFramePool` | PipeWire `org.freedesktop.portal.ScreenCast` |
| Toolbar surface | `AXRole == AXToolbar` or `AXUnifiedTitleAndToolbar` | UIA `ControlType.ToolBar` | AT-SPI2 `Role::ToolBar` |
| Menu-bar extras surface | `SystemUIServer` + `ControlCenter` pid walk | UIA `Shell_TrayWnd` + `NotifyIconOverflowWindow` | AT-SPI2 `StatusNotifierWatcher` D-Bus |
| Dock / taskbar surface | `Dock.app` pid walk | UIA `Shell_TrayWnd` `TaskListButton` children | AT-SPI2 per-DE panel walk |
| `LongPress` | `CGEventCreateMouseEvent(…Down…)` + sleep + `…Up` | `SendInput` hold + release | Coordinate via `ydotool/xdotool` |
| `ForceClick` | `CGEventSetIntegerValueField(kCGMouseEventPressure, …)` + `kCGEventMouseSubtypeTabletPoint` | Pen input `SendInput` with `PEN_FLAGS_BARREL` | Not natively supported — return `ActionNotSupported` |
| `ShowMenu` action | `AXPerformAction(kAXShowMenuAction)` | `ExpandCollapsePattern.Expand` + UIA right-click fallback | AT-SPI2 `Action.DoAction("popup")` |
| `WindowRaise` | `AXUIElementSetAttributeValue(kAXRaiseAction)` | `SetForegroundWindow` + `SetWindowPos(HWND_TOP)` | `wmctrl -a` / `xdotool windowactivate` |
| `Cancel` | `AXPerformAction(kAXCancelAction)` | UIA `WindowPattern.Close` on dialog or `InvokePattern` on cancel button | AT-SPI2 `Action.DoAction("cancel")` or synthesize Escape |
| `DeliverFiles(Vec<PathBuf>)` | 4-tier headless fallback: (1) app-native URL scheme, (2) `NSWorkspace.open(urls:withApplicationAt:configuration:)` with `activates: false`, (3) `NSPasteboard.public.file-url` + `CGEventPostToPid(cmd+v)`, (4) `osascript open`. NEVER `NSDraggingSession` (not headless-compatible — Phase 2 plan Unit 12 research) | `IDataObject` + `DoDragDrop` via OLE (headless on Windows; OLE drag is window-server-native, unlike macOS AppKit-mediated drag) | XDND protocol via `xdotool` file-drag extension — Phase 3 |
| Screen Recording permission | `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess` | Implicit for `Windows.Graphics.Capture` — prompts on first use | PipeWire portal permission dialog |
| Automation permission | `AEDeterminePermissionToAutomateTarget` | N/A (no equivalent restriction) | N/A |

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
| Screenshot | `Windows.Graphics.Capture` | Modern per-window capture via `GraphicsCaptureItem.CreateFromWindowHandle` + `Direct3D11CaptureFramePool`. Prompts for capture permission on first use, no subprocess, respects DWM compositing. `BitBlt` / `PrintWindow` retained as `ScreenshotBackend::Legacy` fallback for pre-Windows-10 1903 environments |
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

### New Dependencies

| Crate | Version | Scope | Purpose |
|-------|---------|-------|---------|
| `uiautomation` | 0.24+ | Windows | UIA client wrapper, tree walker, patterns |
| `windows` | 0.62.2 | Windows | Raw Win32 / WinRT bindings for SendInput, clipboard, `Windows.Graphics.Capture`, D3D11 frame pool. Pinned to 0.62.2 to match `windows-capture 1.5.x`'s own pin. |
| `windows-capture` | 1.5.4 | Windows | Modern per-window screenshot via `Windows.Graphics.Capture` — headless in any interactive session. Replaces `PrintWindow + PW_RENDERFULLCONTENT` legacy fallback. |
| `screencapturekit` | 1.5 (crates.io) | macOS | Published crates.io canonical crate — the doom-fish fork is the maintained successor, NOT a git-SHA pin. |
| `objc2` | 0.5+ | macOS (new for P2-O13 / O17) | Safe bridging to `SCScreenshotManager`, `CGPreflightScreenCaptureAccess`, `NSFilePromiseProvider` — replaces ad-hoc `objc` message sends |
| `screencapturekit` | 0.3+ | macOS (new for P2-O13) | Thin wrapper around the `ScreenCaptureKit` framework (`SCShareableContent`, `SCScreenshotManager`) |

Added to `Cargo.toml` as target-gated dependencies:
```toml
[target.'cfg(target_os = "windows")'.dependencies]
agent-desktop-windows = { path = "crates/windows" }
uiautomation = "0.24"
windows = { version = "0.62.2", features = ["Win32_UI_Input", "Win32_UI_Input_KeyboardAndMouse", "Win32_System_Com", "Win32_System_DataExchange", "Win32_UI_WindowsAndMessaging", "Win32_Graphics_Gdi", "Graphics_Capture", "Win32_Graphics_Direct3D11"] }
windows-capture = "1.5.4"

[target.'cfg(target_os = "macos")'.dependencies]
objc2         = "0.5"
screencapturekit = "0.3"
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

**Cross-platform extension tests (P2-O8 through O17):**
- Stable-selector fields: every interactive node emits `identifier` on both platforms (UIA `AutomationId` on Windows, `AXIdentifier` on macOS) for a target app with known IDs (e.g. Calculator's `AutomationId` on Windows, a test harness app exporting `accessibilityIdentifier` on macOS)
- Event subscription: `watch --event value-changed --ref @e3 --timeout 2000` receives an event within 500 ms of a programmatic value change on both platforms
- Text ranges: `text select-range @e1 5 10` + `text get-selection @e1` round-trips to `{start:5, length:10}` on both platforms for a multi-line text editor (TextEdit / Notepad)
- Text insert-at-caret: `text insert-at-caret @e1 "hello"` produces matching `value` on both platforms with the caret advanced correctly
- Modern screenshot: `screenshot --window <id>` PNG matches a reference capture within SSIM threshold; cold latency <50 ms on both platforms (vs ~300 ms macOS subprocess baseline)
- Toolbar surface: `snapshot --surface toolbar` on Safari (macOS) and Edge (Windows) returns the toolbar's children with refs
- Electron deep-tree: VS Code snapshot with `--force-electron-a11y` exposes ≥100 refs at default depth on both platforms
- Screen Recording permission: on a macOS runner without Screen Recording, `screenshot --window` returns `PermDenied` with the Screen Recording suggestion (distinct from AX denial)
- Automation permission: on a macOS runner without Automation for a target app, `close-app` returns `AutomationPermissionDenied` rather than squeezing into `ActionFailed`

**FFI parity tests (P2-O16):**
- `ad_abi_version()` returns a packed `u32` matching the Cargo version; consumer built against 0.2.0 refuses to load 0.3.0
- `ad_snapshot` writes a refmap and the same `@e5` resolves via `ad_execute_by_ref` without a prior CLI snapshot on disk
- `ad_execute_by_ref(adapter, "@e5", AD_ACTION_KIND_CLICK, &out)` produces identical `AdActionResult` to `ad_resolve_element` + `ad_execute_action`
- `ad_set_log_callback` receives at least one `tracing::debug!` event during a `ad_get_tree` call
- Every new `Action` variant round-trips through the `AdAction.kind` i32 → Rust enum conversion without UB on arbitrary bit patterns (extends the existing `fuzz_arbitrary_bit_patterns_never_panic_across_all_enums` suite)

### CI

- Add GitHub Actions Windows runner alongside existing macOS runner
- Both runners execute: `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`
- `cargo tree -p agent-desktop-core` continues to contain zero platform crate names
- Binary size check: Windows `.exe` must be under 15MB

### Release

- [ ] Prebuilt Windows `.exe` binary added to the existing `.github/workflows/release.yml` `build` matrix (alongside the macOS CLI targets). Uses the same tarball + sha256 + attestation pipeline shipped in Phase 1.5.
- [ ] npm `postinstall.js` gains a `win32-x64` / `win32-arm64` branch so `npm install -g agent-desktop` works on Windows without changes to package shape.
- [ ] The Phase 1.5 FFI cdylib for Windows (`x86_64-pc-windows-msvc`) is already shipping; Phase 2 adds `aarch64-pc-windows-msvc` for ARM64 parity.
- [ ] Every new `ad_*` FFI entrypoint (P2-O16) is included in the `release-ffi` build and CI header drift check.
- [ ] GitHub Release notes document Windows support and installation.

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

## Phase 3 — Linux Adapter + Cross-Platform Parity Completion

**Status: Planned**

Phase 3 completes the three-platform story. The Linux adapter implements the original adapter surface **plus** every cross-platform extension landed in Phase 2 (event subscriptions, text ranges, modern screenshot, stable-selector fields, Toolbar surface, new Action variants, new ErrorCode variants). Each has a canonical AT-SPI2 / D-Bus / Wayland-portal implementation. Core engine, trait contract, command-registry, CLI dispatch, FFI wrappers, and MCP transport are all untouched — per the [Command Surface Architecture](#command-surface-architecture-dry-invariant) invariant, Phase 3 is **pure `PlatformAdapter` trait implementation code**, nothing else. No new command files, no CLI dispatch changes, no FFI wrappers, no MCP tool registrations.

### Objectives

Linux parity (original scope):

| ID | Objective | Metric |
|----|-----------|--------|
| P3-O1 | Linux adapter | `snapshot` on Ubuntu GNOME returns valid tree for Files, Terminal, Settings |
| P3-O2 | All commands cross-platform | Identical JSON schema output on all 3 platforms for every command |
| P3-O3 | Linux input synthesis | `click`, `type`, `press`, all mouse commands via AT-SPI actions + xdotool/ydotool |
| P3-O4 | Linux screenshot | `screenshot` produces PNG via PipeWire ScreenCast portal (Wayland) / XGetImage (X11) |
| P3-O5 | Linux clipboard | `clipboard-get` / `clipboard-set` / `clipboard-clear` via `wl-clipboard` (Wayland) / `xclip` (X11) |
| P3-O6 | Cross-platform CI | GitHub Actions matrix: macOS + Windows + Ubuntu |
| P3-O7 | Linux binary release | Prebuilt CLI binary added to the release pipeline (Phase 1.5 already ships the Linux FFI cdylib) |

Cross-platform extensions (Linux implementations of Phase 2 primitives):

| ID | Objective | Metric |
|----|-----------|--------|
| P3-O8 | Stable-selector fields on Linux | `AccessibilityNode.identifier` populated from AT-SPI2 `accessible-id` attribute (standard since AT-SPI 2.18) with GTK `gtk-id` / Qt `objectName` fallback; `dom_id` / `dom_classes` populated from AT-SPI2 `object-attributes` HTML keys (`id`, `class`) on `WebKitGTK` / `Chromium-Content` embeds |
| P3-O9 | AT-SPI2 event subscriptions (P2-O11 parity) | `watch_element` implemented via `zbus::Proxy::receive_signal` on AT-SPI2 signals: `org.a11y.atspi.Event.Object.PropertyChange`, `ChildrenChanged`, `StateChanged:focused`, `Window:Create`, `Window:Destroy`. Same `wait --event` CLI shape as macOS/Windows. Replaces polling in `crates/linux/src/system/wait.rs` before it's even written |
| P3-O10 | AT-SPI2 Text interface (P2-O12 parity) | Text range primitives via `org.a11y.atspi.Text` D-Bus methods: `GetText(start, end)`, `GetCaretOffset`, `SetCaretOffset`, `GetNSelections`, `GetSelection(n)`, `AddSelection(start, end)`, `RemoveSelection(n)`. `InsertAtCaret` uses `org.a11y.atspi.EditableText.InsertText(position, text, length)` |
| P3-O11 | PipeWire modern screenshot (P2-O13 parity) | `screenshot --window <id>` via `org.freedesktop.portal.ScreenCast` (Wayland) + `org.freedesktop.portal.RemoteDesktop` for capture permission flow. XDG desktop portal handles the user consent dialog exactly like `SCScreenshotManager` does on macOS. X11 fallback uses `XGetImage` for the lowest-permission path |
| P3-O12 | Toolbar + surfaces (P2-O14 parity) | `SnapshotSurface::Toolbar` via AT-SPI2 `Role::ToolBar`. Dock / taskbar surface via per-DE panel process walk (GNOME Shell process for gnome-shell extensions, Plasma `plasmashell` for KDE). StatusNotifierWatcher already scoped in the original Phase 3 tray spec |
| P3-O13 | Action variants on Linux (P2-O9 parity) | `Action::LongPress` via timed `xdotool/ydotool` button-hold; `Action::ShowMenu` via `org.a11y.atspi.Action.DoAction("popup")`; `Action::Cancel` via `Action.DoAction("cancel")` or Escape synthesis; `Action::FileDrop` via XDND (`xdotool key`+selection protocol on X11, portal-based FileTransfer on Wayland); `Action::ForceClick` returns `ActionNotSupported` on Linux (no pressure input primitive) |
| P3-O14 | FFI cdylib continues to ship | Phase 1.5 already publishes Linux FFI for x86_64 + aarch64; Phase 3 adds each new `ad_*` entrypoint's Linux implementation and extends the header drift check. No new FFI bindings to design — just implementations for the platform-specific methods under the existing trait |
| P3-O15 | Flatpak / Snap compatibility note | AT-SPI2 requires `--talk-name=org.a11y.Bus` permission inside sandboxed runtimes. Skill docs include the exact Flatpak override and Snap plug grants, so sandboxed consumers aren't silently empty-tree |

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
- Tray / StatusNotifierItem commands return identical JSON envelope structure across all 3 platforms

**Extension tests for P3-O8 through O15 (Linux-specific parity):**
- AT-SPI `accessible-id` populated for every interactive node in GNOME Calculator, GNOME Files, Firefox (with `ACCESSIBILITY_ENABLED=1`)
- `watch --event value-changed` via `zbus` signal subscription delivers an event within 500 ms for a programmatic value change in a test harness app (GTK4 + pygobject)
- `text select-range` / `get-selection` / `insert-at-caret` round-trips correctly in GNOME Text Editor via `org.a11y.atspi.Text` + `EditableText`
- PipeWire portal screenshot flow: user approves via XDG portal dialog, subsequent calls bypass the dialog within the session grant window; screenshot matches reference
- Toolbar surface: Firefox toolbar + GNOME Files toolbar both enumerate via `Role::ToolBar`
- Flatpak compatibility: a Flatpak-packaged GNOME Text Editor snapshot is non-empty when `--talk-name=org.a11y.Bus` is granted; returns clear diagnostic otherwise

### CI

- GitHub Actions matrix: macOS + Windows + Ubuntu (all three on every PR)
- All runners execute: `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`
- `cargo tree -p agent-desktop-core` continues to contain zero platform crate names
- Binary size check: all platform binaries must be under 15MB

### Release

- [ ] Prebuilt Linux CLI binary added to `.github/workflows/release.yml` matrix for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` (Phase 1.5 already builds the FFI cdylib for both triples on the same runners — Phase 3 reuses those runners)
- [ ] npm `postinstall.js` gains `linux-x64` / `linux-arm64` branches
- [ ] Every new `ad_*` Linux implementation from P3-O9 / O10 / O11 is covered by the existing FFI drift check + Sigstore attestation pipeline
- [ ] GitHub Release notes document Linux support, minimum glibc (2.35, Ubuntu 22.04 baseline), display-server requirements, and Flatpak/Snap compatibility

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

Phase 4 adds a new I/O layer. Core engine and all three platform adapters are unchanged. The MCP server wraps existing command logic in JSON-RPC tool definitions, enabling agent-desktop to work as an MCP-native desktop automation server for Claude Desktop, Cursor, VS Code Copilot, Gemini CLI, Microsoft Agent Framework 1.0, and any other MCP-compatible host.

By Phase 4 the CLI already covers 53+ commands on three platforms, the FFI ships as a shared library for in-process consumers, and the cross-platform event / text-range / stable-selector primitives from Phase 2 / 3 are in place. MCP mode is a **transport + discovery layer**, nothing more. Per the [Command Surface Architecture](#command-surface-architecture-dry-invariant) invariant at the top of this document, the MCP crate contains zero per-tool and zero per-platform code — it walks the same compile-time `inventory` registry the CLI and FFI use, and dispatches to the same `execute(args, adapter)` functions. New commands added in Phase 2 or Phase 5 (e.g. `watch_element`, `text select-range`, `find --visual`) become MCP tools automatically with no changes to `crates/mcp/`.

### Objectives

| ID | Objective | Metric |
|----|-----------|--------|
| P4-O1 | MCP server mode via `--mcp` | Responds to MCP `initialize` handshake, reports capabilities, per-host hello-world passes |
| P4-O2 | All commands as MCP tools | `tools/list` returns 53+ tools with JSON Schemas generated from the CLI arg structs via `schemars`; tool names prefixed `desktop_` |
| P4-O3 | Claude Desktop + Cursor + VS Code + Gemini CLI + MS Agent Framework validated | Each host invokes tools to control a desktop app end-to-end on all three platforms; repo ships `mcp.json` / `claude_desktop_config.json` / `.cursor/mcp.json` examples per host |
| P4-O4 | Tool annotations | `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint` on every tool; Claude Desktop surfaces destructive tools with a confirmation prompt |
| P4-O5 | Ref-based MCP tool shape (Playwright-MCP idiom) | Tools take `{ref: "e5"}` not raw `element_handle`, matching Playwright MCP so agents can swap between the two without relearning selectors. Tree snapshots return as MCP resources with refs inline |
| P4-O6 | MCP resource types | `agent-desktop://refmap/current`, `agent-desktop://snapshot/latest`, `agent-desktop://audit/{trace_id}` (audit log under Phase 5). `resources/list` + `resources/read` expose the current RefMap and last snapshot without re-running the command |
| P4-O7 | Tree-diff notifications | `watch_element` events (Phase 2 P2-O11) stream as MCP `notifications/message` during a long-running wait, so the host sees value-changed / focus-changed events as they happen rather than polling |
| P4-O8 | Progress notifications | `notifications/progress` for `wait`, `snapshot --skeleton` → `--root` drill-down chains, and large-tree traversals. Agents surface progress to users instead of hanging |
| P4-O9 | Tool-level permission tiers | Observation tools (`desktop_snapshot`, `desktop_find`, `desktop_get`, `desktop_is`, `desktop_list_*`) are freely callable. Interaction tools (`desktop_click`, `desktop_type_text`, `desktop_set_value`, `desktop_drag`) are gated behind an `interactive` capability negotiated at `initialize`. Destructive tools (`desktop_close_app`, `desktop_dismiss_all_notifications`) require the `destructive` capability plus the Phase 5 audit log |
| P4-O10 | Session-scoped RefMap | Each MCP session has its own in-memory RefMap keyed by `session_id` — no conflict with the on-disk CLI RefMap, no cross-session leakage when a host runs multiple agent-desktop-mcp instances |
| P4-O11 | MCP `initialize` returns tri-platform capability matrix | The `initialize` response declares platform (macOS / Windows / Linux), permission status (AX + Screen Recording + Automation tri-state from Phase 2 P2-O17), display-server (Linux), and the set of actually-supported tools given current permissions. A host can decide whether to prompt for missing permissions before the first tool call |
| P4-O12 | SSE + Streamable HTTP transports | Stdio remains primary. SSE (pre-March-2025 spec) and **Streamable HTTP** (post-March-2025 replacement) are implemented for remote scenarios — MS Agent Framework and future MCP hosts prefer the HTTP transport |

### Entry Point

The binary crate's `main.rs` detects mode:
- If invoked with `--mcp` or stdin is a pipe: enter MCP server mode
- Otherwise: parse CLI arguments, execute command, print JSON to stdout

This is the invariant: every MCP tool maps 1:1 to a CLI command. `agent-desktop snapshot --app Finder` is identical to invoking the MCP `desktop_snapshot` tool. Testing, debugging, and documentation are never fragmented.

### New Crate: `agent-desktop-mcp` (platform-agnostic, no per-command code)

The MCP crate is small and generic by design. It contains **zero per-tool files and zero per-platform code**. Per the Command Surface Architecture invariant at the top of this document, every CLI command auto-registers into a shared `inventory` registry; the MCP server iterates that registry at startup and exposes each entry as an MCP tool.

```
crates/mcp/src/
├── lib.rs              # mod declarations + re-exports
├── server.rs           # rmcp server bootstrap, initialize handler, walks the command registry
├── transport.rs        # stdio (primary), Streamable HTTP (P4-O12), SSE (legacy)
├── capability.rs       # P4-O9 tier gating (observation / interactive / destructive)
├── resources.rs        # P4-O6 resource types (refmap / snapshot / permissions / events / audit)
├── notifications.rs    # P4-O7 watch event forwarder, P4-O8 progress forwarder
└── schema.rs           # Translates CommandDescriptor → rmcp tool definition
```

That's the whole crate. It doesn't know what `desktop_click` does — it reads the `CommandDescriptor` registered by `crates/core/src/commands/click.rs` and forwards invocations through the same `execute(args, adapter)` function the CLI uses. Adding a command in Phase 2 (`text select-range`, `watch_element`) or Phase 5 (`find --visual`, `audit tail`) means **zero lines of MCP code** — the new command shows up automatically in `tools/list`.

### MCP tool registration — the one-time rewrite

```rust
// crates/mcp/src/server.rs  (illustrative, ~80 lines total for the crate)

pub async fn serve(adapter: Box<dyn PlatformAdapter>) -> Result<()> {
    let mut server = rmcp::ServerBuilder::new("agent-desktop", env!("CARGO_PKG_VERSION"));

    // Walk the compile-time registry. No hand-maintained tool list.
    for cmd in inventory::iter::<CommandDescriptor>() {
        // Skip tools disallowed by current permission set (P4-O11).
        if !cmd.available_under(&adapter.check_permissions()) { continue; }

        server.tool(rmcp::Tool {
            name: cmd.mcp_name,
            description: cmd.description,
            input_schema: (cmd.args_schema)(),       // schemars-derived
            annotations: cmd.annotations.into(),     // ReadOnlyHint etc.
        }, {
            let adapter = Arc::clone(&adapter);
            move |args: Value| async move {
                // Capability tier check (P4-O9).
                capability::gate(cmd, &session)?;
                // Invoke the same execute() the CLI uses.
                let value = (cmd.invoke)(args, adapter.as_ref())?;
                // Audit log entry (Phase 5 P5-O5).
                audit::record(cmd.mcp_name, &args, &value, session.trace_id);
                Ok(value)
            }
        });
    }

    server.run(stdio_transport()).await
}
```

### Tool Surface (what the registry produces)

Each MCP tool maps 1:1 to a CLI command via `CommandDescriptor`. Tool names are prefixed `desktop_` to avoid collision with other MCP servers. The tables below are a **snapshot of what the registry emits**, not hand-written entries. Adding a tool means adding a command file in `crates/core/src/commands/`; the tables refresh on regen.

Observation tools (always available):

| MCP Tool | CLI | Returns |
|----------|-----|---------|
| `desktop_snapshot` | `snapshot` | Tree + refmap in response; also published as `agent-desktop://snapshot/latest` resource |
| `desktop_find` | `find <query>` | Matching refs (array) |
| `desktop_get` | `get <prop> <ref>` | Property value |
| `desktop_is` | `is <state> <ref>` | Boolean |
| `desktop_list_windows` | `list-windows` | Array of windows |
| `desktop_list_apps` | `list-apps` | Array of apps |
| `desktop_list_surfaces` | `list-surfaces` | Array of surfaces (incl. Toolbar / Spotlight / Dock / MenuBarExtras from P2-O14) |
| `desktop_list_notifications` | `list-notifications` | Array of notifications |
| `desktop_screenshot` | `screenshot` | Base64 PNG (or MCP resource link) |
| `desktop_clipboard_get` | `clipboard-get` | Clipboard text |
| `desktop_permissions` | `permissions` | Tri-state permission report (AX + Screen Recording + Automation) |
| `desktop_status` | `status` | Daemon + adapter status |
| `desktop_version` | `version` | Version + ABI version |

Interaction tools (gated by `interactive` capability):

| MCP Tool | CLI | Shape |
|----------|-----|-------|
| `desktop_click` / `desktop_double_click` / `desktop_triple_click` / `desktop_right_click` | `click @e5` (and variants) | `{ref: "e5"}` |
| `desktop_type_text` | `type @e5 "hello"` | `{ref: "e5", text: "hello"}` |
| `desktop_set_value` | `set-value @e5 "hello"` | `{ref: "e5", value: "hello"}` |
| `desktop_clear` | `clear @e5` | `{ref: "e5"}` |
| `desktop_focus` | `focus @e5` | `{ref: "e5"}` |
| `desktop_select` / `desktop_toggle` / `desktop_check` / `desktop_uncheck` / `desktop_expand` / `desktop_collapse` | — | `{ref: "e5"}` (+ `value` for select) |
| `desktop_scroll` / `desktop_scroll_to` | `scroll <dir>` | `{ref: "e5", direction, amount}` |
| `desktop_press_key` / `desktop_key_down` / `desktop_key_up` | `press <keys>` | `{key, modifiers}` |
| `desktop_hover` / `desktop_drag` | `hover`/`drag` | `{ref: "e5"}` or `{from, to}` |
| `desktop_mouse_move` / `desktop_mouse_click` / `desktop_mouse_down` / `desktop_mouse_up` | — | `{x, y, button}` |
| `desktop_wait` | `wait --element / --window / --text / --menu / --notification` | `{condition, timeout_ms}` |
| `desktop_watch_element` (P2-O11) | `watch --event …` | `{ref: "e5", events: [EventKind], timeout_ms}` — streams via `notifications/message` |
| `desktop_launch_app` / `desktop_focus_window` / `desktop_resize_window` / `desktop_move_window` / `desktop_minimize` / `desktop_maximize` / `desktop_restore` | app / window ops | App / window args |
| `desktop_clipboard_set` / `desktop_clipboard_clear` | — | `{text}` / `{}` |
| `desktop_notification_action` | `notification-action <idx> <action>` | `{index, expected_app?, expected_title?, action}` (NC-reorder safe) |
| `desktop_text_select_range` / `desktop_text_get_selection` / `desktop_text_insert_at_caret` / `desktop_text_at_offset` (P2-O12) | `text …` subcommands | `{ref, start, length, text?}` |

Destructive tools (gated by both `interactive` and `destructive` capabilities; always write to the Phase 5 audit log):

| MCP Tool | CLI |
|----------|-----|
| `desktop_close_app` | `close-app <app> [--force]` |
| `desktop_dismiss_notification` | `dismiss-notification <idx>` |
| `desktop_dismiss_all_notifications` | `dismiss-all-notifications` |
| `desktop_batch` | `batch` — accepts destructive sub-commands, each evaluated against its own annotation |

### MCP Resource Types

Resources let hosts pull structured state without re-issuing a tool call:

| URI | Content | Update model |
|-----|---------|--------------|
| `agent-desktop://refmap/current` | JSON RefMap for the current MCP session (not the on-disk CLI refmap) | Replaced on every `desktop_snapshot` invocation; subscribable via `notifications/resources/updated` |
| `agent-desktop://snapshot/latest` | Last `desktop_snapshot` response as JSON (tree + refmap + metadata) | Same update model |
| `agent-desktop://permissions/current` | Tri-state permission report (AX, Screen Recording, Automation, display-server) | Refreshed on request; subscribable when Phase 2 P2-O17 permission observer is available |
| `agent-desktop://events/stream` | Merged `watch_element` event stream for the session | Real-time, subscribable |
| `agent-desktop://audit/{trace_id}` | Phase 5 append-only audit log entries for a trace | Growable; new entries as `notifications/resources/updated` |

### Framework Integration Targets

Every major 2026 MCP host gets a validated config example committed to `examples/mcp-hosts/`:

| Host | Config file | Transport | Notes |
|------|-------------|-----------|-------|
| Claude Desktop | `claude_desktop_config.json` | stdio | Already widespread; our reference host |
| Cursor | `.cursor/mcp.json` | stdio | Per-workspace config |
| VS Code (Copilot) | `.vscode/mcp.json` + `settings.json` | stdio | Copilot Chat 2026 adds MCP tool discovery |
| Gemini CLI | `~/.config/gemini-cli/mcp.json` | stdio | Google's first-party MCP integration |
| Microsoft Agent Framework 1.0 | `agentframework.yaml` MCP section | Streamable HTTP | Cloud-first host, requires HTTP transport (P4-O12) |
| Zed editor | `~/.config/zed/settings.json` | stdio | Desktop IDE with MCP-native agents |
| Continue.dev | `config.json` MCP section | stdio | OSS agent framework |

Each host gets a ~30-line config + a 60-second "hello agent" demo (launch Calculator → compute something → verify result) in the `examples/` directory as a runnable acceptance test.

### Transport

- **Stdio (primary):** MCP host spawns `agent-desktop --mcp` as a child process. JSON-RPC over stdin/stdout. Required; validated against all hosts in the Framework Integration table.
- **Streamable HTTP (P4-O12, required for MS Agent Framework):** Single HTTP endpoint at `POST /mcp` with chunked response streaming; replaces the pre-March-2025 SSE transport. Used when the host declares `transport: http` in its MCP config. Binds to `127.0.0.1` by default; `--mcp-bind <addr:port>` CLI flag overrides.
- **SSE (legacy):** Retained for hosts that haven't migrated to Streamable HTTP. Gated on `--mcp-transport sse`.
- **Session:** On `initialize`, detect platform, probe permissions (AX + Screen Recording + Automation tri-state), report tool capabilities given current permissions. Each MCP session has its own in-memory RefMap keyed by `session_id` — never reads or writes the on-disk CLI refmap at `~/.agent-desktop/last_refmap.json`. Sessions are isolated so the same host can run multiple agent-desktop-mcp instances concurrently without cross-contamination.

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

**Framework host acceptance tests (one per row in the Framework Integration table):**
- Claude Desktop: launch Calculator → snapshot → click buttons → verify result string via `desktop_get`
- Cursor: open a code file → snapshot editor → `desktop_text_insert_at_caret` a function → verify file content
- VS Code Copilot: same as Cursor on the VS Code host
- Gemini CLI: text-only interaction — list open windows, focus one, dismiss a notification
- Microsoft Agent Framework 1.0 (Streamable HTTP): HTTP-based MCP client runs the same Calculator demo against `http://127.0.0.1:<port>/mcp`
- Zed: editor-focused scenario (open file → select range → replace)
- Continue.dev: Claude Opus 4.7 with our server runs a 3-step canvas test in TextEdit

**Capability negotiation tests (P4-O9):**
- Host that negotiates only `observation` cannot invoke `desktop_click` — MCP error with clear `-32601 Method not found within capability set` message
- Host that negotiates `interactive` but not `destructive` cannot invoke `desktop_close_app`
- `initialize` response's `supported_tools` list shrinks correctly when AX permission is denied (only `desktop_permissions`, `desktop_version`, `desktop_status` remain)

**Event streaming tests (P4-O7):**
- `desktop_watch_element` subscription receives `notifications/message` events for a programmatic value change within 500 ms of the change on all three platforms
- Two concurrent watches on different refs get their events routed to the correct subscription ID

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

Phase 5 transforms agent-desktop from functional to enterprise-grade. Persistent daemon process, session isolation for concurrent agents, the safety trio required for enterprise and regulated deployments (dry-run + confirm + audit log), an OCR/vision fallback for custom-rendered UIs where the accessibility tree is empty, session tracing with OpenTelemetry-compatible event streams, and first-class distribution via native package managers.

### Objectives

| ID | Objective | Metric |
|----|-----------|--------|
| P5-O1 | Persistent daemon | Warm snapshot completes in <50ms (vs 200ms+ cold start) |
| P5-O2 | Session isolation | Two agents hold independent RefMaps without interference |
| P5-O3 | Enterprise quality gates | All gates in quality gates table pass |
| P5-O4 | Package manager distribution | Available via brew (macOS), winget/scoop (Windows), snap/apt (Linux) with Sigstore attestation verification on install |
| P5-O5 | Safety trio: `--dry-run` / `--confirm` / append-only audit log | Every destructive command supports `--dry-run` (resolves ref, computes the action, emits the would-be JSON response, does not execute), `--confirm` (stderr prompt with configurable timeout), and `~/.agent-desktop/audit.jsonl` append-only log with trace_id, actor, tool, args, decision (allowed / dry-run / denied / confirmed), exit code, timestamp. Covers EU AI Act Article 14 and OWASP Agentic Top-10 (2026) requirements |
| P5-O6 | Policy allowlist / denylist | `~/.agent-desktop/policy.yaml` defines per-tool rules — e.g. "never call `desktop_close_app` for `com.apple.finder`", "require confirm for any action on bundle ID `com.apple.mail`". Loaded at daemon start, reload-on-SIGHUP. Policy decisions land in the audit log |
| P5-O7 | OCR / vision fallback (`find --visual`) | When the AX tree is empty or the target isn't exposed (Canvas apps, Flutter-desktop, games, remote desktop, Figma plugins), `find --visual "label"` falls back to a per-window screenshot + OCR to locate text. macOS: `Vision` framework `VNRecognizeTextRequest`. Windows: `Windows.Media.Ocr.OcrEngine`. Linux: Tesseract via `tesseract` crate. Returns a synthetic ref that routes to coordinate events; clearly marked `source: "visual"` in output to signal reduced reliability |
| P5-O8 | Session trace + OpenTelemetry-compatible event stream | `--trace-id <uuid>` on every CLI invocation; generated if not provided. Each command appends structured events (command start, adapter call, action dispatched, exit) to `~/.agent-desktop/traces/{trace_id}.jsonl`. Events are OpenTelemetry-compliant (`span_id`, `trace_id`, `parent_span_id`, `span_name`, `attributes`). New `agent-desktop trace view <uuid>` pretty-prints; `trace export <uuid>` emits OTLP JSON |
| P5-O9 | Screencast / screenshot-per-action receipt | `--record-trace <path.mp4>` on long-running MCP sessions or CLI batches. Uses Phase 2 P2-O13 modern screenshot APIs at 2 Hz by default. Parity with Playwright 1.59 `page.screencast`. Mutually exclusive with `--dry-run` (nothing to record) |
| P5-O10 | Sigstore attestation verification at install time | `brew install` formula and `winget` manifest run `cosign verify-blob` / `gh attestation verify` against the downloaded tarball before installing. Prevents supply-chain tampering. apt/snap use distro-native signatures; the formula publishes both Sigstore bundle and the checksum |

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
| `trace view <uuid>` | Pretty-print a session trace from `~/.agent-desktop/traces/{uuid}.jsonl` |
| `trace export <uuid> [--otlp \| --har]` | Export a session trace as OpenTelemetry OTLP JSON or HAR for post-mortem inspection |
| `audit tail [--follow]` | Tail `~/.agent-desktop/audit.jsonl`, optionally streaming new entries |
| `audit verify <path>` | Verify the append-only integrity of an audit log (hash-chain check) |
| `policy check <command> <args…>` | Evaluate the policy file against a would-be command without executing |
| `find --visual "<label>"` | OCR-based visual fallback when the AX tree has no match for `label` (P5-O7) |
| Every command gains `--dry-run` | Resolve ref, compute action, emit the would-be response, **do not execute** (P5-O5) |
| Every destructive command gains `--confirm [--confirm-timeout <ms>]` | Prompt on stderr before executing; defaults off for CLI, on for MCP `destructive` capability |
| Every command gains `--trace-id <uuid>` | Correlate events; auto-generated when not provided (P5-O8) |
| Every command gains `--record-trace <path.mp4>` | Screencast while the command runs (P5-O9) |

### CLI-to-Daemon Migration

When daemon is running:
1. CLI command parses arguments as usual
2. Instead of directly calling the adapter, CLI connects to daemon socket
3. Sends serialized command to daemon
4. Daemon executes command in the caller's session context
5. Returns JSON response to CLI
6. CLI prints response to stdout

When daemon is not running, CLI falls back to direct execution (same as Phases 1-4). Daemon is purely an optimization, never a requirement.

### Safety Trio: `--dry-run` / `--confirm` / Audit Log (P5-O5)

Every destructive operation — `close-app`, `dismiss-all-notifications`, `set-value` (writes), `clear`, `drag`, `file-drop`, `notification-action`, `batch` containing any of the above — supports three layered safety primitives that compose:

1. **`--dry-run`** resolves refs, validates all inputs, evaluates the policy, computes the would-be `data` / `error` fields, and emits the normal JSON envelope with `dry_run: true` added. No adapter call happens. The ref stays valid for a subsequent non-dry-run invocation within the same snapshot.
2. **`--confirm`** prints a structured prompt to stderr:
   ```
   agent-desktop: destructive action requires confirmation
     command: close-app
     target:  Finder (bundle com.apple.finder)
     trace:   9f3c2a…
   Proceed? [y/N] (30s timeout)
   ```
   Defaults: CLI = off (opt-in), MCP `destructive` capability = on (opt-out via `skipConfirm: true` at init).
3. **Append-only audit log** at `~/.agent-desktop/audit.jsonl`:
   ```json
   {"ts":"2026-05-…","trace_id":"9f3c…","actor":"cli|mcp:claude-desktop","tool":"close-app","args":{"app":"Finder"},"policy_decision":"allowed","user_decision":"confirmed","exit":0,"prev_hash":"sha256:…","entry_hash":"sha256:…"}
   ```
   Hash-chained (Merkle-style) so `agent-desktop audit verify` detects tampering. File mode `0o600`, directory `0o700`. Rotated at 100 MB via `audit.jsonl.{N}.gz`.

Maps to real regulatory anchors: **EU AI Act Article 14 (human oversight + traceability)**, **OWASP Agentic Top-10 2026 AA-02 (human-in-the-loop) / AA-06 (audit trail)**. Shipping without the trio closes off enterprise adoption; shipping with it opens it.

### Policy Engine (P5-O6)

`~/.agent-desktop/policy.yaml`, loaded at daemon start, reloaded on `SIGHUP`:

```yaml
version: 1
rules:
  - match: { tool: close-app, bundle: com.apple.finder }
    decision: deny
    reason: "Finder is a system app — refusing."
  - match: { tool: set-value, bundle: com.apple.mail }
    decision: require-confirm
  - match: { trace_mcp_host: claude-desktop }
    decision: allow
  - default: allow
```

Matchers: `tool` (glob), `bundle` (exact or glob), `pid`, `trace_mcp_host` (`cli` / `mcp:<name>`), `ref_role`, `ref_name` (regex). Decisions: `allow` / `deny` / `require-confirm` / `dry-run-only`. Every evaluation writes to the audit log with the matched rule ID for post-mortem.

### OCR / Vision Fallback (P5-O7)

`find --visual "<label>"` closes the gap on apps that don't expose an accessibility tree (Figma plugins, Unity/Unreal games, Flutter-desktop apps, remote desktop clients, Canvas-based whiteboarding).

```
1. Capture the focused window via P2-O13 modern screenshot API.
2. Run OCR (platform-native, no extra runtime dep on macOS/Windows):
     macOS:  Vision.VNRecognizeTextRequest
     Windows: Windows.Media.Ocr.OcrEngine
     Linux:  Tesseract via the `tesseract` crate (libtesseract bundled)
3. Fuzzy-match the label against recognized text spans (Levenshtein ≤ 2).
4. Pick the highest-confidence hit; return a synthetic ref (`@v1`, `@v2`)
   that routes any subsequent action through coordinate-based input.
5. Tag the ref `source: "visual"` and downgrade confidence in the
   response so the agent knows it's acting on OCR not AX.
```

`STALE_REF` semantics stay the same — a visual ref invalidates on the next snapshot. Visual refs never cache in the refmap persisted to disk.

### Session Trace + OpenTelemetry (P5-O8)

Every command generates trace events written to `~/.agent-desktop/traces/{trace_id}.jsonl`:

```json
{"ts":"…","trace_id":"9f3c…","span_id":"…","parent_span_id":"…","name":"cli.snapshot","kind":"internal","attributes":{"app":"Finder","skeleton":true,"ref_count":14,"duration_ms":87}}
{"ts":"…","trace_id":"9f3c…","span_id":"…","parent_span_id":"<snapshot span>","name":"adapter.macos.get_tree","duration_ms":72,"attributes":{"surface":"window"}}
```

Spans are OpenTelemetry-compliant so `agent-desktop trace export <uuid> --otlp` emits a valid OTLP JSON payload ingestable by Grafana Tempo / Jaeger / Honeycomb / Datadog. `--har` exports a HAR-like envelope for quick manual inspection. Screencasts from `--record-trace` attach as trace links.

### Enterprise Quality Gates

| Gate | Requirement |
|------|-------------|
| Security | No arbitrary code execution. No privilege escalation. All actions allowlisted via `Action` enum. Daemon socket scoped to user. Policy engine denies by default when the policy file is syntactically invalid. |
| Safety | Every destructive command supports `--dry-run`; every MCP destructive tool requires the `destructive` capability + audit log; the audit log is hash-chained and tamper-detectable; policy engine evaluated on every invocation. |
| Performance | Cold start <200ms. Warm snapshot <50ms via daemon. Tree traversal timeout 5s default, configurable. `watch --event` latency <500ms (push, not poll) per P2-O11. |
| Reliability | Zero panics in non-test code. Graceful daemon recovery on crash. Stale socket cleanup on startup. FFI panic boundary in release-ffi profile (already shipping). |
| Observability | Every command writes a trace file. Daemon exports `/metrics` Prometheus endpoint when `--metrics` flag is on. OpenTelemetry OTLP export via `trace export --otlp`. |
| Compatibility | Tested against target app matrix: Finder, TextEdit, Xcode, VS Code, Chrome, Slack (macOS); Explorer, Notepad, Settings, VS Code, Edge (Windows); Nautilus, Terminal, Firefox, VS Code (Linux). |
| Distribution | Single binary per platform. No runtime dependencies for the CLI. FFI cdylib tarballs signed via Sigstore (already shipping as of Phase 1.5). Formula / manifest verify Sigstore attestation before installing (P5-O10). |
| Documentation | README, CLI reference, MCP reference, per-platform setup guides, troubleshooting, audit-log format reference, policy-file reference, OpenTelemetry trace schema. |
| FFI stability | Header drift check green on every PR. ABI version exported via `ad_abi_version()`. Pre-1.0: minor version bump for any public struct field add; major version bump for any removed or changed signature. |

### Performance Optimizations

| Optimization | Platform | Details |
|-------------|----------|---------|
| CacheRequest batching | Windows | Batch UIA attribute fetches via CacheRequest — reduces COM round-trips |
| Async tree walking | Linux | Parallel D-Bus calls for tree traversal — concurrent child fetching |
| Cached subtrees | All (daemon) | Reuse unchanged subtrees between snapshots in same session — skip re-traversal of stable UI regions |
| Warm adapter | All (daemon) | Adapter stays initialized between commands — skip COM init (Win), D-Bus connect (Linux), AX bootstrap (macOS) |
| Progressive skeleton drill | All | Skeleton overview + targeted drill-down reduces token consumption 78-96% for dense apps — fewer tokens per snapshot means more budget for actions |

### Package Manager Distribution

| Platform | Package Manager | Format | Install Command | Signing |
|----------|----------------|--------|-----------------|---------|
| macOS | Homebrew | Formula in `lahfir/homebrew-tap` | `brew install lahfir/tap/agent-desktop` | Sigstore `cosign verify-blob` against release tarball |
| Windows | winget | Manifest in `microsoft/winget-pkgs` | `winget install agent-desktop` | Sigstore attestation check via `gh attestation verify` |
| Windows | scoop | Manifest in `scoop-extras` bucket | `scoop install agent-desktop` | Sigstore attestation check |
| Linux | snap | Snap package on snapcraft.io | `snap install agent-desktop` | Snap-native signature (snapd-signed) |
| Linux | apt | `.deb` in custom PPA (`ppa:lahfir/agent-desktop`) | `apt install agent-desktop` | Debian-native `Release.gpg` signature |
| All | `cargo install` | crates.io (the CLI binary crate, not the workspace) | `cargo install agent-desktop` | Sigstore provenance on the crates.io release |

Each package manager distribution includes:
- Prebuilt binary for the target platform (matches `.github/workflows/release.yml` matrix output)
- Matching FFI cdylib tarball for consumers who want both the CLI and the library (Phase 1.5 artifacts)
- SHA256 checksum verification (unchanged from Phase 1)
- Sigstore build-provenance verification at install time (P5-O10) — formulas / manifests run `gh attestation verify` / `cosign verify-blob` before extracting
- Automatic PATH setup
- First-run Accessibility permission walkthrough (macOS) / UIA check (Windows) / AT-SPI bus check (Linux)
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
- brew formula installs and runs on macOS; `brew reinstall --debug agent-desktop` shows Sigstore verification log
- winget/scoop manifest installs and runs on Windows; manifest's `InstallerSuccessExitCodes` includes 0; Sigstore check in install script
- snap package installs and runs on Ubuntu; `--talk-name=org.a11y.Bus` permission requested
- apt `.deb` installs and runs on Ubuntu via PPA; `debsign` signature verified
- `cargo install agent-desktop` succeeds from crates.io with provenance attestation
- All packages produce correct `version` output including the ABI version
- All packages handle permissions correctly on their platform

**Safety trio tests (P5-O5, P5-O6):**
- `close-app Finder --dry-run` emits `{"data": {"would_close": "com.apple.finder"}, "dry_run": true}` and does not actually close
- `close-app Finder --confirm --confirm-timeout 2000` times out with `ErrorCode::Timeout` + audit entry `user_decision: timeout`
- Policy `deny` rule against `close-app` on `com.apple.finder` returns `PermDenied` with the matched rule ID; audit entry `policy_decision: deny`
- `audit verify` on a hand-edited `audit.jsonl` reports the exact tampered line
- `audit verify` on a legitimate append-only log passes cleanly
- Concurrent audit writes serialize correctly under `flock`-protected append

**OCR fallback tests (P5-O7):**
- `find --visual "Sign in"` on a Figma-plugin-style Canvas app returns a `@v1` synthetic ref; subsequent `click @v1` invokes coordinate-based input at the OCR hit center
- `find --visual` on an app with an accessibility tree falls back only when the AX search returns zero hits (does not shadow AX)
- OCR confidence threshold: below 0.6, return `ElementNotFound` rather than a low-confidence synthetic ref
- Visual refs never persist to disk refmap
- On Linux without Tesseract installed, `find --visual` returns `PlatformNotSupported` with the install command

**Session trace tests (P5-O8):**
- Every CLI invocation writes at least one span to `~/.agent-desktop/traces/{trace_id}.jsonl`
- `trace export <uuid> --otlp` produces a valid OpenTelemetry JSON payload that passes `otel-cli validate`
- A multi-command batch under a single `--trace-id` produces a single-rooted span tree (batch command is the parent)
- MCP sessions propagate the `trace_id` from the host's `initialize` params if provided; otherwise generate

**Install-time Sigstore tests (P5-O10):**
- Homebrew formula `install` step fails fast if the downloaded tarball's attestation fails verification
- Winget manifest includes a pre-install script that runs `gh attestation verify`
- Tampered tarball (bit-flip) reliably fails verification

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

### Command Surface DRYness (enforced across all phases)

See [Command Surface Architecture](#command-surface-architecture-dry-invariant) for the full layering. Summary of the invariant enforced on every PR:

- A new command creates exactly **one** file under `crates/core/src/commands/`.
- That file registers itself via `inventory::submit! { CommandDescriptor { … } }`.
- The CLI, FFI, and MCP transports each walk that registry at startup / build time; none of them hand-maintains a command list.
- Per-platform work is limited to the `PlatformAdapter` trait implementations in `crates/{macos,windows,linux}/` — never per-transport, never per-command.
- PRs that add a command to a single transport without updating the shared registry fail review. If a task in this document sounds like it requires per-transport duplication, it's a wording bug — the actual implementation follows the registry pattern.

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
| `windows` 0.62.2 | Phase 2 | Win32 / WinRT bindings (pinned to match `windows-capture 1.5` pin) |
| `windows-capture` 1.5.4 | Phase 2 | Modern `Windows.Graphics.Capture` screenshot |
| `objc2` 0.6 | Phase 2 | macOS safe Objective-C bridging (scoped to `system/screenshot.rs` + `system/permissions.rs`; CI grep guard) |
| `screencapturekit` 1.5 (crates.io) | Phase 2 | ScreenCaptureKit wrapper — published canonical crate, not git fork |
| `atspi` 0.28+ + `zbus` 5.x | Phase 3 | Linux AT-SPI2 client via D-Bus |
| `tokio` 1.x | Phase 3 | Async runtime (required by atspi/zbus) |
| `rmcp` 0.15.0+ | Phase 4 | Official MCP Rust SDK |
| `schemars` 1.2 | Phase 4 | JSON Schema generation for MCP tool parameters (deferred from Phase 2 per plan §KD15 — no Phase 2 consumer) |

### Explicitly NOT Added (research-rejected)

| Crate | Rejected at | Reason |
|-------|-------------|--------|
| `inventory` 0.3 | Phase 2 plan review | Link-GC unreliable across ld64, ld-prime, GNU ld, lld, MSVC for cdylib consumers. Research Topic B: `inventory::submit!` ctor sites are stripped when an rlib is linked into a binary that never references a symbol from that rlib. Replaced with `build.rs` filesystem enumeration. |
| `linkme` | Phase 2 plan review | Named linker sections have active Windows/lld-link edge cases (issues #70, #85, #114). Same reason as `inventory` rejection. |
| `xtask` workspace crate | Phase 2 plan review | Not needed once codegen is pure `build.rs`. Replaced with a tiny `build-helpers/` workspace crate holding the shared filesystem-enumeration function. |

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
| R8 | Ref instability confuses agents | Medium | High | Clear docs: refs are snapshot-scoped. `STALE_REF` error with recovery hint. Stable hashing in Phase 5. Progressive skeleton traversal with scoped invalidation provides a stable drill-down workflow for navigating complex UIs. **Phase 2**: stable-selector fields (`identifier`, `subrole`, `role_description`, `placeholder`, `dom_id`, `dom_classes` via `StableSelectors` flatten) + identifier-preferred resolver drop `STALE_REF` rate on Electron / localized apps. |
| R9 | Headless operation requirement | High | Critical | **Phase 2 plan §Headless-First Invariant** codifies the contract: UIA-first / AX-first paths for all actions; `NSDraggingSession` rejected for FileDrop (replaced by `DeliverFiles` 4-tier fallback); `ad_init()` version handshake; integration tests assert no focus steal and no cursor movement for non-mouse commands. |
| R10 | Command registry link-GC | Medium | High | Research Topic B confirmed `inventory`/`linkme` are unreliable across linkers for cdylib consumers. Resolved by pure `build.rs` filesystem enumeration — zero linker magic. |
| R11 | Skeleton traversal cross-platform | Low | High | Core is already platform-agnostic (`crates/core/src/snapshot_ref.rs`); Windows needs ~50 LOC glue (`ControlViewWalker` + `FindAll(TreeScope_Children, TrueCondition)` + fresh `UICacheRequest` per drill-down). Research Topic 4 confirmed `ElementFromHandle(hwnd)` is headless-safe. |
