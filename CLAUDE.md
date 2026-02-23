# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Commands

```bash
cargo build                                    # Debug build
cargo build --release                          # Release build (<15MB target)
cargo test --lib --workspace                   # Run all unit tests
cargo test --lib -p agent-desktop-core         # Test core crate only
cargo test --lib -p agent-desktop-macos        # Test macOS crate only
cargo test test_name                           # Run a single test by name
cargo clippy --all-targets -- -D warnings      # Lint (must pass, zero warnings)
cargo fmt --all -- --check                     # Format check
cargo fmt --all                                # Auto-format
cargo tree -p agent-desktop-core               # Verify no platform crate leaks (CI enforces)
```

Run the binary: `./target/release/agent-desktop snapshot --app Finder -i`

## Project Overview

Cross-platform Rust CLI + MCP server enabling AI agents to observe and control desktop applications via native OS accessibility trees.

## Git & Commits

- All commits are authored by **Lahfir**
- NEVER add `Co-Authored-By` lines, AI attribution badges, or "Generated with" footers
- NEVER include co-committers of any kind
- **Conventional Commits required.** Every commit message must use a type prefix:
  - `feat:` — new feature (triggers minor version bump)
  - `fix:` — bug fix (triggers patch version bump)
  - `feat!:` or `BREAKING CHANGE:` footer — breaking change (triggers major version bump)
  - `docs:` — documentation only
  - `style:` — formatting, no code change
  - `refactor:` — code change that neither fixes a bug nor adds a feature
  - `chore:` — maintenance tasks, dependencies
  - `ci:` — CI/CD changes
  - `test:` — adding or fixing tests
- Format: `type: concise imperative description` (lowercase type, no capital after colon)
- Focus on "why" not "what"
- Examples: `feat: add scroll-to command`, `fix: prevent stale ref on window resize`, `ci: add binary size check`

## Core Principle

agent-desktop is NOT an AI agent. It is a tool that AI agents invoke. It outputs structured JSON with ref-based element identifiers. The observation-action loop lives in the calling agent.

## Architecture

### Workspace Layout

```
agent-desktop/
├── Cargo.toml              # workspace: members, shared deps
├── rust-toolchain.toml     # pinned Rust version
├── clippy.toml             # project-wide lint config
├── crates/
│   ├── core/               # agent-desktop-core (platform-agnostic)
│   ├── macos/              # agent-desktop-macos (Phase 1)
│   ├── windows/            # agent-desktop-windows (stub → Phase 2)
│   └── linux/              # agent-desktop-linux (stub → Phase 2)
├── src/                    # agent-desktop binary (entry point)
│   ├── main.rs             # entry point, permission check, JSON envelope
│   ├── cli.rs              # clap derive enum (Commands)
│   ├── cli_args.rs         # all command argument structs
│   ├── dispatch.rs         # command dispatcher + parse helpers
│   └── batch_dispatch.rs   # batch command execution
└── tests/
    ├── fixtures/           # golden JSON snapshots
    └── integration/        # macOS CI integration tests
```

### Dependency Inversion (Non-Negotiable)

- `agent-desktop-core` defines the `PlatformAdapter` trait and all shared types
- Platform crates (`macos`, `windows`, `linux`) implement the trait
- **Core NEVER imports platform crates.** Platform crates NEVER import each other.
- The binary crate (`src/`) is the only place that wires platform → core
- CI enforces this: `cargo tree -p agent-desktop-core` must contain zero platform crate names

### Platform Selection

Compile-time via `#[cfg(target_os)]` in `build_adapter()`. Agents never specify platform — `agent-desktop snapshot -i` works identically on macOS, Windows, and Linux.

```rust
fn build_adapter() -> impl PlatformAdapter {
    #[cfg(target_os = "macos")]
    { agent_desktop_macos::MacOSAdapter::new() }

    #[cfg(target_os = "windows")]
    { agent_desktop_windows::WindowsAdapter::new() }

    #[cfg(target_os = "linux")]
    { agent_desktop_linux::LinuxAdapter::new() }
}
```

### Target-Gated Dependencies

Binary crate `Cargo.toml` uses platform-specific deps, NOT unconditional deps with `#[cfg]` in source:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
agent-desktop-macos = { path = "crates/macos" }

[target.'cfg(target_os = "windows")'.dependencies]
agent-desktop-windows = { path = "crates/windows" }

[target.'cfg(target_os = "linux")'.dependencies]
agent-desktop-linux = { path = "crates/linux" }
```

### Command Dispatch

Direct `match` in the binary crate. No `Command` trait, no `CommandRegistry`. Each command is a standalone `execute()` function under `crates/core/src/commands/`.

```rust
pub fn dispatch(cmd: Commands, adapter: &dyn PlatformAdapter) -> Result<serde_json::Value, AppError> {
    match cmd {
        Commands::Snapshot(args) => commands::snapshot::execute(args, adapter),
        Commands::Click(args) => commands::click::execute(args, adapter),
        // one arm per command
    }
}
```

### Additive Phase Model

- **Phase 1:** Foundation + macOS MVP (30 commands, core engine, macOS adapter)
- **Phase 2:** Windows + Linux adapters, 10+ new commands — core untouched
- **Phase 3:** MCP server mode via `--mcp` flag — wraps existing commands
- **Phase 4:** Daemon, sessions, enterprise quality gates

Phases 2–4 add adapters/transports/hardening. Nothing in core is rebuilt.

## Coding Standards

### File Rules

- **400 LOC hard limit per file.** If approaching 400, split by responsibility. No exceptions.
- **No inline comments.** Code must be self-documenting through naming. Only Rust doc-comments (`///`) on public items when the name alone is insufficient.
- **One struct/enum per file** for domain types. `node.rs` defines `AccessibilityNode`. `action.rs` defines `Action`.
- **One command per file.** Each CLI command lives in its own file under `commands/`. Filename matches the command name.
- **No God objects.** No struct with more than 7 fields. No function with more than 5 parameters. Use builder patterns or config structs.
- **Explicit pub boundaries.** Only `lib.rs` re-exports public items. Internal modules use `pub(crate)`. No wildcard re-exports.

### Error Handling

- **Zero `unwrap()` in non-test code.** All `Result`s propagated with `?` or matched explicitly. Panics are test-only.
- Every error carries: `ErrorCode` enum (machine-readable), `message: String` (human-readable), `suggestion: Option<String>` (recovery hint), `platform_detail: Option<String>` (OS-specific detail)
- All platform adapter functions return `Result<T, AdapterError>`
- All command handlers return `Result<serde_json::Value, AppError>`
- The binary's `main()` converts `AppError` to JSON and sets the exit code

### Error Codes

```
PERM_DENIED, ELEMENT_NOT_FOUND, APP_NOT_FOUND, ACTION_FAILED,
ACTION_NOT_SUPPORTED, STALE_REF, WINDOW_NOT_FOUND,
PLATFORM_NOT_SUPPORTED, TIMEOUT, INVALID_ARGS, INTERNAL
```

### Exit Codes

- `0` — success
- `1` — structured error (JSON with error code)
- `2` — argument/parse error

### Naming Conventions

| Element | Convention | Example |
|---------|-----------|---------|
| Crate names | `agent-desktop-{name}` | `agent-desktop-core`, `agent-desktop-macos` |
| Module files | `snake_case`, singular | `snapshot.rs`, `list_windows.rs` |
| Structs | PascalCase, descriptive noun | `SnapshotEngine`, `RefAllocator` |
| Traits | PascalCase, adjective/capability | `PlatformAdapter`, `Executable` |
| Enums | PascalCase, variants PascalCase | `Action::Click`, `ErrorCode::PermDenied` |
| Functions | `snake_case`, verb-first | `build_tree()`, `allocate_refs()` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_TREE_DEPTH`, `DEFAULT_TIMEOUT_MS` |
| CLI flags | kebab-case | `--max-depth`, `--include-bounds` |
| Ref IDs | `@e{n}` sequential | `@e1`, `@e2`, `@e14` |

### Platform Crate Folder Structure

All platform crates (`macos`, `windows`, `linux`) follow an identical subfolder layout. New files must be placed in the correct subfolder.

```
crates/{macos,windows,linux}/src/
├── lib.rs              # mod declarations + re-exports only
├── adapter.rs          # PlatformAdapter trait impl (~175 LOC)
├── tree/               # Reading & understanding the UI
│   ├── mod.rs          # re-exports
│   ├── element.rs      # AXElement struct + attribute readers
│   ├── builder.rs      # build_subtree, tree traversal
│   ├── roles.rs        # Role mapping
│   ├── resolve.rs      # Element re-identification
│   └── surfaces.rs     # Surface detection
├── actions/            # Interacting with elements
│   ├── mod.rs          # re-exports
│   ├── dispatch.rs     # perform_action match arms
│   ├── activate.rs     # Smart AX-first activation chain
│   └── extras.rs       # select_value, ax_scroll
├── input/              # Low-level OS input synthesis
│   ├── mod.rs          # re-exports
│   ├── keyboard.rs     # Key synthesis, text typing
│   ├── mouse.rs        # Mouse events
│   └── clipboard.rs    # Clipboard get/set
└── system/             # App lifecycle, windows, permissions
    ├── mod.rs          # re-exports
    ├── app_ops.rs      # launch, close, focus
    ├── window_ops.rs   # window operations
    ├── key_dispatch.rs # app-targeted key press
    ├── permissions.rs  # permission checks
    ├── screenshot.rs   # screen capture
    └── wait.rs         # wait utilities
```

**Placement rules:**
- Tree reading/traversal/resolution → `tree/`
- Element interaction/activation → `actions/`
- Raw OS input (keyboard, mouse, clipboard) → `input/`
- App lifecycle, windows, permissions, screenshots → `system/`
- `adapter.rs` stays at root — it's the PlatformAdapter impl that wires everything together

### Extensibility Pattern

Adding a new command requires exactly these steps:
1. Create `crates/core/src/commands/{name}.rs` with an `execute()` function
2. Register it in `crates/core/src/commands/mod.rs`
3. Add the CLI subcommand variant to `src/cli.rs` (clap derive enum)
4. Add a match arm in `dispatch()` in the binary crate
5. If new `Action` variant needed, add to `crates/core/src/action.rs`
6. If new adapter method needed, add to `PlatformAdapter` trait with a default returning `Err(AdapterError::not_supported())`

No existing files are modified beyond the registration points. Enforce via code review.

## JSON Output Contract

Every command produces a response envelope:

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

Error responses:

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

### Serialization Rules

- Omit null/None fields (`#[serde(skip_serializing_if = "Option::is_none")]`)
- Omit empty arrays (`#[serde(skip_serializing_if = "Vec::is_empty")]`)
- Omit bounds in compact mode
- `ref_count` and `tree` go inside `data`, not as top-level siblings

## Ref System

- Refs allocated in depth-first document order: `@e1`, `@e2`, etc.
- Only interactive roles receive refs: `button`, `textfield`, `checkbox`, `link`, `menuitem`, `tab`, `slider`, `combobox`, `treeitem`, `cell`
- Static text, groups, containers do NOT get refs (they remain in tree for context)
- Refs are deterministic within a snapshot but NOT stable across snapshots if UI changed
- RefMap stored at `~/.agent-desktop/last_refmap.json` with `0o600` permissions, directory at `0o700`
- Each snapshot REPLACES the refmap file entirely (atomic write via temp + rename)
- Action commands use optimistic re-identification: `(pid, role, name, bounds_hash)`. Return `STALE_REF` on mismatch.

## PlatformAdapter Trait

12 methods with default implementations returning `not_supported()`:

```rust
pub trait PlatformAdapter: Send + Sync {
    fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError>;
    fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError>;
    fn get_tree(&self, win: &WindowInfo, opts: &TreeOptions) -> Result<AccessibilityNode, AdapterError>;
    fn execute_action(&self, handle: &NativeHandle, action: Action) -> Result<ActionResult, AdapterError>;
    fn resolve_element(&self, entry: &RefEntry) -> Result<NativeHandle, AdapterError>;
    fn check_permissions(&self) -> PermissionStatus;
    fn focus_window(&self, win: &WindowInfo) -> Result<(), AdapterError>;
    fn launch_app(&self, id: &str, wait: bool) -> Result<WindowInfo, AdapterError>;
    fn close_app(&self, id: &str, force: bool) -> Result<(), AdapterError>;
    fn screenshot(&self, target: ScreenshotTarget) -> Result<ImageBuffer, AdapterError>;
    fn get_clipboard(&self) -> Result<String, AdapterError>;
    fn set_clipboard(&self, text: &str) -> Result<(), AdapterError>;
}
```

## Key Types

- `AccessibilityNode` — platform-agnostic tree node: `ref`, `role`, `name`, `value`, `description`, `states`, `bounds`, `children`
- `Action` — Click, DoubleClick, RightClick, SetValue(String), SetFocus, Expand, Collapse, Select(String), Toggle, Scroll(Direction, Amount), PressKey(KeyCombo)
- `NativeHandle` — opaque platform pointer with `PhantomData<*const ()>` to prevent auto-Send/Sync. Inner field is `pub(crate)`.
- `RefEntry` — `{ pid, role, name, bounds_hash, available_actions }`
- `WindowInfo` — `{ id, title, app_name, pid, bounds }`
- `ErrorCode` — 11-variant enum with `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`
- `AdapterError` — struct with `code`, `message`, `suggestion`, `platform_detail`
- `AppError` — enum with `#[from]` impls for `AdapterError`, `std::io::Error`, `serde_json::Error`

## macOS Adapter (Phase 1)

### Tree Traversal
- Entry: `AXUIElementCreateApplication(pid)` for app root
- Children: `kAXChildrenAttribute` recursively with **ancestor-path set** (not global visited set — macOS reuses AXUIElementRef pointers across sibling branches)
- **Use `AXUIElementCopyMultipleAttributeValues`** for batch attribute fetch (3-5x faster)
- Role mapping: AXRole strings → unified role enum in `tree/roles.rs`
- Max depth default: 10. Configurable via `--max-depth`

### Action Execution
- Click: `AXUIElementPerformAction(kAXPressAction)`
- SetValue: `AXUIElementSetAttributeValue(kAXValueAttribute, value)`
- SetFocus: `AXUIElementSetAttributeValue(kAXFocusedAttribute, true)`
- Keyboard/Mouse: `CGEventCreateKeyboardEvent` / `CGEventCreateMouseEvent`
- Clipboard: `NSPasteboard.generalPasteboard` via Cocoa FFI
- Screenshot: `CGWindowListCreateImage`

### Permission Detection
- Call `AXIsProcessTrusted()` on startup
- If false, return `PERM_DENIED` with guidance: "Open System Settings > Privacy > Accessibility and add your terminal"
- Optionally call `AXIsProcessTrustedWithOptions(prompt: true)` to trigger system dialog

### AXElement Safety
- Inner field: `pub(crate)` not `pub` (prevents double-free via raw pointer extraction)
- `Clone` impl must call `CFRetain`
- `Drop` impl must call `CFRelease`

## Testing Strategy

### Unit Tests (core)
- `AccessibilityNode` ser/de roundtrips
- Ref allocator only assigns interactive roles
- `SnapshotEngine` filtering
- Error serialization
- MockAdapter: in-memory `PlatformAdapter` returning hardcoded trees

### Golden Fixtures (`tests/fixtures/`)
- Real snapshots from Finder, TextEdit, etc. checked into repo
- Regression-test serialization format changes

### Integration Tests (macOS CI)
- Snapshot Finder, TextEdit, System Settings — non-empty trees with refs
- Click button in test app — verify action succeeded
- Type text into TextEdit via ref — verify content changed
- Clipboard get/set roundtrip
- Permission denied scenario — correct error code and guidance
- Large tree (Xcode) snapshot in under 2 seconds

## Dependencies (Phase 1)

| Crate | Version | Purpose |
|-------|---------|---------|
| clap | 4.x | CLI parsing with derive macros |
| serde + serde_json | 1.x | JSON serialization |
| thiserror | 2.x | Error derive macros |
| tracing | 0.1+ | Structured logging |
| base64 | 0.22+ | Screenshot encoding |
| accessibility-sys | 0.1+ | macOS AXUIElement FFI |
| core-foundation | 0.10+ | macOS CF types |
| core-graphics | 0.24+ | macOS CG types |

### Deferred Dependencies
- `tokio` — Phase 2/3 (all Phase 1 ops are synchronous)
- `rmcp` (0.15.0) — Phase 3 (MCP server)
- `schemars` — Phase 3 (JSON Schema generation)
- `uiautomation` (0.24+) — Phase 2 (Windows)
- `atspi` (0.28+) + `zbus` (5.x) — Phase 2 (Linux)

## Build Configuration

```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

Target binary size: <15MB per platform.

## CI Requirements

- GitHub Actions macOS runner executes full test suite on every PR
- `cargo tree -p agent-desktop-core` must not contain platform crate names
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --workspace`
- Binary size check: fail if release binary exceeds 15MB

## Implemented Commands (50)

| Category | Commands |
|----------|----------|
| App/Window (10) | `launch`, `close-app`, `list-windows`, `list-apps`, `focus-window`, `resize-window`, `move-window`, `minimize`, `maximize`, `restore` |
| Observation (6) | `snapshot`, `screenshot`, `find`, `get`, `is`, `list-surfaces` |
| Interaction (14) | `click`, `double-click`, `triple-click`, `right-click`, `type`, `set-value`, `clear`, `focus`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse` |
| Scroll (2) | `scroll`, `scroll-to` |
| Keyboard (3) | `press`, `key-down`, `key-up` |
| Mouse (5) | `hover`, `drag`, `mouse-move`, `mouse-click`, `mouse-down`, `mouse-up` |
| Clipboard (3) | `clipboard-get`, `clipboard-set`, `clipboard-clear` |
| Wait (1) | `wait` (with `--element`, `--window`, `--text`, `--menu` flags) |
| System (3) | `status`, `permissions`, `version` |
| Batch (1) | `batch` |

## Non-Goals

- Does NOT embed or invoke LLMs
- Does NOT provide a GUI, TUI, or interactive prompt — machine-facing only
- Does NOT automate web browsers (use agent-browser for that)
- Does NOT record or replay macros (stateless per invocation until Phase 4 daemon)
- Does NOT work with custom-rendered or game-engine UIs lacking accessibility exposure

## Reference Documents

- PRD v2.0: `docs/agent_desktop_prd_v2.pdf`
- Architecture Brainstorm: `docs/brainstorms/2026-02-19-architecture-validation-brainstorm.md`
- Phase 1 Plan: `docs/plans/2026-02-19-feat-agent-desktop-phase1-foundation-plan.md`
