---
title: "feat: agent-desktop Phase 1 — Foundation + macOS MVP"
type: feat
date: 2026-02-19
phase: 1
duration_weeks: 10
deepened: 2026-02-19
---

# feat: agent-desktop Phase 1 — Foundation + macOS MVP

## Enhancement Summary

**Deepened on:** 2026-02-19 (round 1), 2026-02-19 (round 2 — code quality audit)
**Research agents used:** 15 (Architecture Strategist, Performance Oracle, Security Sentinel, Code Simplicity Reviewer, Agent-Native Reviewer, Pattern Recognition Specialist, Best Practices Researcher, Framework Docs Researcher, Spec Flow Analyzer, Agent-Native Architecture Skill, Dead Code/LOC Auditor, Test-Before-Implement Researcher, Modular Architecture Researcher, Code Quality Reviewer, Architecture Extensibility Reviewer)
**Context7 queries:** clap 4 derive patterns, serde performance patterns
**Web research:** macOS AX FFI best practices, cargo-dist distribution, rmcp latest version, accessibility-sys alternatives, thiserror 2.0 changes

### Round 2 Structural Findings (Fix During Implementation)

- **Dead code:** `clipboard.rs` (zero callers), `batch::execute` stub (never called)
- **LOC violation:** `tree.rs` at 403 lines — split into `ax_element.rs` / `ax_attrs.rs` / `ax_tree.rs`
- **Dispatch duplication:** 187/397 lines in `dispatch.rs` are a near-verbatim parallel dispatch table — collapse to single code path via `Commands` enum deserialization
- **Probe binaries:** `axprobe.rs` / `axprobe2.rs` in `src/bin/` compile on every build — move to `examples/` with `required-features = ["dev-tools"]`
- **Bugs:** `wait.rs` silently discards RefMap load errors; `is_check.rs` reads stale state; `press.rs` undocumented null-handle convention

### Blockers (Fix Before Writing Code)

1. **Circular dependency in CommandRegistry** — `CommandRegistry::dispatch` references `crate::cli::Commands` from the binary crate, creating a circular dep (core ↔ binary). **Fix:** Drop `Command` trait and `CommandRegistry` entirely; dispatch via `match` in the binary crate.
2. **Missing `resolve_handle` on PlatformAdapter** — `click.rs` calls `adapter.resolve_handle(entry)` but the method does not exist on the 13-method trait. **Fix:** Add `fn resolve_element(&self, entry: &RefEntry) -> Result<NativeHandle, AdapterError>` as the 14th trait method.
3. **`AppError` type referenced but never defined** — `command.rs` and `click.rs` return `AppError` but only `AdapterError` and `ErrorCode` are defined in `error.rs`. **Fix:** Define `AppError` enum with `#[from]` impls for `AdapterError`, `std::io::Error`, `serde_json::Error`.
4. **`unwrap()` in `refmap_path()`** — Violates the plan's own "zero unwrap in non-test code" invariant. Panics if `HOME` is unset. **Fix:** Return `Result`, propagate error.

### Critical Improvements

5. **Use `AXUIElementCopyMultipleAttributeValues`** for batch attribute fetch (3-5x faster tree traversal, likely required to hit 2s Xcode target).
6. **Remove `IsTerminal` stdin detection** — Piped input (`< /dev/null`) falsely triggers MCP mode. Use only `--mcp` flag.
7. **Atomic RefMap writes** — Use temp-file + `rename()` to prevent corruption on concurrent access.
8. **File permissions** — RefMap at `0o600`, directory at `0o700`. Currently world-readable.
9. **`AXElement` inner field** — Change from `pub` to `pub(crate)` to prevent double-free via raw pointer extraction.
10. **NativeHandle soundness** — Add `PhantomData<*const ()>` to opt out of auto-`Send`/`Sync`.

### Key Simplifications (YAGNI)

11. **Delete:** `Command` trait, `CommandRegistry`, `command.rs`, `crates/mcp/` stub, `--mcp` flag, token estimation, `manage_window`, `synthesize_input`.
12. **Defer:** `schemars` and `schemas/` to Phase 3 (no consumer in P1). `tokio` to Phase 2/3 (all P1 ops are sync).
13. **Simplify:** `Response<T>` → `Response` with `serde_json::Value`. Entry point from ~40 lines to ~15 lines. Pipeline from 5 stages to 3.

### Version Corrections

14. **rmcp is at 0.15.0** (February 2026), NOT 0.8+ as PRD claimed. Now the official Rust SDK at `modelcontextprotocol/rust-sdk`.
15. **core-foundation** may be at 0.10.0 (plan says 0.9). Verify with `cargo search`.
16. **core-foundation-sys** is at 0.8.x (plan says 0.9). Version mismatch between the two crates.
17. Add `panic = "abort"` to release profile (200-500KB smaller binary).

### Agent-Native Gaps

18. **Add `batch` command** — Single process invocation for observe-act-verify sequences. Eliminates N startup costs.
19. **Fix snapshot envelope** — `ref_count`/`tree` must be inside `data`, not top-level siblings of `version`/`ok`.
20. **Post-action state in responses** — Every action command should return the element's state after the action (eliminates verify round-trip).
21. **Exit code contract** — 0=success, 1=structured error, 2=argument error.
22. **Default `snapshot` to focused window** of frontmost app when no args given.

---

## Overview

Build the complete vertical for `agent-desktop`: a Rust CLI + MCP server that enables
AI agents to observe and control desktop applications via native OS accessibility trees.
Phase 1 targets macOS exclusively, delivers 30 production-ready commands, establishes
every shared abstraction, and ships a binary distribution via cargo-dist.

Phases 2–4 are strictly additive: new platform adapters, new transport, new
hardening — nothing in core is rebuilt.

**Source:** PRD v2.0 (February 2026) + Architecture Brainstorm 2026-02-19.

---

## Architecture Decisions

From the brainstorm session (`docs/brainstorms/2026-02-19-architecture-validation-brainstorm.md`):

| Decision | Choice | Rationale |
|---|---|---|
| `PlatformAdapter` trait | Unified (13 methods) | Command extensibility never touches the trait |
| NativeHandle persistence | Optimistic + `STALE_REF` | Store `(pid, role, name, bounds_hash)`; return `STALE_REF` on mismatch |
| Test strategy | MockAdapter + golden fixtures | Offline unit tests + output contract regression |
| Workspace bootstrap | All platform crates from day one | Enforces adapter boundary at compile time |

### Research Insights — Architecture

**From Architecture Strategist:**
- The core-to-platform isolation is the strongest architectural property. Enforce with CI: `cargo tree -p agent-desktop-core` must contain no platform crate names.
- Binary crate's `Cargo.toml` must use **target-gated dependencies** for platform crates (not unconditional deps with `#[cfg]` in source).
- Consider extracting `RefMapStore` trait to decouple persistence from core (enables in-memory store for MCP/daemon in Phase 3/4).
- `src/` as a workspace member for the binary crate is non-standard. Consider workspace root or `crates/cli/`.

**From Code Simplicity Reviewer:**
- Drop `Command` trait + `CommandRegistry` — use plain functions + `match`. The trait's associated types prevent `dyn Command`, making the registry useless. Each command is already a standalone `execute()` function in practice.
- Delete `crates/mcp/` stub — unlike platform stubs which enforce adapter boundary, the MCP crate enforces nothing. Create in Phase 3.
- Remove `manage_window` and `synthesize_input` from trait — these serve Phase 2 commands only. Trait can grow additively.
- Trait drops from 13 to 11 methods after removing these + adding `resolve_element`.

**From Pattern Recognition Specialist:**
- Extract `resolve_ref()` and `wrap_response()` helpers to eliminate 100-150 lines of boilerplate across 10+ ref-based command files.
- Provide default impls on `PlatformAdapter` that return `not_supported()` — eliminates 65-line stub implementations per platform.
- `Response::ok(command, data)` and `Response::err(command, error)` constructors prevent envelope construction duplication.

---

## Dependency Versions

All versions confirmed against latest stable (February 2026). Use `workspace = true`
inheritance throughout.

### `Cargo.toml` (workspace root)

```toml
[workspace]
members = [
    "crates/core",
    "crates/macos",
    "crates/windows",
    "crates/linux",
    "crates/mcp",
    "src",
]
resolver = "2"

[workspace.package]
edition      = "2021"
rust-version = "1.78"
license      = "Apache-2.0"

[workspace.dependencies]
clap               = { version = "4",   features = ["derive"] }
serde              = { version = "1",   features = ["derive"] }
serde_json         = "1"
thiserror          = "2"
tokio              = { version = "1",   features = ["rt", "io-std", "macros", "sync", "time"] }
tracing            = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
schemars           = "0.8"
base64             = "0.22"
anyhow             = "1"

# Phase 3 only — verify actual published version before adding:
# rmcp = { version = "0.8", features = ["server", "transport-io"] }

[profile.release]
opt-level      = 3
lto            = true
codegen-units  = 1
strip          = true
```

### `rust-toolchain.toml` (workspace root)

```toml
[toolchain]
channel  = "stable"
profile  = "minimal"
targets  = ["aarch64-apple-darwin", "x86_64-apple-darwin"]
```

### Platform crate dependencies

**`crates/macos/Cargo.toml`**
```toml
[dependencies]
agent-desktop-core    = { path = "../core" }
thiserror.workspace   = true

[target.'cfg(target_os = "macos")'.dependencies]
accessibility-sys     = "0.1"
core-foundation       = "0.9"
core-foundation-sys   = "0.9"
```

**`crates/windows/Cargo.toml`** (Phase 2 stub)
```toml
[dependencies]
agent-desktop-core  = { path = "../core" }
thiserror.workspace = true

[target.'cfg(target_os = "windows")'.dependencies]
uiautomation = "0.24"
```

**`crates/linux/Cargo.toml`** (Phase 2 stub)
```toml
[dependencies]
agent-desktop-core  = { path = "../core" }
thiserror.workspace = true

[target.'cfg(target_os = "linux")'.dependencies]
atspi = "0.28"
zbus  = "5"
```

**`crates/mcp/Cargo.toml`** (Phase 3 stub)
```toml
[dependencies]
agent-desktop-core  = { path = "../core" }
serde.workspace     = true
serde_json.workspace = true
schemars.workspace  = true
# rmcp added in Phase 3 after version verification
```

### Version selection rationale

| Crate | Version | Why |
|---|---|---|
| `clap` | `4` | 4.5.x stable; derive API is mature; 40+ subcommands fit cleanly |
| `thiserror` | `2` | 2.0 released 2024; breaking changes are minor; `#[error(transparent)]` cleaner |
| `base64` | `0.22` | Engine API is required; free-function API removed in 0.22 |
| `schemars` | `0.8` | 1.0-alpha is unstable; 0.8 is production-stable; required by rmcp |
| `tokio` | minimal features | Do NOT use `full`; adds TLS/net/process not needed until Phase 3 |
| `rmcp` | verify before Phase 3 | PRD says 0.8+; actual crates.io version may differ; pin exact minor |
| `accessibility-sys` | `0.1` | Thin stable FFI to Apple AXUIElement.h; safer than higher-level `accessibility` crate |
| `core-foundation` | `0.9` | Required companion for CFTypeRef, CFString, CFArray management |

### Research Insights — Dependencies

**Version corrections (February 2026):**

| Crate | Plan Says | Actual/Recommended | Action |
|---|---|---|---|
| `rmcp` | `0.8+` | **0.15.0** (official SDK at `modelcontextprotocol/rust-sdk`) | Update Phase 3 comment. Pin exact: `"=0.15.0"` |
| `core-foundation` | `0.9` | Possibly `0.10.0` | Run `cargo search core-foundation` to confirm |
| `core-foundation-sys` | `0.9` | `0.8.x` (NOT 0.9.x!) | `core-foundation` 0.10.x depends on `core-foundation-sys` 0.8.x |
| `thiserror` | `2` | Correct (`2.0.x`) | Requires Rust 1.78+ (plan already specifies) |
| `schemars` | `0.8` | Correct (1.0 is alpha) | Defer to Phase 3 — no P1 consumer |
| `tokio` | `1` (5 features) | Correct but **not needed in P1** | Remove from P1; all ops are sync |

**Recommended release profile addition:**
```toml
[profile.release]
panic = "abort"    # 200-500KB smaller binary, no unwind tables
```

**Pre-implementation verification checklist:**
```bash
cargo search accessibility-sys
cargo search core-foundation
cargo search core-foundation-sys
cargo search rmcp
# Then create throwaway project and run:
cargo tree -d    # check for duplicate/conflicting versions
```

The `core-foundation` / `core-foundation-sys` / `accessibility-sys` version triangle is the highest-risk compatibility issue. Verify with `cargo tree` before committing versions.

**From Best Practices Researcher — version-pinned dependency table:**

| Crate | Pinned Version | Rationale |
|---|---|---|
| `clap` | `"4.5"` | 4.5.x stable; derive API mature |
| `serde` | `"1.0"` | Stable forever |
| `serde_json` | `"1.0"` | Stable forever |
| `thiserror` | `"2.0"` | 2.0 cleaner `#[error(transparent)]`; requires Rust 1.78+ |
| `tracing` | `"0.1"` | Stable |
| `tracing-subscriber` | `"0.3"` + `env-filter` | Stable |
| `base64` | `"0.22"` | Engine API required |
| `anyhow` | `"1.0"` | Test helpers only |
| `accessibility-sys` | `"0.1"` | Thin stable FFI |
| `core-foundation` | `"0.10"` or `"0.9"` | Verify compat |
| `core-graphics` | `"0.24"` | For CGEvent, CGWindowList |
| `rustc-hash` | `"2.0"` | FxHashSet for cycle detection (zero-dep, 2KB) |

---

## Workspace Layout

Matches PRD §4.1 exactly.

```
agent-desktop/
├── Cargo.toml                          # workspace root, shared deps
├── Cargo.lock
├── rust-toolchain.toml
├── clippy.toml                         # project-wide lint config
├── schemas/
│   ├── snapshot_response.json          # generated, checked in
│   ├── action_response.json
│   └── error_response.json
├── docs/
│   ├── brainstorms/
│   └── plans/
├── tests/
│   ├── fixtures/                       # golden JSON snapshots for regression tests
│   │   ├── finder_documents.json
│   │   ├── textedit_untitled.json
│   │   └── system_settings.json
│   └── integration/
│       ├── macos_snapshot.rs
│       ├── macos_actions.rs
│       └── cross_platform.rs           # stub, enabled in Phase 2
├── crates/
│   ├── core/src/
│   │   ├── lib.rs                      # pub re-exports only
│   │   ├── node.rs                     # AccessibilityNode, Rect, WindowInfo
│   │   ├── adapter.rs                  # PlatformAdapter trait
│   │   ├── action.rs                   # Action enum, ActionResult, InputEvent, WindowOp
│   │   ├── refs.rs                     # RefAllocator, RefMap, RefEntry
│   │   ├── snapshot.rs                 # SnapshotEngine (filter, allocate, serialize)
│   │   ├── error.rs                    # ErrorCode enum, AdapterError, AppError
│   │   ├── output.rs                   # Response envelope, JSON formatting
│   │   ├── command.rs                  # Command trait + CommandRegistry
│   │   └── commands/
│   │       ├── mod.rs                  # register_all()
│   │       ├── snapshot.rs
│   │       ├── click.rs
│   │       ├── type_text.rs
│   │       ├── set_value.rs
│   │       ├── press.rs
│   │       ├── find.rs
│   │       ├── get.rs
│   │       ├── is_check.rs
│   │       ├── screenshot.rs
│   │       ├── scroll.rs
│   │       ├── select.rs
│   │       ├── toggle.rs
│   │       ├── expand.rs
│   │       ├── collapse.rs
│   │       ├── focus.rs
│   │       ├── launch.rs
│   │       ├── close_app.rs
│   │       ├── list_windows.rs
│   │       ├── list_apps.rs
│   │       ├── focus_window.rs
│   │       ├── clipboard.rs
│   │       ├── wait.rs
│   │       ├── status.rs
│   │       ├── permissions.rs
│   │       └── version.rs
│   ├── macos/src/
│   │   ├── lib.rs
│   │   ├── adapter.rs                  # MacOSAdapter: PlatformAdapter impl
│   │   ├── tree.rs                     # AXUIElement traversal, AXElement newtype
│   │   ├── actions.rs                  # AXPress, SetValue, SetFocus, Expand, Select
│   │   ├── roles.rs                    # AXRole string → unified role mapping
│   │   ├── input.rs                    # CGEvent keyboard/mouse synthesis
│   │   ├── screenshot.rs               # CGWindowListCreateImage
│   │   └── permissions.rs              # AXIsProcessTrusted, TCC guidance
│   ├── windows/src/
│   │   ├── lib.rs
│   │   └── adapter.rs                  # WindowsAdapter stub → PLATFORM_UNSUPPORTED
│   ├── linux/src/
│   │   ├── lib.rs
│   │   └── adapter.rs                  # LinuxAdapter stub → PLATFORM_UNSUPPORTED
│   └── mcp/src/
│       ├── lib.rs
│       └── server.rs                   # MCP stub → Phase 3
└── src/
    ├── main.rs                         # entry point: mode detection, dispatch
    └── cli.rs                          # clap derive structs for all 30 commands
```

---

## Core Data Models

### `crates/core/src/node.rs`

```rust
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AccessibilityNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,          // @e1, @e2 — only on interactive roles

    pub role: String,                    // normalized: button, textfield, checkbox, etc.

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub states: Vec<String>,             // focused, selected, expanded, checked, etc.

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect>,            // only when --include-bounds

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<AccessibilityNode>,
}

### Research Insights — AccessibilityNode

**From Performance Oracle — consider Role enum + States bitfield:**
Roles come from a fixed set (~30 values). Using `role: String` allocates heap memory per node. A `Role` enum eliminates one allocation per node (2000 saved on Xcode-scale trees):
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Button, TextField, Checkbox, Link, MenuItem, Tab, Slider,
    ComboBox, TreeItem, Cell, Window, Group, Toolbar, StaticText,
    Image, Table, ScrollArea,
    #[serde(other)]
    Unknown,
}
```
Similarly, `states: Vec<String>` can become a bitfield (2 bytes vs 24+ bytes + heap):
```rust
bitflags::bitflags! {
    pub struct States: u16 {
        const FOCUSED = 0b0001; const SELECTED = 0b0010;
        const EXPANDED = 0b0100; const CHECKED = 0b1000;
        const DISABLED = 0b0001_0000;
    }
}
```
**Estimated savings:** 40-60% fewer heap allocations during tree construction.

**Trade-off:** Custom serde impls needed to maintain JSON array-of-strings format for `states`. The `#[serde(other)]` on `Role` handles unknown AX roles gracefully. Consider whether this optimization is worth the implementation cost for Phase 1 — `String` types work fine and are simpler. Apply this optimization if benchmarks show allocation pressure.

**From Pattern Recognition Specialist — remove `JsonSchema` derive:**
`schemars` is deferred to Phase 3. Remove `#[derive(JsonSchema)]` from all structs in Phase 1.

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: String,                      // w-4521 format
    pub title: String,
    pub app: String,
    pub pid: i32,
    pub bounds: Option<Rect>,
    pub is_focused: bool,
}
```

### `crates/core/src/refs.rs`

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Stable metadata for re-identifying an AXUIElement across invocations.
/// Stored in ~/.agent-desktop/last_refmap.json for CLI mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefEntry {
    pub pid: i32,
    pub role: String,
    pub name: Option<String>,
    pub bounds_hash: Option<u64>,        // FxHash of bounds for disambiguation
    pub available_actions: Vec<String>,  // ["Click", "SetValue"] etc.
    // Internal: NativeHandle is NOT serialized — resolved at action time
}

pub struct RefMap {
    inner: HashMap<String, RefEntry>,   // "@e1" → RefEntry
    counter: u32,
}

impl RefMap {
    pub fn new() -> Self {
        Self { inner: HashMap::new(), counter: 0 }
    }

    pub fn allocate(&mut self, entry: RefEntry) -> String {
        self.counter += 1;
        let ref_id = format!("@e{}", self.counter);
        self.inner.insert(ref_id.clone(), entry);
        ref_id
    }

    pub fn get(&self, ref_id: &str) -> Option<&RefEntry> {
        self.inner.get(ref_id)
    }

    pub fn save(&self) -> Result<(), AppError> {
        let path = refmap_path()?;
        let dir = path.parent().ok_or(AppError::Internal("invalid refmap path".into()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir_all(dir)?;

        let json = serde_json::to_string(&self)?;  // compact, not pretty
        let tmp = path.with_extension("tmp");

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
            std::io::Write::write_all(&mut file, json.as_bytes())?;
        }
        #[cfg(not(unix))]
        std::fs::write(&tmp, &json)?;

        std::fs::rename(&tmp, &path)?;  // atomic replace
        Ok(())
    }

    pub fn load() -> Result<Self, AppError> {
        let path = refmap_path()?;
        let json = std::fs::read_to_string(path)?;
        let inner: HashMap<String, RefEntry> = serde_json::from_str(&json)?;
        let counter = inner.keys()
            .filter_map(|k| k.strip_prefix("@e"))
            .filter_map(|n| n.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        Ok(Self { inner, counter })
    }
}

fn refmap_path() -> Result<std::path::PathBuf, AppError> {
    let home = dirs::home_dir()
        .ok_or(AppError::Internal("HOME directory not found".into()))?;
    Ok(home.join(".agent-desktop").join("last_refmap.json"))
}

### Research Insights — RefMap

**Changes from original:**
- **Atomic writes** — temp file + `rename()` prevents corruption on concurrent access or kill.
- **File permissions** — `0o600` for RefMap, `0o700` for directory (was world-readable).
- **Compact JSON** — `to_string` instead of `to_string_pretty` (30-40% smaller, faster to parse).
- **Fixed `unwrap()`** — `refmap_path()` returns `Result` instead of panicking.
- **Fixed counter** — Derived from highest ref number in keys, not `inner.len()` (prevents collisions on sparse maps).

**From Architecture Strategist — RefMap counter serialization:**
Serialize the counter alongside the map to make the format self-consistent:
```rust
#[derive(Serialize, Deserialize)]
pub struct RefMap {
    inner: HashMap<String, RefEntry>,
    counter: u32,
}
```

**From Security Sentinel — RefMap integrity:**
- Add file size limit on load (reject >1MB to prevent memory exhaustion).
- In Phase 3/4, consider HMAC for file integrity checking.
- Include `source_app` and `source_window_id` in `RefEntry` to prevent the silent wrong-target bug when two agents clobber each other's RefMap.

**From Spec Flow Analyzer — concurrent access race (Race 3):**
Agent A snapshots Finder, Agent B snapshots TextEdit (overwrites RefMap), Agent A clicks @e5 — silently hits TextEdit element. **Mitigation:** Add `source_app` field to RefEntry and validate before action execution.
```

### `crates/core/src/adapter.rs`

The single most important abstraction. Never import platform crates from core.

```rust
use crate::{
    action::{Action, ActionResult, InputEvent, WindowOp},
    node::AccessibilityNode,
    node::WindowInfo,
    error::AdapterError,
};

pub struct WindowFilter {
    pub focused_only: bool,
    pub app: Option<String>,
}

pub struct TreeOptions {
    pub max_depth: u8,
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
}

pub enum ScreenshotTarget {
    Screen(usize),
    Window(String),          // window ID
    Element(NativeHandle),
    FullScreen,
}

pub enum PermissionStatus {
    Granted,
    Denied { suggestion: String },
}

/// Opaque handle to a native UI element within a snapshot context.
/// Not Send/Sync on macOS (AXUIElement). Methods that receive a NativeHandle
/// must be called on the thread that created it.
pub struct NativeHandle(pub(crate) *const std::ffi::c_void);

pub struct ImageBuffer {
    pub data: Vec<u8>,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}

pub enum ImageFormat { Png, Jpg }

pub trait PlatformAdapter: Send + Sync {
    fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError>;
    fn list_apps(&self) -> Result<Vec<String>, AdapterError>;
    fn get_tree(&self, win: &WindowInfo, opts: &TreeOptions)
        -> Result<AccessibilityNode, AdapterError>;
    fn execute_action(&self, handle: &NativeHandle, action: Action)
        -> Result<ActionResult, AdapterError>;
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

### Research Insights — PlatformAdapter Trait

**Changes from original (13 methods → 12 methods):**
- **Added** `resolve_element` — every action command needs ref-to-native-handle resolution. This is platform-specific (macOS walks AX tree, Windows queries UIA).
- **Removed** `synthesize_input` — Phase 2 concern. P1 keyboard needs route through `execute_action` with `Action::PressKey(KeyCombo)`.
- **Removed** `manage_window` — all window geometry commands (`resize`, `move`, `minimize`) are Phase 2. Only P1 window op is `focus_window` which has its own method.

**From Performance Oracle — `resolve_element` must use early-termination search:**
```rust
fn resolve_element(&self, entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    let root = element_for_pid(entry.pid);
    resolve_recursive(&root, entry, 0, 20)
        .ok_or_else(|| AdapterError::stale_ref())
}
```
Search stops the moment the matching element is found (O(k) average vs O(n) full traversal).

**From Security Sentinel — NativeHandle soundness fix:**
```rust
use std::marker::PhantomData;

pub struct NativeHandle {
    pub(crate) ptr: *const std::ffi::c_void,
    _not_send_sync: PhantomData<*const ()>,
}
```
Raw pointers are `!Send + !Sync`. For Phase 1 (single-threaded CLI), add `unsafe impl Send/Sync` with safety documentation. Revisit for Phase 3 async runtime.

**From Architecture Strategist — provide default implementations:**
```rust
pub trait PlatformAdapter: Send + Sync {
    fn list_windows(&self, _: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_windows"))
    }
    // ... defaults for all methods
}
```
This eliminates 60+ lines of boilerplate per stub adapter.

### `crates/core/src/error.rs`

```rust
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    PermissionDenied,
    ElementNotFound,
    ApplicationNotFound,
    ActionFailed,
    ActionNotSupported,
    StaleRef,
    WindowNotFound,
    PlatformNotSupported,
    Timeout,
    InvalidArgs,
    Internal,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct AdapterError {
    pub code: ErrorCode,
    pub message: String,
    pub suggestion: Option<String>,
    pub platform_detail: Option<String>,
}

impl AdapterError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), suggestion: None, platform_detail: None }
    }

    pub fn with_suggestion(mut self, s: impl Into<String>) -> Self {
        self.suggestion = Some(s.into());
        self
    }

    pub fn stale_ref(ref_id: &str) -> Self {
        Self::new(ErrorCode::StaleRef, format!("{ref_id} not found in current RefMap"))
            .with_suggestion("Run 'snapshot' to refresh, then retry with updated ref")
    }

    pub fn not_supported(msg: &str) -> Self {
        Self::new(ErrorCode::PlatformNotSupported, msg)
            .with_suggestion("This platform adapter ships in Phase 2")
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Adapter(#[from] AdapterError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Internal(String),
}
```

### Research Insights — Error Design

**Changes from original (12 variants → 10):**
- **Merged** `TreeTimeout` + `Timeout` → single `Timeout` (the `message` field provides context).
- **Removed** `ClipboardEmpty` — empty clipboard is a valid state, not an error. Return `{ "ok": true, "data": { "text": null } }`.
- **Added** `InvalidArgs` — for clap parse failures wrapped in JSON envelope.
- **Renamed** for consistency: `PermDenied` → `PermissionDenied`, `AppNotFound` → `ApplicationNotFound`, `PlatformUnsupported` → `PlatformNotSupported`.

**From Agent-Native Reviewer — exit code contract:**
- Exit 0 = `ok: true`, valid JSON on stdout
- Exit 1 = `ok: false`, valid JSON on stdout (structured error)
- Exit 2 = binary-level failure (no JSON guarantee — clap errors, panics)

**From Agent-Native Reviewer — add `retry_command` to ErrorPayload:**
```rust
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
    pub retry_command: Option<String>,  // e.g., "snapshot --app Finder"
    pub platform_detail: Option<String>,
}
```
Gives agents a machine-executable recovery path instead of parsing natural language from `suggestion`.

**From Security Sentinel — strip `platform_detail` in MCP mode:**
Only include raw AXError codes / HRESULT values when `--verbose` is set or in CLI mode. MCP responses should omit platform internals.

### `crates/core/src/output.rs`

```rust
use serde::Serialize;
use schemars::JsonSchema;

/// The versioned response envelope. All commands produce this structure.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Response<T: Serialize> {
    pub version: &'static str,           // "1.0"
    pub ok: bool,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<AppContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorPayload>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AppContext {
    pub name: String,
    pub window: Option<WindowContext>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct WindowContext {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_detail: Option<String>,
}
```

---

## Command Extensibility Pattern

Every command is one file under `crates/core/src/commands/`. No existing files change
when a command is added — only `mod.rs` and `src/cli.rs` receive new lines.

### Dispatch via `match` (replaces `Command` trait + `CommandRegistry`)

```rust
pub fn dispatch(
    cmd: Commands,
    adapter: &dyn PlatformAdapter,
) -> Result<serde_json::Value, AppError> {
    match cmd {
        Commands::Snapshot(args) => commands::snapshot::execute(args, adapter),
        Commands::Click(args) => commands::click::execute(args.into(), adapter),
        Commands::Find(args) => commands::find::execute(args, adapter),
        // ... one arm per command
    }
}
```

Adding a command = add one file + add one `Commands` variant + add one match arm. Same cost as the registry approach but without runtime indirection or trait object gymnastics.

### Shared helpers: `crates/core/src/commands/helpers.rs`

```rust
pub fn resolve_ref(
    ref_id: &str,
    adapter: &dyn PlatformAdapter,
) -> Result<(RefEntry, NativeHandle), AppError> {
    validate_ref_id(ref_id)?;
    let refmap = RefMap::load()?;
    let entry = refmap.get(ref_id).ok_or(AppError::stale_ref(ref_id))?.clone();
    let handle = adapter.resolve_element(&entry)?;
    Ok((entry, handle))
}

fn validate_ref_id(ref_id: &str) -> Result<(), AppError> {
    let valid = ref_id.starts_with("@e")
        && ref_id.len() <= 10
        && ref_id[2..].chars().all(|c| c.is_ascii_digit());
    if !valid {
        return Err(AppError::invalid_input("ref_id must match @e{N}"));
    }
    Ok(())
}
```

### Example: `crates/core/src/commands/click.rs`

```rust
use crate::{adapter::PlatformAdapter, error::AppError, commands::helpers::resolve_ref};

pub struct ClickArgs {
    pub ref_id: String,
}

pub fn execute(args: ClickArgs, adapter: &dyn PlatformAdapter) -> Result<serde_json::Value, AppError> {
    let (_entry, handle) = resolve_ref(&args.ref_id, adapter)?;
    let result = adapter.execute_action(&handle, crate::action::Action::Click)?;
    Ok(serde_json::to_value(result)?)
}
```

### Research Insights — Command Pattern

**Why `Command` trait + `CommandRegistry` were removed:**
- The trait's associated types (`type Args`, `type Output`) prevent `dyn Command` — the registry cannot store heterogeneous commands without type erasure.
- The `CommandRegistry::dispatch` referenced `crate::cli::Commands` from the binary crate, creating a circular dependency.
- The actual `click.rs` code was already a plain function, not a trait impl. The plan contradicted itself.
- `Response` envelope wrapping now happens once in the dispatch layer, not duplicated in every command file.

**From Agent-Native Reviewer — add `batch` command:**
```
agent-desktop batch '[
  {"command": "snapshot", "args": {"app": "Finder"}},
  {"command": "click", "args": {"ref": "@e3"}},
  {"command": "get", "args": {"ref": "@e3", "property": "value"}}
]'
```
Returns array of responses. Eliminates N process spawns for observe-act-verify sequences. Add `--stop-on-error` flag.

**From Agent-Native Reviewer — post-action state in responses:**
Every action command should return the element's state after the action:
```json
{
  "data": {
    "action": "click",
    "ref": "@e5",
    "post_state": { "role": "checkbox", "states": ["checked", "focused"] }
  }
}
```
Eliminates the verify round-trip (2 tool calls → 1 per action).

---

## SnapshotEngine Processing Pipeline

Lives entirely in `crates/core/src/snapshot.rs`. No platform code.

**3 stages** (simplified from original 5):
1. **Raw tree** — `adapter.get_tree(window, opts)` returns full `AccessibilityNode`
2. **Filter + allocate refs** — single depth-first pass: prune invisible/offscreen nodes, enforce `max_depth`, assign `@e1, @e2, …` to interactive roles, build `RefMap`
3. **Serialize** — `serde_json::to_writer(BufWriter::new(stdout.lock()), &response)` — stream directly to stdout, no intermediate String allocation

**Removed stages:**
- Stage 4 (serialize) was a one-liner calling `serde_json::to_value()` — not a pipeline stage.
- Stage 5 (token estimate) was YAGNI — agents see the output directly and know their own token budget. Validate P1-O4 (<500 tokens for Finder) in integration tests using actual tiktoken, not a runtime heuristic.

**Interactive roles** (only these receive refs):
`button, textfield, checkbox, link, menuitem, tab, slider, combobox, treeitem, cell`

**RefMap write behavior:** Each snapshot REPLACES the full file. Action commands that
follow a snapshot always operate on fresh refs. Multi-window workflows require
separate snapshots (each replaces the previous RefMap).

### Research Insights — Snapshot Pipeline

**From Performance Oracle — budget-aware tree construction:**
Integrate token budget checking into the filter stage. If estimated chars exceed `max_tokens * 4`, stop adding children to deeper subtrees. This prevents serializing trees that will exceed the budget.

**From Agent-Native Reviewer — default behavior:**
- `snapshot` with no arguments should default to the focused window of the frontmost application (the 80% case).
- Add `--subtree <ref>` flag to snapshot only the subtree rooted at a specific element.
- Add `--filter-role <roles>` to only include specific roles.

**From Spec Flow Analyzer — `find` semantics:**
`find` should perform a fresh snapshot internally (using the last-used window or requiring `--app`), update the RefMap, and return matching elements with their refs. Include an ancestry `path` field for disambiguation:
```json
{ "ref": "@e7", "role": "button", "name": "Save", "path": ["window:Documents", "toolbar", "button:Save"] }
```

**From Performance Oracle — serialization performance:**
Use `serde_json::to_writer(BufWriter::new(stdout.lock()), &data)` instead of `to_string()` + `println!`. Eliminates intermediate String allocation (saves 2-50KB depending on tree size). For 1000-node trees: `to_writer` ~1.8ms vs `to_string` ~2.5ms.

---

## macOS Adapter Implementation

### `crates/macos/src/` — Tree module split

`tree.rs` exceeds 400 LOC and is split into three files (see "Code Quality and Structural Refactors" section). The patterns below apply to the split files: `AXElement` and `element_for_pid` go in `ax_element.rs`, attribute helpers in `ax_attrs.rs`, traversal logic in `ax_tree.rs`.

### `crates/macos/src/ax_element.rs` — Key patterns

```rust
use accessibility_sys::{
    AXUIElementRef, AXUIElementCreateApplication,
    AXUIElementCopyAttributeValue,
    kAXChildrenAttribute, kAXRoleAttribute,
    kAXTitleAttribute, kAXDescriptionAttribute,
    kAXValueAttribute, kAXEnabledAttribute,
    kAXErrorSuccess, kAXErrorNoValue, kAXErrorAttributeUnsupported,
};
use core_foundation::{base::{CFTypeRef, CFRelease, TCFType}, string::CFString};
use std::collections::HashSet;

/// Owns an AXUIElementRef. Releases on drop.
pub struct AXElement(pub AXUIElementRef);

impl Drop for AXElement {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0 as CFTypeRef) }
        }
    }
}

// SAFETY: AXUIElement is NOT thread-safe. The MacOSAdapter is designed so
// all AX calls happen on the calling thread within a single method invocation.
// No AXElement is stored in MacOSAdapter fields.

pub fn element_for_pid(pid: i32) -> AXElement {
    AXElement(unsafe { AXUIElementCreateApplication(pid) })
}

/// Traverse with depth limit and cycle detection.
/// visited: raw pointer addresses (usize) prevent infinite loops on malformed trees.
pub fn build_subtree(
    el: &AXElement,
    depth: usize,
    max_depth: usize,
    visited: &mut HashSet<usize>,
) -> Option<agent_desktop_core::node::AccessibilityNode> {
    if depth > max_depth { return None; }
    if !visited.insert(el.0 as usize) { return None; } // cycle

    let role = copy_string_attr(el, unsafe { kAXRoleAttribute })?;
    // ... build AccessibilityNode, recurse into children
}
```

### `crates/macos/src/permissions.rs`

```rust
use accessibility_sys::{AXIsProcessTrusted, AXIsProcessTrustedWithOptions};

pub fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Prompt system dialog. Only call via `permissions --request`.
pub fn request_trust() -> bool {
    // Build CFDictionary with kAXTrustedCheckOptionPrompt = kCFBooleanTrue
    // ... omitted for brevity
    unsafe { AXIsProcessTrustedWithOptions(options) }
}
```

### AX attribute error handling

```rust
match err {
    kAXErrorSuccess             => { /* use value */ }
    kAXErrorNoValue             => return None,      // attribute has no value
    kAXErrorAttributeUnsupported => return None,     // element lacks this attribute
    kAXErrorInvalidUIElement    => return Err(AdapterError::stale()),  // element gone
    kAXErrorCannotComplete      => return Err(AdapterError::timeout()), // app busy/hung
    _                           => return Err(AdapterError::ax(err)),
}
```

### Research Insights — macOS Adapter

**From Performance Oracle (CRITICAL) — batch attribute fetch:**
`AXUIElementCopyMultipleAttributeValues` fetches all requested attributes in a single IPC round-trip. Reduces 7 calls per node to 1. For 2000 nodes (Xcode), drops from 14,000 IPC calls to 2,000. **3-5x faster traversal.**

This API is NOT in `accessibility-sys` 0.1. Declare manually:
```rust
extern "C" {
    fn AXUIElementCopyMultipleAttributeValues(
        element: AXUIElementRef,
        attributes: CFArrayRef,
        options: u32,
        values: *mut CFArrayRef,
    ) -> AXError;
}
```

**Traversal performance projections:**

| Scenario | Nodes | FFI Calls (current) | FFI Calls (batch) | Time (current) | Time (batch) |
|---|---|---|---|---|---|
| Finder Documents | ~100 | ~700 | ~100 | 35-70ms | 10-20ms |
| TextEdit simple | ~50 | ~350 | ~50 | 18-35ms | 5-10ms |
| System Settings | ~500 | ~3,500 | ~500 | 175-350ms | 50-100ms |
| Xcode (depth=8) | ~1,500 | ~10,500 | ~1,500 | 525-1,050ms | 150-300ms |

Without batch fetch, the 2-second Xcode target is achievable only at depth ≤8. With it, depth 12-15 becomes feasible.

**From Security Sentinel — `AXElement` safety fixes:**
- Make inner field private: `pub struct AXElement(AXUIElementRef)` (not `pub AXUIElementRef`)
- Implement `Clone` with `CFRetain`:
```rust
impl Clone for AXElement {
    fn clone(&self) -> Self {
        if !self.0.is_null() {
            unsafe { CFRetain(self.0 as CFTypeRef); }
        }
        AXElement(self.0)
    }
}
```

**From Best Practices Researcher — CFTypeRef memory management:**
- `Create` / `Copy` functions → `wrap_under_create_rule` (you own it, Drop releases)
- `Get` functions → `wrap_under_get_rule` (borrowed, do NOT release)
- `AXUIElementCopyAttributeValue` returns OWNED `CFTypeRef` → use create rule

**From Framework Docs Researcher — missing FFI declarations needed:**
- `AXUIElementCopyMultipleAttributeValues` (batch fetch)
- `CGWindowListCopyWindowInfo` (from CoreGraphics, not Accessibility)
- `CGEventCreateKeyboardEvent` / `CGEventCreateMouseEvent` (from CoreGraphics)
- NSWorkspace bindings for `launch_app` / `list_apps` (use `objc` crate or shell out to `open -a`)

**From Performance Oracle — cycle detection optimization:**
Replace `HashSet<usize>` with `FxHashSet<usize>` from `rustc-hash` (2x faster hash ops, zero downside since keys are machine pointers, not user input).

**From Security Sentinel — `kAXErrorCannotComplete` (-25204):**
Most common error in practice. Target app is busy or hung. Retry with backoff or return `TIMEOUT`.

---

## Entry Point — `src/main.rs`

```rust
mod cli;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            if e.kind() == clap::error::ErrorKind::DisplayHelp
                || e.kind() == clap::error::ErrorKind::DisplayVersion
            {
                e.exit();
            }
            let json = serde_json::json!({
                "version": "1.0",
                "ok": false,
                "command": "unknown",
                "error": { "code": "INVALID_ARGS", "message": e.to_string().lines().next().unwrap_or("Unknown error") }
            });
            println!("{json}");
            std::process::exit(2);
        }
    };

    init_tracing(cli.verbose);

    match cli.command {
        Some(Commands::Version(args)) => handle_version(args),
        Some(Commands::Status) => handle_status(),
        Some(cmd) => {
            let adapter = build_adapter();
            let result = dispatch(cmd, &adapter);
            match result {
                Ok(data) => {
                    let response = serde_json::json!({
                        "version": "1.0", "ok": true,
                        "command": cmd_name, "data": data
                    });
                    println!("{response}");
                    std::process::exit(0);
                }
                Err(e) => {
                    println!("{}", e.to_json());
                    std::process::exit(1);
                }
            }
        }
        None => { Cli::command().print_help().unwrap(); }
    }
}

fn build_adapter() -> impl agent_desktop_core::adapter::PlatformAdapter {
    #[cfg(target_os = "macos")]
    { agent_desktop_macos::MacOSAdapter::new() }

    #[cfg(not(target_os = "macos"))]
    compile_error!("Unsupported platform")
}

fn init_tracing(verbose: bool) {
    use tracing_subscriber::{fmt, EnvFilter};
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(if verbose { "debug" } else { "warn" }))
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
}
```

### Research Insights — Entry Point

**Changes from original:**
- **Removed** `--mcp` flag + `IsTerminal` stdin detection — MCP is Phase 3. Piped input (`< /dev/null`) falsely triggered MCP mode. Add `--mcp` flag in Phase 3 as a one-line addition.
- **Removed** `Arc<dyn PlatformAdapter>` — Phase 1 is single-threaded. Return `impl PlatformAdapter` instead.
- **Added** `Cli::try_parse()` with JSON error wrapping — agents parsing stdout for JSON will get unparseable plain text from clap's default error handler. Now they get structured `INVALID_ARGS` errors.
- **Lazy adapter construction** — `Version` and `Status` commands don't need the adapter. Only construct it when needed.
- **Added** `RUST_LOG` environment variable override — `try_from_default_env().unwrap_or(filter)` lets users override via `RUST_LOG=agent_desktop_macos=trace`.
- **Added** `with_ansi(false)` — no color codes in stderr (may be redirected).

**From Best Practices Researcher — startup performance budget:**

| Phase | Typical Cost | Notes |
|-------|-------------|-------|
| Dynamic linker | 1-2ms | Static linking helps |
| `main()` entry | <0.1ms | |
| clap parsing | 0.5-2ms | 30 subcommands |
| tracing init | 0.2-0.5ms | env-filter parsing |
| Adapter construction | <0.1ms | Just struct init |
| **Total cold start** | **2-5ms** | Well under 10ms target |

Removing tokio from Phase 1 saves 1-3ms startup and 200-400KB binary size.

---

## CLI Structure — `src/cli.rs`

Key patterns from research:

- Use `global = true` on `--verbose` and `--mcp` so they work after subcommands
- `kebab-case` flags are automatic from `snake_case` field names in clap 4
- `std::io::IsTerminal` from stable std — no `atty` dependency
- `subcommand_required = false` + explicit `None` handling (not `arg_required_else_help`)

```rust
#[derive(Parser, Debug)]
#[command(name = "agent-desktop", version, about = "Desktop automation for AI agents")]
pub struct Cli {
    #[arg(long, global = true, hide = true)]
    pub mcp: bool,

    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Snapshot(SnapshotArgs),
    Find(FindArgs),
    Screenshot(ScreenshotArgs),
    Get(GetArgs),
    Is(IsArgs),
    Click(RefArgs),
    DoubleClick(RefArgs),
    RightClick(RefArgs),
    Type(TypeArgs),
    SetValue(SetValueArgs),
    Focus(RefArgs),
    Select(SelectArgs),
    Toggle(RefArgs),
    Expand(RefArgs),
    Collapse(RefArgs),
    Scroll(ScrollArgs),
    Launch(LaunchArgs),
    CloseApp(CloseAppArgs),
    ListWindows(ListWindowsArgs),
    ListApps(ListAppsArgs),
    FocusWindow(FocusWindowArgs),
    Press(PressArgs),
    ClipboardGet,
    ClipboardSet(ClipboardSetArgs),
    Wait(WaitArgs),
    Status,
    Permissions(PermissionsArgs),
    Version(VersionArgs),
    Batch(BatchArgs),
}
```

### Research Insights — CLI

**From Best Practices Researcher:**
- Use `Cli::try_parse()` instead of `Cli::parse()` to catch clap errors and wrap them in JSON envelope with `INVALID_ARGS` error code.
- `help` and `version` display remains plain text (not JSON). All other errors are JSON.
- For 30 subcommands, parsing is ~1ms. Do NOT use `arg_required_else_help = true` (prevents `--mcp` from working).
- `kebab-case` is automatic from `snake_case` field names in clap 4.

**From Agent-Native Reviewer — `batch` command added:**
```rust
Batch(BatchArgs),  // JSON array of commands, returns array of responses
```

**From Spec Flow Analyzer — missing global flags to consider:**

| Flag | Purpose |
|---|---|
| `--timeout <ms>` (global) | Default timeout for any blocking command |
| `--session <id>` | Scope RefMap to a session (prevents multi-agent corruption) |
| `--dry-run` | Show what would happen without executing (agent debugging) |

**From Best Practices Researcher — subcommand grouping for `--help`:**
```rust
#[command(after_help = "\
CATEGORIES:
  Observation:  snapshot, find, screenshot, get, is
  Interaction:  click, double-click, right-click, type, set-value, focus, select, toggle, expand, collapse, scroll, press
  App/Window:   launch, close-app, list-windows, list-apps, focus-window
  Clipboard:    clipboard-get, clipboard-set
  Wait:         wait
  System:       status, permissions, version
  Batch:        batch")]
```

---

## JSON Output Contract

All commands produce this envelope. Schema files live in `schemas/` and are
generated from the Rust structs via `schemars`. Schema version is tracked via
the `version` field.

```json
{
  "version": "1.0",
  "ok": true,
  "command": "snapshot",
  "app": { "name": "Finder", "window": { "id": "w-4521", "title": "Documents" } },
  "ref_count": 14,
  "tree": {
    "role": "window",
    "name": "Documents",
    "children": [
      { "role": "toolbar", "children": [
        { "ref": "@e1", "role": "button", "name": "Back" },
        { "ref": "@e2", "role": "button", "name": "Forward" }
      ]},
      { "ref": "@e3", "role": "textfield", "name": "Search", "value": "" }
    ]
  }
}
```

Error envelope:
```json
{
  "version": "1.0",
  "ok": false,
  "command": "click",
  "error": {
    "code": "STALE_REF",
    "message": "@e3 not found in current RefMap",
    "suggestion": "Run 'snapshot' to refresh, then retry with updated ref",
    "retry_command": "snapshot --app Finder"
  }
}
```

### Research Insights — JSON Contract

**From Agent-Native Reviewer — envelope consistency fix:**
The snapshot example had `ref_count` and `tree` as **top-level** fields, but `Response<T>` wraps data in `data`. Every command without exception must nest its output inside `data`. Use `Response` with `serde_json::Value`:

```rust
pub struct Response {
    pub version: &'static str,
    pub ok: bool,
    pub command: String,
    pub app: Option<AppContext>,
    pub data: Option<serde_json::Value>,
    pub error: Option<ErrorPayload>,
}
```

**Corrected snapshot example:**
```json
{
  "version": "1.0",
  "ok": true,
  "command": "snapshot",
  "app": { "name": "Finder", "window": { "id": "w-4521", "title": "Documents" } },
  "data": {
    "ref_count": 14,
    "tree": { "role": "window", "children": [...] }
  }
}
```

**From Spec Flow Analyzer — `screenshot` output format:**
If `[path]` argument is provided, write to file and return `{ "path": "/tmp/screenshot.png" }` in data. If not provided, return base64-encoded data. The `base64` crate is already in dependencies for this purpose.

**From Spec Flow Analyzer — `is` command output format:**
```json
{ "data": { "property": "visible", "result": true } }
```

---

## Testing Plan

### Unit tests — `crates/core/src/` (no macOS required, runs on any CI)

```
snapshot_engine_filter_test       - invisible nodes removed, depth pruning works
ref_allocator_ordering_test       - depth-first order, only interactive roles get refs
ref_allocator_interactive_test    - button/textfield get refs; group/statictext do not
snapshot_serialize_compact_test   - null fields omitted, empty arrays omitted
snapshot_serialize_roundtrip_test - AccessibilityNode ser/de roundtrip
refmap_save_load_test             - RefMap REPLACE semantics; stale detection
error_serialize_test              - every ErrorCode serializes to SCREAMING_SNAKE_CASE
json_schema_validate_test         - generated schema matches output
```

**MockAdapter** for unit tests:

```rust
// crates/core/src/commands/tests/mock_adapter.rs
pub struct MockAdapter {
    pub tree: AccessibilityNode,   // hardcoded tree returned by get_tree()
    pub clipboard: String,
    pub windows: Vec<WindowInfo>,
}
impl PlatformAdapter for MockAdapter { ... }
```

### Golden fixture regression — `tests/fixtures/`

Checked-in JSON files captured from real macOS apps. Tests load them and verify:
- Ref count matches expected
- Interactive elements have refs; static text does not
- JSON schema validates against `schemas/snapshot_response.json`

Fixtures captured once during development and committed. Re-capture with a
`--update-fixtures` flag when intentional format changes occur.

### Research Insights — Testing

**From Best Practices Researcher — CI configuration for macOS tests:**
```yaml
jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --lib --all

  integration-tests:
    runs-on: macos-14           # Apple Silicon runner (has display session)
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Grant accessibility (TCC)
        run: |
          sudo tccutil reset Accessibility
      - run: cargo test --test integration
```

**From Performance Oracle — add benchmarks during Week 3-4:**
```rust
// benches/traversal.rs (using criterion)
fn bench_finder_traversal(c: &mut Criterion) {
    c.bench_function("finder_full_tree", |b| {
        b.iter(|| adapter.get_tree(&window, &opts))
    });
}
```

Add `criterion` as dev-dependency. Benchmark traversal, serialization, and refmap I/O independently. This validates whether batch fetch (CRITICAL-3) is needed on target hardware.

**From Architecture Strategist — CI isolation enforcement:**
```yaml
- name: Verify core isolation
  run: |
    cargo tree -p agent-desktop-core | grep -E "accessibility-sys|core-foundation|uiautomation|atspi|zbus" && exit 1 || true
```

**From Spec Flow Analyzer — missing integration test:**
Add a test for the canonical agent workflow: `snapshot → find → click → snapshot → verify`. This is the most important flow to regression-test.

### Integration tests — `tests/integration/` (macOS CI only)

Real AX adapter tests on GitHub Actions macOS runner:

```
macos_snapshot_finder_test       - snapshot Finder Documents window → non-empty tree
macos_snapshot_textedit_test     - snapshot TextEdit → textfield refs present
macos_click_test                 - launch test app, click button, verify state changed
macos_type_test                  - type text into TextEdit, verify content
macos_clipboard_test             - clipboard get/set roundtrip
macos_launch_close_test          - launch Calculator, verify window appears, close
macos_perm_denied_test           - simulate no TCC permission → PERM_DENIED error code
macos_large_tree_test            - snapshot Xcode → completes under 2 seconds
```

---

## Phase 1 — 10-Week Implementation Timeline

### Weeks 1–2: Scaffold + Core Types

**Deliverables:**
- `Cargo.toml` workspace with all 6 crates + resolver 2
- `rust-toolchain.toml` pinned to stable 1.78+
- `crates/core/src/`: `node.rs`, `adapter.rs`, `action.rs`, `error.rs`, `output.rs`
- `crates/core/src/refs.rs`: `RefMap`, `RefEntry`, `RefAllocator` structs
- `crates/core/src/command.rs`: `Command` trait + `CommandRegistry` skeleton
- `crates/core/src/commands/mod.rs`: empty `register_all()`
- Stub adapters for windows, linux (all methods → `AdapterError::not_supported`)
- Stub MCP crate
- `src/cli.rs`: all 30 subcommand structs (no-op implementations)
- `src/main.rs`: mode detection, `build_adapter()`, `init_tracing()`
- `clippy.toml` with `unwrap_in_result = "deny"`, `panic = "deny"` for non-test
- `schemas/` directory with placeholder JSON Schema files
- CI: `cargo check --all-targets` passes on macOS

**Key invariants to enforce from day one:**
- `crates/core` has zero imports from any platform crate (verify with `cargo tree` in CI)
- Zero `unwrap()` calls in non-test code
- All error types implement the structured pattern from `error.rs`
- Binary crate uses target-gated deps: `[target.'cfg(target_os = "macos")'.dependencies]`
- `clippy.toml` with `forbid-unwrap-in-result = true`

### Research Insights — Weeks 1–2

**Simplified workspace from agent research:**
- Delete: `crates/mcp/` stub (create in Phase 3), `command.rs` (use direct dispatch), `schemas/` dir (defer to Phase 3)
- Stub crates: single `lib.rs` per stub (not separate `adapter.rs`), no platform deps in stub `Cargo.toml`
- Omit from workspace deps: `tokio`, `schemars` (not needed until Phase 2/3)
- Add to workspace deps: `rustc-hash = "2.0"` (FxHashSet for cycle detection)

---

### Weeks 3–4: macOS Tree Traversal

**Deliverables:**
- `crates/macos/src/ax_element.rs`: `AXElement` newtype with `Drop`, `Clone` (CFRetain), `element_for_pid()`, `window_element_for()`
- `crates/macos/src/ax_attrs.rs`: `copy_string_attr`, `copy_ax_array`, `fetch_node_attrs`, `read_bounds`
- `crates/macos/src/ax_tree.rs`: `build_subtree()` with cycle detection via `FxHashSet<usize>`, depth limiting, `resolve_element_name`
- `crates/macos/src/roles.rs`: AXRole string → unified role enum mapping (AXButton → "button", AXTextField → "textfield", etc.)
- `crates/macos/src/permissions.rs`: `is_trusted()`, `request_trust()`
- `crates/macos/src/adapter.rs`: `MacOSAdapter::get_tree()` and `MacOSAdapter::check_permissions()` implemented; all other methods stub
- `crates/macos/examples/axprobe.rs`: probe binary moved from `src/bin/`, gated on `dev-tools` feature
- Unit tests: role mapping coverage, cycle detection (synthetic circular tree), permission mock
- Stage 1–3 probe validation completed for each AX attribute used before wiring into adapter
- Manual validation: `cargo run -- snapshot --app Finder` produces valid JSON

**Gotchas from research:**
- `kAXErrorNoValue` is non-fatal — return `None`, not `Err`
- `kAXErrorCannotComplete` (-25204) is the most common error — app is busy/hung. Retry or return `TIMEOUT`
- `kAXVisibleChildrenAttribute` fallback for scroll views
- Never call AX APIs on tokio thread pool — all AX stays on calling thread
- `build.rs` in `crates/macos/` to link `ApplicationServices` and `CoreFoundation`
- `AXElement.0` must be `pub(crate)` not `pub` to prevent double-free
- Use `wrap_under_create_rule` for Copy/Create function results; `wrap_under_get_rule` for Get results
- Implement `AXUIElementCopyMultipleAttributeValues` as manual `extern "C"` (not in accessibility-sys 0.1)
- Add `criterion` benchmarks during this phase to validate performance targets

---

### Weeks 5–6: Snapshot Engine + Ref System

**Deliverables:**
- `crates/core/src/snapshot.rs`: full `SnapshotEngine` with 5-stage pipeline
- `crates/core/src/refs.rs`: `RefAllocator` depth-first ordering, interactive-role
  filtering, `RefMap::save()` / `RefMap::load()` with REPLACE semantics
- RefMap written to `~/.agent-desktop/last_refmap.json` after each snapshot
- Token estimation (char ÷ 4 heuristic, warn at 500)
- `crates/core/src/commands/snapshot.rs`: full implementation
- Unit tests: all 8 snapshot + ref unit tests passing
- Golden fixtures captured: Finder, TextEdit, System Settings
- JSON Schema files generated and committed to `schemas/`
- Integration test: `macos_snapshot_finder_test` passes

---

### Weeks 6–7: Core Interaction Commands

**Deliverables:**
- `click.rs`, `double_click.rs`, `right_click.rs`
- `type_text.rs` (CGEventCreateKeyboardEvent synthesis)
- `set_value.rs` (AXUIElementSetAttributeValue)
- `focus.rs` (AXFocusedAttribute = true)
- `press.rs` (CGEvent key combos: cmd+c, ctrl+shift+s, etc.)
- `find.rs` (tree search by name, role, or value substring)
- `get.rs` (text, value, title, bounds, role, states)
- `is_check.rs` (visible, enabled, checked, focused, expanded)
- `crates/macos/src/input.rs`: key combo parsing and CGEvent synthesis
- Integration tests: `macos_click_test`, `macos_type_test` passing

---

### Weeks 7–8: App/Window + Remaining Interaction Commands

**Deliverables:**
- `launch.rs` (NSWorkspace / `open -a`, --wait via window polling)
- `close_app.rs` (graceful quit + SIGKILL --force)
- `list_windows.rs` (CGWindowListCopyWindowInfo filtered to visible)
- `list_apps.rs` (running processes via NSWorkspace)
- `focus_window.rs` (AXRaise + NSActivateApp)
- `screenshot.rs` (CGWindowListCreateImage for window; full screen)
- `select.rs` (kAXSelectedAttribute on child elements)
- `toggle.rs` (detect role + appropriate action: AXPress for checkbox)
- `expand.rs` / `collapse.rs` (kAXExpandedAttribute toggle)
- `scroll.rs` (kAXScrollByPageAttribute or CGEventScrollWheelCreate)
- Integration tests: `macos_launch_close_test`, `macos_snapshot_xcode_test` (large tree)

---

### Weeks 8–9: Clipboard, Wait, System Commands

**Deliverables:**
- `clipboard.rs` get/set (NSPasteboard via Cocoa FFI)
- `wait.rs`:
  - `wait <ms>` — `std::thread::sleep`
  - `wait --element <ref>` — poll RefMap every 100ms up to timeout
  - `wait --window <title>` — poll window list for title match
- `status.rs` (platform, permissions, daemon PID placeholder)
- `permissions.rs` (check + optional `--request` prompt)
- `version.rs` (from `CARGO_PKG_VERSION`, optional --json)
- Integration tests: `macos_clipboard_test`, `macos_perm_denied_test`

---

### Weeks 9–10: Testing, CI, Binary Distribution, Polish

**Deliverables:**
- All 8 unit test suites passing (`cargo test --lib`)
- All integration tests passing on GitHub Actions macOS runner
- JSON Schema validation added to CI: `cargo test --test schema_validation`
- GitHub Actions workflow (`.github/workflows/ci.yml`):
  - `cargo fmt --check`
  - `cargo clippy --deny warnings`
  - `cargo test --lib` (any runner)
  - `cargo test --test integration` (macOS runner)
  - `cargo build --release` builds under 15MB
- `cargo-dist init` with `[workspace.metadata.dist]` config
- Release workflow (`.github/workflows/release.yml`) via cargo-dist:
  - Produces `aarch64-apple-darwin` + `x86_64-apple-darwin` tarballs
  - SHA256 checksums attached to GitHub Release
- Binary size verified: `cargo build --release && ls -lh target/release/agent-desktop`
- CLI smoke test: `./target/release/agent-desktop snapshot --app Finder | jq .ok`
- README with installation, usage, and macOS TCC setup instructions
- P1-O1 through P1-O9 success criteria all verified (PRD §6.1)

---

## Phase 1 Acceptance Criteria

From PRD §6.1:

| ID | Criterion | Verification |
|---|---|---|
| P1-O1 | Working macOS snapshot CLI | `snapshot --app Finder` returns valid JSON with refs |
| P1-O2 | Platform adapter trait | Trait compiles with MockAdapter; MacOSAdapter satisfies all methods |
| P1-O3 | Ref-based interaction | `click @e3` invokes AXPress on resolved element |
| P1-O4 | Context efficiency | Finder Documents snapshot < 500 tokens |
| P1-O5 | Typed JSON contract | Output validates against JSON Schema; schema is versioned |
| P1-O6 | Permission detection | Missing TCC permission prints macOS setup instructions |
| P1-O7 | Command extensibility | Adding a new command = 1 new file + 2 registration lines |
| P1-O8 | 30 working commands | All P1-scoped commands from §5 pass integration tests |
| P1-O9 | CI pipeline | GitHub Actions macOS runner executes full test suite on every PR |

---

## Phase 2–4 Reference

| Phase | Duration | Key Work |
|---|---|---|
| P2: Cross-Platform | 10 weeks | Windows adapter (uiautomation 0.24), Linux adapter (atspi 0.28 + zbus 5), cross-platform CI |
| P3: MCP Server | 6 weeks | rmcp integration (verify version), stdio transport, Claude Desktop validation |
| P4: Hardening | 8 weeks | Persistent daemon, session isolation, brew/winget/snap packages, performance benchmarks |

**Phase 3 note:** rmcp is confirmed at **0.15.0** (February 2026) — the official Rust
MCP SDK at `modelcontextprotocol/rust-sdk`. Pin to exact: `"=0.15.0"`. The PRD's
`0.8+` version was incorrect. Verify features: `server`, `transport-io`.
Before Phase 3, implement auth/authz policy system (Security finding F03).

---

## Security Hardening (Phase 1)

From the Security Sentinel audit (17 findings, 5 CRITICAL/HIGH for Phase 1):

### Immediate (Week 1 — before first code commit)

| # | Finding | Fix |
|---|---|---|
| F01 | `NativeHandle` is `Send/Sync` unsound | Add `PhantomData<*const ()>` field |
| F02 | `AXElement` double-free risk | Make inner field `pub(crate)`, implement `Clone` with `CFRetain` |
| F04 | RefMap world-readable permissions | `0o600` file, `0o700` directory |
| F05 | `home_dir().unwrap()` panic | Return `Result`, propagate error |

### During Implementation (Weeks 2–10)

| # | Finding | Fix |
|---|---|---|
| F06 | No input validation on `ref_id` | `validate_ref_id()` at entry of every ref-based command |
| F07 | `launch_app` accepts arbitrary paths | Validate app identifiers (bundle ID or name only), reject raw paths. Use `NSWorkspace` APIs, not `open -a` |
| F08 | `press` accepts dangerous key combos | Blocklist: `cmd+q`, `cmd+shift+q`, `cmd+opt+esc`, `ctrl+cmd+q`, `cmd+shift+delete` |
| F09 | `type_text` enables command injection | Text length limits (10,000 chars max). Log invocations with target app context |
| F12 | `close_app --force` lacks safeguards | Protected process list: `loginwindow`, `WindowServer`, `Dock`, `launchd`. PID ownership check |
| F14 | Cycle detection uses pointer address | Add hard `ABSOLUTE_MAX_DEPTH = 50` cap |
| F17 | No audit logging | Implement `~/.agent-desktop/audit.log` with `0o600` permissions |

### Pre-Phase 3 (Before MCP Ships)

| # | Finding | Fix |
|---|---|---|
| F03 | MCP server has no auth/authz | Policy file (`~/.agent-desktop/policy.toml`) with command allowlist/denylist |
| F16 | MCP has no message size limits | 1MB max per message, request timeout |
| F10 | Screenshot data exposure | Separate TCC permission check (`kTCCServiceScreenCapture`). Add `--no-capture` flag |
| F11 | Clipboard credential exposure | Detect `org.nspasteboard.ConcealedType` (password manager entries), refuse to read |

---

## Risk Mitigations

| Risk | Mitigation |
|---|---|
| macOS TCC blocks CI integration tests | Document setup; use GitHub Actions `macos-14` runner (ARM, has display session); `tccutil reset Accessibility` + sqlite3 TCC.db grant |
| AXUIElement cycle causes stack overflow | `FxHashSet<usize>` cycle detection; max_depth hard stop at 20; `ABSOLUTE_MAX_DEPTH = 50` cap |
| Large AX trees (Xcode) exceed 5s timeout | Default max_depth = 8; focused-window-only; `AXUIElementCopyMultipleAttributeValues` batch fetch (3-5x faster); integration test asserts < 2s |
| rmcp version mismatch | **Confirmed: rmcp 0.15.0** (Feb 2026). Pin exact: `"=0.15.0"`. Fall back to hand-rolled JSON-RPC if API changes |
| Binary > 15MB | `lto = true`, `strip = true`, `panic = "abort"` in release profile; feature-gate `schemars`; monitor with CI size check. Expected 3-6MB |
| core-foundation version mismatch | Run `cargo tree -d` to check for duplicates. `core-foundation` 0.10.x depends on `core-foundation-sys` 0.8.x (NOT matching versions) |
| Concurrent agent RefMap corruption | Atomic writes (temp + rename). Document as known limitation. Add `source_app` to RefEntry for validation |
| Focus theft during `type` command | Document that `type` is best-effort, `set-value` is atomic. Consider verifying focus before each keystroke batch |

---

## Agent-Native Checklist

From the Agent-Native Reviewer (score: 26/30, 4 structural gaps):

### Must-Fix (before Phase 1 ships)

- [x] **Batch command** — single process invocation for multi-step workflows
- [x] **Envelope consistency** — `ref_count`/`tree` inside `data`, not top-level
- [x] **Post-action state** — every action returns element state after the action
- [x] **Exit code contract** — 0=ok, 1=structured error, 2=argument error
- [x] **Default snapshot** — no args = focused window of frontmost app

### Should-Fix (Phase 1)

- [ ] **`wait --match`** — semantic criteria (`--role button --name "Save"`) instead of stale ref
- [ ] **`retry_command`** in ErrorPayload — machine-executable recovery
- [ ] **`find` ancestry path** — `["window:Documents", "toolbar", "button:Save"]`
- [ ] **`available_actions`** in tree output — opt-in with `--include-actions`
- [ ] **MCP naming convention** — `desktop_{command}` documented now, implemented in Phase 3

### Consider (late Phase 1 or Phase 2)

- [ ] Namespaced refs by window (`@w4521.e1`) for multi-window workflows
- [ ] `resolve <ref>` probe command (check validity without replacing RefMap)
- [ ] `diff` command (change detection between snapshots)
- [ ] `status --app <name>` responsiveness check
- [ ] `--max-tokens` flag on snapshot

---

## Code Quality and Structural Refactors

These findings are not blockers for Phase 1 feature work, but must be resolved before the Phase 1 milestone is closed. Each has a clear fix and zero risk of behaviour change.

### Dead Code to Remove

**`crates/core/src/commands/clipboard.rs`** — 13 lines, exports `execute_get()` and `execute_set()` with zero callers anywhere in the workspace. The clipboard commands are implemented in the correct split files `clipboard_get.rs` and `clipboard_set.rs` (one command per file), which `dispatch.rs` calls directly. `clipboard.rs` is a leftover from before the split and should be deleted along with its `pub mod clipboard;` line in `mod.rs`.

**`batch::execute` stub** — `crates/core/src/commands/batch.rs` exports an `execute()` function that returns `Ok(json!({"note": "..."}))`. It is never called; `dispatch.rs` handles batch routing inline. Delete the `execute` function body. Keep `BatchArgs`, `BatchCommand`, and `parse_commands` — they are live.

### LOC Violations

**`crates/macos/src/tree.rs` is at 403 lines** — exceeds the 400-line hard limit. Split into three files by single responsibility:

| New file | Contents | Estimated LOC |
|---|---|---|
| `ax_element.rs` | `AXElement` newtype, `Drop`, `Clone` (with `CFRetain`), `element_for_pid`, `window_element_for` | ~55 |
| `ax_attrs.rs` | `copy_string_attr`, `copy_value_typed`, `copy_bool_attr`, `copy_ax_array`, `read_bounds`, `fetch_node_attrs` | ~200 |
| `ax_tree.rs` | `build_subtree`, `resolve_element_name`, `label_from_children`, `copy_children` | ~115 |

**`crates/macos/src/lib.rs`** — update module declarations:
```rust
pub mod ax_element;
pub mod ax_attrs;
pub mod ax_tree;
// Remove: pub mod tree;
```

Internal cross-file imports use `super::` paths:
```rust
// ax_tree.rs
use super::ax_element::AXElement;
use super::ax_attrs::{copy_string_attr, fetch_node_attrs};
```

**`src/dispatch.rs` is at 397 lines** — 3 lines under the limit but contains 187 lines of duplication (see below). Collapsing the duplication brings it to ~210 lines.

### Dispatch Duplication

`dispatch.rs` has two parallel dispatch tables:
- Lines 16–169: `dispatch()` — type-safe `Commands` enum, 29 match arms
- Lines 171–358: `dispatch_batch_command()` — string-keyed version, same 29 commands parsed differently

**Fix:** Collapse `dispatch_batch_command()` by deserializing batch JSON into the `Commands` enum. Because `Commands` derives `Deserialize`, `serde_json::from_value(json!({"snapshot": {"app": "Finder"}}))` already works:

```rust
pub fn dispatch_batch_command(
    name: &str,
    args: serde_json::Value,
    adapter: &dyn PlatformAdapter,
) -> Result<serde_json::Value, AppError> {
    let cmd: Commands = serde_json::from_value(serde_json::json!({ name: args }))
        .map_err(|e| AppError::invalid_args(e.to_string()))?;
    dispatch(cmd, adapter)
}
```

This eliminates ~187 lines of duplication. `batch.rs`'s `parse_commands` routes `{"command": "snapshot", "args": {...}}` entries to `dispatch_batch_command("snapshot", args, adapter)` — one unified code path.

### Probe Binary Reorganization

`src/bin/axprobe.rs` and `src/bin/axprobe2.rs` are development probe binaries compiled on every `cargo build`. They should live in `examples/` and only compile when explicitly requested:

```
crates/macos/
└── examples/
    ├── axprobe.rs       # moved from src/bin/axprobe.rs
    ├── axprobe2.rs      # moved from src/bin/axprobe2.rs
    └── probe_utils.rs   # extract shared helpers (find_pid, string readers, array readers)
```

In `crates/macos/Cargo.toml`:
```toml
[[example]]
name = "axprobe"
path = "examples/axprobe.rs"
required-features = ["dev-tools"]

[features]
dev-tools = []
```

Run with: `cargo run -p agent-desktop-macos --example axprobe --features dev-tools`

### Bugs to Fix Before Phase 1 Ships

| File | Bug | Fix |
|---|---|---|
| `wait.rs` | `if let Ok(refmap) = RefMap::load()` silently discards load errors on every polling iteration | Propagate with `?` or emit `tracing::warn!` |
| `is_check.rs` | States read from stale RefMap entry; `_handle` is resolved (liveness check) but then discarded | Add `///` doc-comment: "Returns state from last snapshot. Run `snapshot` first for live state." |
| `press.rs` | `parts.last().unwrap()` — safe but proof depends on preceding `is_empty` guard being visible | Change to `parts.last().ok_or_else(|| AppError::invalid_args("empty key combo"))` |
| `press.rs` | `NativeHandle::null()` passed for `PressKey` action — convention undocumented | Add `// SAFETY: PressKey synthesises a global CGEvent; no element handle is required.` |

---

## Test-Before-Implement Development Workflow

Every new AX capability must be confirmed at the OS level before being wired into the adapter. This 4-stage process eliminates "wrote 100 lines then discovered the attribute doesn't exist" failures. Document this in `CLAUDE.md` under `## Development Workflow`.

### Stage 1 — Accessibility Inspector

Open `/Applications/Xcode.app/Contents/Applications/Accessibility Inspector.app`. Point at the target application. Confirm the AX attribute exists and has a non-null value. Note the exact attribute name string (e.g. `AXMenus`, `AXFocusedWindow`).

### Stage 2 — JXA Probe (30-second sanity check)

```bash
osascript -l JavaScript -e '
  var sys = Application("System Events");
  var proc = sys.processes.whose({ name: "Finder" })[0];
  // inspect proc.windows(), proc.menuBars() etc.
  JSON.stringify(proc.windows[0].attributes.whose({ name: "AXRole" })[0].value())
'
```

If this returns the expected value, proceed to Stage 3. If it errors or returns null, the attribute is not accessible via standard AX for this app/macOS version — stop and reassess.

### Stage 3 — Rust Probe in examples/

Add a probe function to `examples/axprobe.rs` that calls the candidate AX API directly and prints the result:

```rust
// examples/axprobe.rs
fn probe_ax_menus(pid: i32) {
    let app = ax_element::element_for_pid(pid);
    let menus = ax_attrs::copy_ax_array(&app, "AXMenus");
    println!("AXMenus count: {:?}", menus.map(|v| v.len()));
}
```

Run: `cargo run -p agent-desktop-macos --example axprobe --features dev-tools -- --pid $(pgrep Finder)`

Only proceed to Stage 4 when Stage 3 prints the expected value.

### Stage 4 — Implement as `pub(crate)` + Unit Test

1. Add the function to the appropriate `ax_*.rs` file with `pub(crate)` visibility
2. Write a unit test (with MockAdapter or synthetic tree if real AX not available)
3. Wire into `adapter.rs` / `adapter.get_tree()`

### Why the Stages Matter

- AX attribute availability varies by app (Electron apps expose nothing; native apps vary)
- Some attributes exist but always return `kAXErrorNoValue` for certain element subtypes
- Stage 2 costs 30 seconds; Stage 3 costs 5 minutes; Stage 4 costs hours — fail early
- The probe files double as executable documentation of how each AX API was validated

---

## References

- PRD v2.0: `agent_desktop_prd_v2.pdf`
- Brainstorm: `docs/brainstorms/2026-02-19-architecture-validation-brainstorm.md`
- accessibility-sys: https://crates.io/crates/accessibility-sys
- core-foundation: https://crates.io/crates/core-foundation
- rmcp (MCP Rust SDK, v0.15.0): https://github.com/modelcontextprotocol/rust-sdk
- clap 4 derive docs: https://docs.rs/clap/latest/clap/_derive/index.html
- Apple AXUIElement reference: https://developer.apple.com/documentation/applicationservices
- Apple AXUIElementCopyMultipleAttributeValues: ApplicationServices/HIServices
- cargo-dist: https://github.com/axodotdev/cargo-dist
- Rust IsTerminal (stable 1.70+): https://doc.rust-lang.org/std/io/trait.IsTerminal.html
- rustc-hash (FxHashSet): https://crates.io/crates/rustc-hash
- bitflags: https://crates.io/crates/bitflags
- criterion (benchmarks): https://crates.io/crates/criterion
