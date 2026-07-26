---
title: "feat: implement Windows adapter via UI Automation"
type: feat
status: active
date: 2026-02-25
deepened: 2026-02-25
origin: docs/brainstorms/2026-02-25-windows-adapter-phase2-brainstorm.md
---

# feat: Implement Windows Adapter via UI Automation

## Enhancement Summary

**Deepened on:** 2026-02-25 (two rounds)
**Sections enhanced:** 14
**Research agents used:** Architecture Strategist, Performance Oracle, Security Sentinel, Pattern Recognition Specialist, Code Simplicity Reviewer, Agent-Native Reviewer, Spec Flow Analyzer, Best Practices Researcher, Framework Docs Researcher, Repo Research Analyst, CacheRequest Deep Dive (uiautomation-rs source), App Lifecycle Security (MSDN + Raymond Chen), COM/SendInput/DPI Specialist, Phase 0 Exact Analysis

### Key Improvements
1. **CacheRequest optimization** — Concrete `UIProperty` enum, `get_cached_*` methods, `UITreeWalker::*_build_cache` pattern for 5-10x tree traversal speedup. Corrected to `ElementMode::Full` (not `None` — `None` cannot receive actions)
2. **COM pointer lifecycle contract** — Explicit `AddRef`/`ManuallyDrop` pattern for `NativeHandle` on Windows, preventing double-free and use-after-free
3. **Security readiness** — `CreateProcessW` primary (safe for untrusted input), `ShellExecuteW` fallback with metacharacter rejection (not regex), clipboard RAII guard with 10x/100ms retry
4. **Chromium detection update** — Chrome 138+ has native UIA by default (since July 2025); version detection via `GetFileVersionInfoW`; sparse trees returned immediately with warning (no hidden latency)
5. **Complete role mapping** — Added 8 missing UIA control types (RadioButton, Spinner, ProgressBar, ScrollBar, StatusBar, Thumb, SplitButton, HeaderItem)
6. **SendInput batching** — 10-50x performance improvement for `type` command; double/triple click as 4/6 INPUT events in single call
7. **DPI awareness** — Corrected multi-monitor formula: `(pixel - virt_origin) * 65535 / (virt_size - 1)` with `MOUSEEVENTF_VIRTUALDESK`
8. **HRESULT→ErrorCode mapping table** — Complete mapping moved to Phase 0 `error_mapping.rs`
9. **PID type fix** — `i32` → `u64` promoted to Phase 0 (two-line change now vs painful migration later)
10. **Defense-in-depth timeout** — 4-layer strategy: `IsHungAppWindow` → `IUIAutomation2` timeouts → thread timeout → graceful degradation
11. **Two-tier acceptance criteria** — "core" (must-pass for merge) vs "complete" (must-pass for sign-off)

### New Considerations Discovered
- `WindowInfo.pid` was `i32` but Windows PIDs are `u32` (DWORD) — fixed in Phase 0 (§0.7) by changing to `u64` everywhere
- `SnapshotSurface::Sheet` and `SnapshotSurface::Popover` have no Windows equivalent — must return `ElementNotFound`
- `AppInfo.bundle_id` has no Windows analog — use `None` (field is already `Option<String>`)
- `key_down`/`key_up` commands skip blocked combo check on macOS too — safety bug to fix in Phase 0
- `wait --gone` referenced in plan but does not exist as a command variant — remove reference
- `pinch` command does not exist in the codebase — remove from acceptance criteria
- `PrintWindow` with `PW_RENDERFULLCONTENT` flag needed for modern DWM-composited apps (plain `BitBlt` returns black)
- Cloaked windows (virtual desktops) returned by `EnumWindows` but invisible — filter via `DwmGetWindowAttribute(DWMWA_CLOAKED)`
- ApplicationFrameHost.exe hosts UWP windows with PID mismatch from actual app — needs special handling in `list_windows`
- Hung/frozen app UIA calls block indefinitely — need timeout wrapper via thread + channel

---

## Overview

Implement the Windows `PlatformAdapter` for agent-desktop, enabling all 50 CLI commands to work on Windows via the UI Automation COM API. Uses `uiautomation` (v0.24+) for tree traversal and pattern-based actions, complemented by the `windows` crate for Win32 APIs (input synthesis, clipboard, screenshot, process lifecycle). Zero changes to core, macOS, or Linux crates.

## Problem Statement

agent-desktop currently only runs on macOS. The Phase 1 architecture was explicitly designed for additive platform expansion (see brainstorm: `docs/brainstorms/2026-02-25-windows-adapter-phase2-brainstorm.md`). The Windows adapter is the first test of this architecture — it must prove the trait-based isolation model works without touching any existing code.

## Proposed Solution

Implement `WindowsAdapter` in `crates/windows/` following the exact same delegation pattern as `MacOSAdapter`. The adapter is a stateless unit struct that creates `UIAutomation` instances per-call (COM threading safety). Platform-specific dependencies are target-gated. A Windows CI job validates real UIA integration.

### Research Insights: Architecture Validation

**Trait Friction Points:**
- `SnapshotSurface::Sheet` and `SnapshotSurface::Popover` are macOS concepts with no Windows equivalent. The Windows adapter must return `Err(AdapterError::element_not_found("No sheet/popover on Windows"))` for these variants.
- `AppInfo.bundle_id` is macOS-centric. Windows adapter returns `None` (the field is already `Option<String>`). No Windows equivalent needed for Phase 2.
- `WindowInfo.pid` is `i32` but Windows PIDs are `u32` (DWORD). Fixed in Phase 0 (see §0.7) — two-line change now avoids a painful migration later.

**Per-Call vs Cached UIAutomation:**
- Per-call `UIAutomation::new()` costs ~0.5-2ms COM init overhead. Keep per-call as default.
- **Concrete benchmark threshold:** If a 10-command batch exceeds 500ms cumulative COM init overhead, introduce `thread_local!` caching. Measure during Phase 1 integration tests.

**References:**
- Microsoft UIA threading docs: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-threading
- `uiautomation::UIAutomation::new()` internally calls `CoInitializeEx(COINIT_MULTITHREADED)` — MTA is correct for CLI

## Technical Approach

### Architecture

```
                  ┌─────────────────────────────┐
                  │   agent-desktop (binary)     │
                  │   src/main.rs                │
                  │   build_adapter()            │
                  │   #[cfg(target_os="windows")]│
                  └──────────┬──────────────────┘
                             │ &dyn PlatformAdapter
                  ┌──────────▼──────────────────┐
                  │  agent-desktop-core          │
                  │  PlatformAdapter trait (22)   │
                  │  SnapshotEngine, RefMap       │
                  │  Commands, Action enum        │
                  └──────────┬──────────────────┘
                             │ impl PlatformAdapter
         ┌───────────────────▼───────────────────┐
         │       agent-desktop-windows            │
         │  ┌─────────┐ ┌──────────┐ ┌─────────┐│
         │  │  tree/   │ │ actions/ │ │ input/  ││
         │  │UITreeWalk│ │UIInvoke  │ │SendInput││
         │  │UICacheReq│ │UIValue   │ │Clipboard││
         │  │UIMatcher │ │UIToggle  │ │Mouse    ││
         │  └─────────┘ └──────────┘ └─────────┘│
         │  ┌──────────────────────────────────┐ │
         │  │           system/                 │ │
         │  │ CreateProcess, ShowWindow, BitBlt │ │
         │  │ EnumWindows, WaitForInputIdle     │ │
         │  └──────────────────────────────────┘ │
         │                                        │
         │  Dependencies:                          │
         │  - uiautomation 0.24+ (UIA wrapper)    │
         │  - windows 0.62+ (Win32 APIs)           │
         └────────────────────────────────────────┘
```

### Implementation Phases

#### Phase 0: Pre-Work (Foundation Fixes)

Minimal, targeted changes that benefit all platforms and unblock Windows work. This is the **only phase that touches files outside `crates/windows/`**.

**0.1. Refactor BLOCKED_COMBOS out of core** (`crates/core/src/commands/press.rs:8-14`)

Current state: macOS-specific `BLOCKED_COMBOS` const in core. `key_down`/`key_up` skip the check entirely.

Approach: Add a `blocked_combos(&self) -> &[&str]` method to `PlatformAdapter` with an empty default. The `press`, `key_down`, and `key_up` commands call `adapter.blocked_combos()` to validate. macOS adapter returns its current list. Windows adapter returns Windows-specific list.

Files changed:
- `crates/core/src/adapter.rs` — add `blocked_combos` method with empty default
- `crates/core/src/commands/press.rs` — replace const with `adapter.blocked_combos()` call
- `crates/core/src/commands/key_down.rs` — add blocked combo check (safety fix)
- `crates/core/src/commands/key_up.rs` — add blocked combo check (safety fix)
- `crates/macos/src/adapter.rs` — override `blocked_combos` returning current macOS list

### Research Insights: Phase 0

**Security: `key_down`/`key_up` bypass is a real bug.** Currently on macOS, `press ctrl+alt+delete` is blocked but `key-down ctrl` → `key-down alt` → `key-down delete` → `key-up delete` → `key-up alt` → `key-up ctrl` bypasses the check entirely. The Phase 0 fix must apply blocked combo validation to individual key events by tracking modifier state, not just checking the full combo string.

**Pattern: Phase 0 must be an atomic PR.** Ship all Phase 0 changes in a single PR, reviewed and merged before any Windows work begins. This prevents merge conflicts and ensures core changes are validated by macOS CI first.

---

**0.2. Remove dead `permission_denied()` method** (`crates/core/src/error.rs:107-115`)

Zero callers in the entire codebase. The macOS adapter constructs permission errors via `PermissionReport::Denied { suggestion }` directly. Remove the dead method.

Files changed:
- `crates/core/src/error.rs` — remove `permission_denied()` method

**0.3. Add Windows CI job** (`.github/workflows/ci.yml`)

Add `test-windows` job on `windows-latest`. Use PowerShell for binary size check (`(Get-Item target/release/agent-desktop.exe).Length`). Initially runs only `cargo check` and `cargo test --lib -p agent-desktop-windows` until real implementation exists.

Files changed:
- `.github/workflows/ci.yml` — add `test-windows` job

### Research Insights: Windows CI

**Best Practices:**
- GitHub Actions `windows-latest` runners have a real desktop session — UIA works, GUI apps render
- Use `--test-threads=1` for integration tests — concurrent tests fighting over foreground windows causes flakiness
- Scope clippy to `cargo clippy -p agent-desktop-windows --all-targets -- -D warnings` — avoid false positives from platform-gated code in other crates
- `notepad.exe` is the ideal CI test target — guaranteed available, simple UI, has textfield
- `calc.exe` is a Store/UWP app that may NOT be pre-installed on CI images — do not rely on it

**CI Workflow Pattern:**
```yaml
test-windows:
  runs-on: windows-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - run: cargo clippy -p agent-desktop-windows --all-targets -- -D warnings
    - run: cargo test --lib -p agent-desktop-windows
    - name: Integration tests
      run: |
        Start-Process notepad.exe
        Start-Sleep -Seconds 2
        cargo test --test windows_integration -- --test-threads=1
        Stop-Process -Name notepad -ErrorAction SilentlyContinue
```

---

**0.4. Create folder skeleton in `crates/windows/`**

Create the full subfolder structure with `mod.rs` files and empty stubs. Each stub module re-exports nothing initially. `adapter.rs` has an empty `impl PlatformAdapter for WindowsAdapter {}` that delegates nothing (same as current).

### Research Insights: cfg-gate Pattern

**Every `.rs` file in `crates/windows/src/` must use the cfg-gate pattern:**

```rust
// Every file follows this pattern:
#[cfg(target_os = "windows")]
mod imp {
    // Real implementation using uiautomation, windows crates
}

#[cfg(not(target_os = "windows"))]
mod imp {
    // Stub that returns AdapterError::not_supported()
}

// Re-export from imp
pub use imp::*;
// OR: pub(crate) fn some_function(...) { imp::some_function(...) }
```

This ensures `cargo check` and `cargo clippy` pass on macOS/Linux during development. Without these stubs, CI on non-Windows platforms will fail. Budget ~15-20 additional LOC per file for stubs.

**Add `rustc-hash` to dependencies.** The macOS adapter uses `FxHashSet` from `rustc-hash` for visited-set tracking. The Windows adapter needs the same for cycle prevention in tree traversal. Currently missing from the planned Cargo.toml.

---

Files created:
```
crates/windows/src/
├── lib.rs              # mod declarations + re-export WindowsAdapter
├── adapter.rs          # PlatformAdapter impl (empty initially)
├── tree/
│   ├── mod.rs
│   ├── element.rs
│   ├── builder.rs
│   ├── roles.rs
│   ├── resolve.rs
│   └── surfaces.rs
├── actions/
│   ├── mod.rs
│   ├── dispatch.rs
│   ├── activate.rs
│   └── extras.rs
├── input/
│   ├── mod.rs
│   ├── keyboard.rs
│   ├── mouse.rs
│   └── clipboard.rs
└── system/
    ├── mod.rs
    ├── app_ops.rs
    ├── window_ops.rs
    ├── key_dispatch.rs
    ├── permissions.rs
    ├── screenshot.rs
    └── wait.rs
```

**0.5. Create `error_mapping.rs`** (`crates/windows/src/error_mapping.rs`)

Consolidate all HRESULT→ErrorCode and UIA error→AdapterError conversion in one place. This unblocks all subsequent phases and prevents duplicating error mapping logic across modules.

```rust
// crates/windows/src/error_mapping.rs (~60 LOC)
fn hresult_to_error_code(hr: i32) -> ErrorCode {
    match hr {
        0x80040201 => ErrorCode::StaleRef,        // UIA_E_ELEMENTNOTAVAILABLE
        0x80040200 => ErrorCode::ActionFailed,     // UIA_E_ELEMENTNOTENABLED
        0x80040204 => ErrorCode::ActionNotSupported, // UIA_E_NOTSUPPORTED
        0x80131509 => ErrorCode::ActionFailed,     // UIA_E_INVALIDOPERATION
        0x80131505 => ErrorCode::Timeout,          // UIA_E_TIMEOUT
        0x80070005 => ErrorCode::PermDenied,       // E_ACCESSDENIED
        0x80070057 => ErrorCode::InvalidArgs,      // E_INVALIDARG
        _          => ErrorCode::Internal,         // E_FAIL and others
    }
}
```

Files created:
- `crates/windows/src/error_mapping.rs`

---

**0.6. Add dependencies to `crates/windows/Cargo.toml`**

```toml
[dependencies]
agent-desktop-core.workspace = true
thiserror.workspace          = true
serde.workspace              = true
serde_json.workspace         = true
tracing.workspace            = true
base64.workspace             = true
rustc-hash.workspace         = true

[target.'cfg(target_os = "windows")'.dependencies]
uiautomation = { version = "0.24", features = ["process"] }
windows = { version = "0.62", features = [
    "Win32_Foundation",
    "Win32_System_Threading",
    "Win32_System_Com",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_DataExchange",
    "Win32_Graphics_Gdi",
    "Win32_System_Memory",
    "Win32_System_ProcessStatus",
    "Win32_Graphics_Dwm",
    "Win32_UI_Accessibility",
    "Win32_Storage_FileSystem",
] }
```

### Research Insights: Dependencies

**Added `Win32_Graphics_Dwm` feature** — needed for `DwmGetWindowAttribute(DWMWA_CLOAKED)` to filter cloaked windows (virtual desktop windows that `EnumWindows` returns but aren't visible).

**Added `Win32_UI_Accessibility` feature** — needed for `IUIAutomation2` (timeout configuration: `SetConnectionTimeout`, `SetTransactionTimeout`) and `IsHungAppWindow` pre-check.

**Added `Win32_Storage_FileSystem` feature** — needed for `GetFileVersionInfoW` / `VerQueryValueW` (Chromium version detection from exe).

**Removed `uiautomation` `clipboard` feature** — clipboard operations should use the `windows` crate directly for consistency with other Win32 operations and to avoid duplicating clipboard logic across two crate APIs.

**Added `rustc-hash` workspace dependency** — same `FxHashSet` used by macOS adapter for cycle prevention.

---

**0.7. Fix PID type: `i32` → `u64`** (`crates/core/src/node.rs`)

`WindowInfo.pid` and `AppInfo.pid` are `i32`, but Windows PIDs are `u32` (DWORD) and macOS PIDs are `pid_t` (signed 32-bit). Using `u64` accommodates both platforms without truncation risk. This is a two-line type change in core with mechanical updates to callers — trivial now, painful migration later when Windows snapshot tests depend on PID handling.

Files changed:
- `crates/core/src/node.rs` — change `pid: i32` → `pid: u64` in `WindowInfo` and `AppInfo`
- All callers (mechanical `as u64` casts in macOS adapter, direct use in Windows adapter)

---

#### Phase 1: Observation Tier

Implement the commands that let agents SEE the Windows desktop. This is the most critical tier — without observation, nothing else works.

**1.1. Tree traversal + role mapping** (`tree/builder.rs`, `tree/roles.rs`, `tree/element.rs`)

Implement `get_tree` using `UITreeWalker` with `UICacheRequest` for batch attribute retrieval. Map Windows UIA `ControlType` values to agent-desktop's unified role strings.

```
tree/element.rs   — Wrapper around UIElement, attribute readers (name, role, value, states, bounds)
tree/builder.rs   — build_subtree() using UITreeWalker, depth-first traversal, ancestor-set cycle prevention
tree/roles.rs     — ControlType → role string mapping (Button→button, Edit→textfield, CheckBox→checkbox, etc.)
```

Key UIA control type mappings:
| UIA ControlType | agent-desktop role |
|---|---|
| UIA_ButtonControlTypeId | button |
| UIA_EditControlTypeId | textfield |
| UIA_CheckBoxControlTypeId | checkbox |
| UIA_HyperlinkControlTypeId | link |
| UIA_MenuItemControlTypeId | menuitem |
| UIA_TabItemControlTypeId | tab |
| UIA_SliderControlTypeId | slider |
| UIA_ComboBoxControlTypeId | combobox |
| UIA_TreeItemControlTypeId | treeitem |
| UIA_DataItemControlTypeId | cell |
| UIA_TextControlTypeId | statictext |
| UIA_GroupControlTypeId | group |
| UIA_WindowControlTypeId | window |
| UIA_ToolBarControlTypeId | toolbar |
| UIA_MenuBarControlTypeId | menubar |
| UIA_ListItemControlTypeId | listitem |
| UIA_RadioButtonControlTypeId | radiobutton |
| UIA_SpinnerControlTypeId | spinbutton |
| UIA_ProgressBarControlTypeId | progressbar |
| UIA_ScrollBarControlTypeId | scrollbar |
| UIA_StatusBarControlTypeId | statusbar |
| UIA_ThumbControlTypeId | thumb |
| UIA_SplitButtonControlTypeId | splitbutton |
| UIA_HeaderItemControlTypeId | columnheader |

### Research Insights: CacheRequest Optimization (Critical Performance)

**This is THE most impactful performance decision.** Without CacheRequest, every property access on every element triggers a separate cross-process COM call. Benchmarks from research:

| App | Without CacheRequest | With CacheRequest | Improvement |
|-----|---------------------|-------------------|-------------|
| Explorer | ~1.5s | ~200ms | 7.5x |
| VS Code | ~6s+ | ~500ms | 12x |
| Chrome | ~15s+ | ~1.2s | 12.5x |

**Properties to cache (batch in a single cross-process call):**
```
UIA_NamePropertyId              → AccessibilityNode.name
UIA_ControlTypePropertyId       → role (via roles.rs mapping)
UIA_BoundingRectanglePropertyId → bounds
UIA_ValueValuePropertyId        → value
UIA_HelpTextPropertyId          → description
UIA_IsEnabledPropertyId         → states.enabled
UIA_HasKeyboardFocusPropertyId  → states.focused
UIA_IsKeyboardFocusablePropertyId → states.focusable
UIA_IsOffscreenPropertyId       → (skip offscreen elements)
UIA_RuntimeIdPropertyId         → element re-identification
UIA_ProcessIdPropertyId         → RefEntry.pid
```

**Patterns to pre-fetch:**
```
InvokePattern       → determines available_actions: ["click"]
ValuePattern        → determines available_actions: ["set-value", "clear"]
TogglePattern       → determines available_actions: ["toggle", "check", "uncheck"]
ExpandCollapsePattern → determines available_actions: ["expand", "collapse"]
SelectionItemPattern  → determines available_actions: ["select"]
ScrollPattern       → determines available_actions: ["scroll", "scroll-to"]
RangeValuePattern   → determines available_actions: ["set-value"] (for sliders)
```

**Use `AutomationElementMode::Full` (not `None`).** Research confirmed that `None` mode elements cannot receive actions, cannot call `build_updated_cache()`, and **cannot be upgraded to Full**. Since agent-desktop's RefMap stores elements that may later receive actions (click, set-value), Full mode is required. The cost is minimal — cached properties are still read locally.

**Use the control view walker** (not raw view). This eliminates internal framework elements:
- WPF: removes layout containers, adorner decorators
- Win32: removes internal child windows of common controls
- Typically reduces element count by 40-60%

**Implementation pattern:**
```rust
let automation = UIAutomation::new()?;
let cache = automation.create_cache_request()?;
cache.add_property(UIProperty::Name)?;
cache.add_property(UIProperty::ControlType)?;
cache.add_property(UIProperty::BoundingRectangle)?;
// ... add all properties above
cache.add_pattern(UIPatternType::Invoke)?;
cache.add_pattern(UIPatternType::Value)?;
// ... add all patterns above
cache.set_tree_scope(TreeScope::Element)?;
cache.set_element_mode(ElementMode::Full)?; // Full mode: elements can receive actions later

let walker = automation.get_control_view_walker()?;
// Use walker.get_first_child_build_cache(&root, &cache) for traversal
```

**Defense-in-depth timeout protection for hung apps:**
1. **Layer 1 — Pre-check:** Call `IsHungAppWindow(hwnd)` before UIA traversal. Return `ACTION_FAILED` immediately if hung.
2. **Layer 2 — UIA timeouts:** Cast to `IUIAutomation2` and set `ConnectionTimeout` to 1000ms (default 2s) and `TransactionTimeout` to 5000ms (default 20s).
3. **Layer 3 — Thread timeout:** Wrap entire traversal in `mpsc::channel` + `recv_timeout(Duration::from_secs(2))`.
4. **Layer 4 — Graceful degradation:** If a subtree times out, return partial tree with timed-out branch pruned.

```rust
// Layer 1: pre-check
if unsafe { IsHungAppWindow(hwnd) }.as_bool() {
    return Err(AdapterError::action_failed("Target window is not responding"));
}
// Layer 2: configure UIA timeouts via IUIAutomation2
let automation2: IUIAutomation2 = raw_automation.cast()?;
unsafe { automation2.SetConnectionTimeout(1000)?; automation2.SetTransactionTimeout(5000)?; }
// Layer 3: thread timeout
let (tx, rx) = std::sync::mpsc::channel();
std::thread::spawn(move || { tx.send(build_subtree(...)).ok(); });
rx.recv_timeout(Duration::from_secs(2))
    .map_err(|_| AdapterError::timeout("Tree traversal timed out"))?
```

**References:**
- UIA caching: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-cachingforclients
- `uiautomation::UICacheRequest`: https://docs.rs/uiautomation/latest/uiautomation/core/struct.UICacheRequest.html

---

Trait methods implemented: `get_tree`

**1.2. Window and app enumeration** (`system/window_ops.rs`, `system/app_ops.rs`)

```
system/window_ops.rs — EnumWindows + GetWindowText + GetWindowThreadProcessId for list_windows
                       GetForegroundWindow for focused_window
system/app_ops.rs    — EnumProcesses + QueryFullProcessImageName for list_apps
```

### Research Insights: Window Enumeration

**Filter cloaked windows.** `EnumWindows` returns windows on virtual desktops (cloaked). Filter via:
```rust
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
let mut cloaked: u32 = 0;
DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut _ as _, size_of::<u32>() as u32);
if cloaked != 0 { continue; } // Skip cloaked windows
```

**Handle ApplicationFrameHost.exe for UWP apps.** UWP windows report their PID as ApplicationFrameHost.exe, not the actual app. Use `GetApplicationUserModelId` or `GetPackageFamilyName` to resolve the real app identity. For `list_windows`, this means the PID in `WindowInfo` may not match the app that owns the content.

**EnumWindows filters out UWP windows (Windows 8+).** Use `FindWindowEx` loop as fallback to discover `ApplicationFrameWindow` class windows, then filter cloaked ones via `DwmGetWindowAttribute(DWMWA_CLOAKED)`.

**Main window detection algorithm** (matches Alt+Tab/taskbar visibility):
1. Must be visible (`IsWindowVisible`)
2. Must not be cloaked
3. Must not be a tool window (`WS_EX_TOOLWINDOW`)
4. Must have no owner (unless `WS_EX_APPWINDOW` is set)
5. Must not be the shell/desktop window

**Window ID generation.** Use the same `FxHasher` pattern as macOS: `hash(pid, title)` → `w-{hex}`.

---

Trait methods implemented: `list_windows`, `list_apps`, `focused_window`

**1.3. Element finding and state queries** (`tree/resolve.rs`)

```
tree/resolve.rs — resolve_element via UIMatcher (pid, control_type, name, bounds_hash)
                  Also used by find, get, is commands (which use SnapshotEngine in core + adapter.resolve_element)
```

### Research Insights: Element Resolution

**Use server-side `FindFirst`, not client-side tree walk.** For `resolve_element`, build a `UIMatcher` condition combining `ProcessId`, `ControlType`, and `Name`, then call `find_first(TreeScope::Subtree, &condition)`. This is a single cross-process call instead of walking potentially thousands of elements.

**Store `RuntimeId` in RefEntry for fast verification.** UIA elements have a `RuntimeId` property (an `int[]` array). If the RuntimeId matches, the element is confirmed valid without fuzzy matching. Falls back to `(pid, role, name, bounds_hash)` when RuntimeId changes (UI rebuilt).

**Virtualized control awareness.** WPF/UWP `ListView` and `DataGrid` use UI virtualization — only visible items have UIA elements. When `scroll` moves content, previously-resolved refs to list items may point to recycled containers. Return `STALE_REF` and let the agent re-snapshot.

---

Trait methods implemented: `resolve_element`, `get_live_value`, `get_element_bounds`

**1.4. Permissions check** (`system/permissions.rs`)

UIA doesn't require TCC-like permissions for most apps. Check:
- COM initialization succeeds
- Can enumerate at least one window
- Return `PermissionReport::Granted` or `Denied` with Windows-specific guidance

### Research Insights: Permissions

**UIA reads work across UIPI boundaries without special permissions.** Unlike macOS TCC, no explicit user grant is needed for UIA read access. The permission check is essentially a smoke test that COM works and the desktop is accessible.

**Report UIPI limitations proactively.** When `check_permissions` detects the process is not elevated but elevated apps exist, add a note to the permission status: "Some elevated applications may not respond to input commands. Run as administrator for full control."

---

Trait methods implemented: `check_permissions`

**1.5. Surface detection** (`tree/surfaces.rs`)

Detect Windows surface types:
- Menu bars (UIA Menu/MenuBar control type)
- Modal dialogs (Window with IsModal=true)
- Popups/Flyouts (Pane with IsKeyboardFocusable)

### Research Insights: Surface Mapping

**SnapshotSurface → Windows mapping:**
| SnapshotSurface | Windows Detection | Notes |
|---|---|---|
| `Window` | `UIElement` from HWND | Direct equivalent |
| `Focused` | `get_focused_element()` → walk to nearest surface | Same as macOS |
| `Menu` | `ControlType == Menu` or `MenuBar` | Direct equivalent |
| `Menubar` | `ControlType == MenuBar` | Direct equivalent |
| `Alert` | `ControlType == Window` with `IsModal == true` | Maps to modal dialogs |
| `Sheet` | **No Windows equivalent** | Return `ElementNotFound("No sheet on Windows")` |
| `Popover` | **No Windows equivalent** | Return `ElementNotFound("No popover on Windows")` |

---

Trait methods implemented: `list_surfaces`

**1.6. Screenshot** (`system/screenshot.rs`)

Use `PrintWindow` with `PW_RENDERFULLCONTENT` for window-specific capture, `BitBlt` for screen capture. `GetDIBits` to extract raw pixel data. Encode to PNG via the same base64 pipeline as macOS.

### Research Insights: Screenshot

**Use `PrintWindow` with `PW_RENDERFULLCONTENT` (flag value `2`) for per-window capture.** Plain `BitBlt` returns black pixels for DWM-composited windows (most modern apps). `PrintWindow` with this flag requests the window to render its full content to the provided DC.

**Fallback chain:** `PrintWindow(PW_RENDERFULLCONTENT)` → `PrintWindow(0)` → `BitBlt` (last resort).

**Multi-monitor support.** For `ScreenshotTarget::Screen(idx)`, use `EnumDisplayMonitors` to get monitor rects, then `BitBlt` with the correct source coordinates. `ScreenshotTarget::FullScreen` should capture the virtual screen (`GetSystemMetrics(SM_XVIRTUALSCREEN/SM_YVIRTUALSCREEN/SM_CXVIRTUALSCREEN/SM_CYVIRTUALSCREEN)`).

**DPI awareness.** Call `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` early in adapter initialization. Without this, screenshot coordinates and dimensions may be scaled incorrectly on high-DPI displays.

---

Trait methods implemented: `screenshot`

**After Phase 1:** `snapshot --app Explorer` returns valid JSON with refs. `list-windows`, `list-apps`, `find`, `get`, `is`, `list-surfaces`, `screenshot`, `permissions` all work.

---

#### Phase 2: Core Interaction Tier

Enable agents to ACT on elements. Pattern-based UIA actions.

**2.1. Action dispatch** (`actions/dispatch.rs`)

Central `perform_action(handle, action)` that matches `Action` variants to UIA patterns:
```
Action::Click        → UIInvokePattern::invoke()
Action::SetValue(v)  → UIValuePattern::set_value(v)
Action::SetFocus     → UIElement::set_focus()
Action::Expand       → UIExpandCollapsePattern::expand()
Action::Collapse     → UIExpandCollapsePattern::collapse()
Action::Select(v)    → UISelectionItemPattern::select()
Action::Toggle       → UITogglePattern::toggle()
Action::Check        → UITogglePattern (set to On)
Action::Uncheck      → UITogglePattern (set to Off)
Action::Clear        → UIValuePattern::set_value("")
```

### Research Insights: Action Dispatch

**NativeHandle COM pointer lifecycle (Critical):**

On Windows, `NativeHandle` wraps a `*const c_void` that points to a `UIElement` (which wraps `IUIAutomationElement` COM interface). The lifecycle contract:

```rust
// Creating a NativeHandle from UIElement:
fn element_to_handle(el: UIElement) -> NativeHandle {
    let boxed = Box::new(el);
    NativeHandle::from_ptr(Box::into_raw(boxed) as *const std::ffi::c_void)
}

// Recovering UIElement from NativeHandle (non-owning):
unsafe fn handle_to_element(handle: &NativeHandle) -> &UIElement {
    &*(handle.as_raw() as *const UIElement)
}

// The UIElement's Drop impl calls IUIAutomationElement::Release() (COM Release)
// ManuallyDrop prevents double-free when temporarily borrowing
```

**Important:** The macOS adapter uses `ManuallyDrop::new(AXElement(...))` to prevent double-free when temporarily borrowing the handle. The Windows adapter must use the same pattern. Never call `Box::from_raw` on a handle unless you intend to take ownership and free it.

**Pattern fallback chain:**
When `InvokePattern` is unavailable for a Click action (some legacy controls), fall back to:
1. `LegacyIAccessiblePattern::do_default_action()` — MSAA bridge
2. Coordinate click via `SendInput` at element center — last resort

**Check/Uncheck via TogglePattern:**
`TogglePattern::toggle()` cycles through states (On → Off → Indeterminate → On). For `Check`, call `toggle()` only if current state is not `On`. For `Uncheck`, call `toggle()` only if current state is not `Off`. Read `get_toggle_state()` first.

**HRESULT → ErrorCode mapping:**
| HRESULT | ErrorCode | Context |
|---|---|---|
| `UIA_E_ELEMENTNOTAVAILABLE` (0x80040201) | `StaleRef` | Element no longer in UI tree |
| `UIA_E_ELEMENTNOTENABLED` (0x80040200) | `ActionFailed` | Element disabled |
| `UIA_E_NOTSUPPORTED` (0x80040204) | `ActionNotSupported` | Pattern not available |
| `UIA_E_INVALIDOPERATION` (0x80131509) | `ActionFailed` | Operation invalid in current state |
| `UIA_E_TIMEOUT` (0x80131505) | `Timeout` | Provider took too long |
| `E_ACCESSDENIED` (0x80070005) | `PermDenied` | UIPI or security restriction |
| `E_FAIL` (0x80004005) | `Internal` | Generic COM failure |
| `E_INVALIDARG` (0x80070057) | `InvalidArgs` | Bad argument to UIA call |

---

**2.2. Activation chain** (`actions/activate.rs`)

Smart activation: perform UIA patterns without foreground activation by default. Coordinate click via `SendInput` is reserved for an explicit physical policy path.

### Research Insights: Window Activation

**`SetForegroundWindow` restrictions.** Windows prevents background processes from stealing foreground. The call succeeds only if:
- The calling process is the foreground process
- The foreground process has called `AllowSetForegroundWindow` for the caller
- The user is not interacting with another window

**Workaround chain:**
1. `AttachThreadInput` to the foreground thread
2. `SetForegroundWindow(target)`
3. `BringWindowToTop(target)`
4. `SetFocus` on the specific element via UIA

---

**2.3. Type text** (`input/keyboard.rs`)

`Action::TypeText(s)` → iterate characters, use `SendInput` with `KEYEVENTF_UNICODE` flag for each. Handle delay between keystrokes (`--delay` flag).

### Research Insights: SendInput Batching (Major Performance)

**Batch all `INPUT` structs in a single `SendInput` call for 10-50x performance improvement.**

Instead of:
```rust
// SLOW: one cross-process call per character
for ch in text.chars() {
    SendInput(&[key_down(ch)], size); // cross-process call
    SendInput(&[key_up(ch)], size);   // cross-process call
}
```

Do:
```rust
// FAST: single cross-process call for entire text
let mut inputs = Vec::with_capacity(text.len() * 2);
for ch in text.chars() {
    inputs.push(key_down_unicode(ch));
    inputs.push(key_up_unicode(ch));
}
SendInput(&inputs, size_of::<INPUT>() as i32); // ONE call
```

**Check `SendInput` return value.** It returns the number of events successfully inserted. If 0, input was blocked (UIPI). Detect elevation mismatch and return `PERM_DENIED` with actionable guidance.

---

Trait methods implemented: `execute_action` (all interaction commands use this)

**After Phase 2:** `click @e3`, `type @e5 "hello"`, `set-value @e5 "world"`, `toggle @e7`, `select @e8 "Option A"` all work.

---

#### Phase 3: Input Synthesis Tier

Raw keyboard and mouse input, independent of accessibility patterns.

**3.1. Keyboard synthesis** (`input/keyboard.rs`)

- `press` — `SendInput` with virtual key codes + modifier mapping (Cmd→Ctrl)
- `key-down` / `key-up` — individual key events
- Windows-specific blocked combos: `alt+f4`, `ctrl+alt+delete`, `win+l`, `win+r`

### Research Insights: Keyboard Synthesis

**Cmd→Ctrl mapping contract.** When agent sends `cmd+c`, the Windows adapter must:
1. Map `cmd` modifier → `VK_CONTROL`
2. Map the combo `cmd+c` → `ctrl+c`
3. This is a display-only mapping — the JSON output should show the actual keystroke sent (`ctrl+c`)

**Virtual key code mapping for common keys:**
```
VK_CONTROL (0x11), VK_SHIFT (0x10), VK_MENU (0x12=Alt)
VK_LWIN (0x5B), VK_RETURN (0x0D), VK_TAB (0x09), VK_ESCAPE (0x1B)
VK_BACK (0x08), VK_DELETE (0x2E), VK_SPACE (0x20)
VK_UP/DOWN/LEFT/RIGHT (0x26-0x28, 0x25)
VK_HOME (0x24), VK_END (0x23), VK_PRIOR (0x21=PageUp), VK_NEXT (0x22=PageDown)
VK_F1-VK_F12 (0x70-0x7B)
```

**DoubleClick/TripleClick/RightClick via SendInput.** These are NOT UIA pattern operations — they require raw mouse input synthesis:
- DoubleClick: two `LEFTDOWN`+`LEFTUP` pairs with <500ms gap at same coordinates
- TripleClick: three `LEFTDOWN`+`LEFTUP` pairs
- RightClick: `RIGHTDOWN`+`RIGHTUP` pair

---

**3.2. Mouse synthesis** (`input/mouse.rs`)

- `hover` — `SendInput` MOUSEEVENTF_MOVE + MOUSEEVENTF_ABSOLUTE
- `mouse-move`, `mouse-click`, `mouse-down`, `mouse-up` — direct SendInput
- `drag` — mouse down → series of moves → mouse up

### Research Insights: Mouse Coordinate Translation

**DPI awareness is critical.** SendInput uses normalized absolute coordinates (0-65535 range), not pixels:
```rust
let abs_x = (pixel_x * 65535) / screen_width;
let abs_y = (pixel_y * 65535) / screen_height;
```

**Multi-monitor.** Coordinates must account for the virtual screen origin. Use `MOUSEEVENTF_VIRTUALDESK` flag:
```rust
let virt_left = GetSystemMetrics(SM_XVIRTUALSCREEN);
let virt_top = GetSystemMetrics(SM_YVIRTUALSCREEN);
let virt_width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
let virt_height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
// Correct formula: divide by (size - 1), not size
let abs_x = ((pixel_x - virt_left) * 65535) / (virt_width - 1);
let abs_y = ((pixel_y - virt_top) * 65535) / (virt_height - 1);
// Set MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK on the INPUT struct
```

**UIA BoundingRectangle returns physical pixels** when DPI-aware (`SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`). No conversion needed between UIA bounds and SendInput target coordinates.

**Double/triple click via SendInput.** Batch all events in a single call — no `Sleep` needed between clicks:
- DoubleClick: 4 `INPUT` events (`LBUTTONDOWN`, `LBUTTONUP`, `LBUTTONDOWN`, `LBUTTONUP`) in one `SendInput` call
- TripleClick: 6 `INPUT` events in one `SendInput` call

---

**3.3. Scroll** (`actions/extras.rs` + `input/mouse.rs`)

- `scroll` — `UIScrollPattern::scroll()` for element scroll, `SendInput` MOUSEEVENTF_WHEEL for window scroll
- `scroll-to` — `UIScrollPattern::set_scroll_percent()` or repeated scroll events

Trait methods implemented: `press_key_for_app`, `mouse_event`, `drag`

**After Phase 3:** All keyboard, mouse, scroll, and drag commands work.

---

#### Phase 4: System Operations Tier

App lifecycle, window management, clipboard.

**4.1. App lifecycle** (`system/app_ops.rs`)

- `launch` — `CreateProcessW` for paths, `ShellExecuteW` for app names. `WaitForInputIdle` for `--wait` flag. Returns `WindowInfo` of first window.
- `close-app` — `WM_CLOSE` for graceful, `TerminateProcess` for `--force`

### Research Insights: App Lifecycle Security

**CRITICAL: Use `CreateProcessW` as primary launch method.** `CreateProcessW` with non-NULL `lpApplicationName` does NOT parse shell metacharacters — safe for untrusted input. Always pass the full resolved path as `lpApplicationName`.

**Path resolution chain:**
1. App Paths registry (`HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{name}.exe`) — fastest lookup for installed apps
2. `SearchPathW` API — searches system PATH, Windows directory
3. PATH environment variable — manual split and search

**`ShellExecuteW` only as fallback** (for UWP apps, protocol handlers). When falling back:
- Always pass the app name as `lpFile` (not `lpParameters`) and leave `lpParameters` null
- Reject shell metacharacters in `lpFile`: `&`, `|`, `>`, `<`, `;`, `` ` ``
- Do NOT use the regex `[a-zA-Z0-9._-]+(.exe)?` — it rejects legitimate paths containing spaces (e.g., `C:\Program Files\App Name\app.exe`)
- Instead, reject only the 6 metacharacters that trigger `cmd.exe` interpretation when `lpFile` resolves to a `.bat`/`.cmd` file

**UWP apps:** Use `IApplicationActivationManager::ActivateApplication` with the app's AUMID (Application User Model ID). `CreateProcessW` does not work for UWP apps.

**Windows protected processes (PPL).** `TerminateProcess` fails with `ERROR_ACCESS_DENIED` on Protected Process Light (PPL) processes. `OpenProcess(PROCESS_TERMINATE)` itself fails — you never get a handle. Detect protection level via `GetProcessInformation(ProcessProtectionLevelInfo)`. Protected processes include: antivirus (Antimalware signer), LSASS, csrss.exe, smss.exe. `SeDebugPrivilege` does NOT bypass PPL.

**Graceful close always works:** `WM_CLOSE` sent via `PostMessage` works even for protected processes because it's a window message, not a process operation. For `close-app --force`, try `TerminateProcess` first; if `ERROR_ACCESS_DENIED`, fall back to `WM_CLOSE` and report protection status.

**`WaitForInputIdle` caveats (Raymond Chen).** This is a one-shot function — once a process reaches "input idle" (first `GetMessage` call), subsequent calls return immediately even if busy. Unreliable for multi-threaded apps (splash screen thread can trigger it). Combine with UIA tree polling for reliable readiness detection.

---

**4.2. Window operations** (`system/window_ops.rs`)

- `focus-window` — `SetForegroundWindow` + `BringWindowToTop`
- `resize-window` — `SetWindowPos` with new dimensions
- `move-window` — `SetWindowPos` with new coordinates
- `minimize` / `maximize` / `restore` — `ShowWindow` with `SW_MINIMIZE` / `SW_MAXIMIZE` / `SW_RESTORE`

**4.3. Clipboard** (`input/clipboard.rs`)

- `clipboard-get` — `OpenClipboard` + `GetClipboardData(CF_UNICODETEXT)` + `GlobalLock/Unlock`
- `clipboard-set` — `OpenClipboard` + `EmptyClipboard` + `SetClipboardData(CF_UNICODETEXT)`
- `clipboard-clear` — `OpenClipboard` + `EmptyClipboard`

### Research Insights: Clipboard Safety

**CRITICAL: RAII `ClipboardGuard` with retry.**

`OpenClipboard` takes a global system lock. If the process crashes between `OpenClipboard` and `CloseClipboard`, the clipboard is locked system-wide.

```rust
struct ClipboardGuard;

impl ClipboardGuard {
    fn open() -> Result<Self, AdapterError> {
        // Retry up to 10 times with 100ms delay (matches .NET Clipboard.SetDataObject default)
        // OpenClipboard returns ERROR_ACCESS_DENIED (5) when another app holds the lock
        for attempt in 0..10 {
            if unsafe { OpenClipboard(HWND::default()) }.is_ok() {
                return Ok(Self);
            }
            if attempt < 9 {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        Err(AdapterError::action_failed("Clipboard locked by another application after 10 retries")
            .with_suggestion("Close clipboard-intensive applications and retry"))
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe { CloseClipboard().ok() };
    }
}
```

**Always use UTF-16 (`CF_UNICODETEXT`).** Never use `CF_TEXT` (ANSI) — Windows auto-converts between them via synthesized formats, but `CF_UNICODETEXT` preserves full Unicode.

**Thread safety:** `OpenClipboard` and `CloseClipboard` MUST be called from the same thread. Different threads in the same process are treated as different owners. The RAII guard must not be sent across threads.

**`GlobalAlloc` ownership transfer:** After `SetClipboardData(CF_UNICODETEXT, hmem)` succeeds, ownership transfers to the system — do NOT call `GlobalFree`. If `SetClipboardData` fails, you still own the memory and must free it.

---

**4.4. Wait** (`system/wait.rs`)

- `wait --element @e5` — poll `resolve_element` until success or timeout
- `wait --window "Title"` — poll `list_windows` with title filter
- `wait --text @e5 "expected"` — poll `get_live_value` until match
- `wait --menu` — poll `list_surfaces` for menu surface

### Research Insights: Wait Command Cleanup

**`wait --gone` does not exist.** The original plan referenced `wait --gone @e5` but this is not an implemented command variant. Remove from the plan.

**`wait --text` performance on large trees.** Each poll calls `get_live_value` which resolves the element and reads its value. This is fast for a single element but creates COM overhead per poll iteration. Use a polling interval of 200ms (not 100ms) to reduce COM pressure.

---

Trait methods implemented: `launch_app`, `close_app`, `focus_window`, `window_op`, `get_clipboard`, `set_clipboard`, `clear_clipboard`, `wait_for_menu`

**After Phase 4:** All 50 commands functional on Windows.

---

#### Phase 5: Polish & Edge Cases

**5.1. Chromium detection** (in `tree/builder.rs` or `adapter.rs`)

When `get_tree` returns a suspiciously small tree (< 5 nodes) for a known Chromium process, add a warning to the snapshot output suggesting `--force-renderer-accessibility`.

### Research Insights: Chromium Detection Update (Chrome 138+)

**Chrome 138+ (shipped July 2025) enables native UIA by default.** The detection strategy needs updating:

1. **For Chrome/Edge >= 138:** Native UIA support is on. If tree is sparse, the renderer may not have activated yet — **return the sparse tree immediately with a warning**, and let the AI agent decide whether to retry. Do not silently wait or hide latency.

2. **For Electron apps:** Electron lags behind Chrome releases. Many Electron apps (VS Code, Slack, Discord) may still be on older Chromium versions. Keep the `--force-renderer-accessibility` guidance for Electron.

3. **Enterprise policy override:** `UiAutomationProviderEnabled` policy can disable native UIA and revert to MSAA bridge. Supported through Chrome 146.

4. **Version detection:** Use `GetFileVersionInfoW` + `VerQueryValueW` on the process exe path to read the Chromium major version. For WebView2, use `GetAvailableCoreWebView2BrowserVersionString`. There is no UIA property that exposes the Chromium version — file version is the only reliable method.

**Updated detection flow (no hidden latency):**
```
if tree_size < 5 && is_chromium_process(pid) {
    let version = get_chromium_version_from_exe(pid);
    if is_electron_app(process_name) {
        warning = "Electron app may need --force-renderer-accessibility flag"
    } else if version >= 138 {
        warning = "Chromium renderer initializing. Re-run snapshot to get full tree."
    } else {
        warning = "Chromium < 138: launch with --force-renderer-accessibility"
    }
    // Return the sparse tree WITH the warning — let the agent decide to retry
}
```

**Known Chromium process names:**
- Browsers: `chrome.exe`, `msedge.exe`, `brave.exe`, `vivaldi.exe`
- Electron: `electron.exe`, `code.exe`, `slack.exe`, `discord.exe`, `teams.exe`

---

**5.2. Adapter-level blocked combos**

Override `blocked_combos()` returning:
```rust
&["alt+f4", "ctrl+alt+delete", "win+l", "win+r", "ctrl+shift+esc"]
```

**5.3. App name resolution**

Windows apps can be identified by:
- Process name (e.g., `notepad.exe`)
- Window title (e.g., `Untitled - Notepad`)
- App user model ID for UWP/Store apps

The `launch` and `close-app` commands need to handle all three. Use `shell:AppsFolder` for modern app resolution.

**5.4. UWP / Windows Store app support**

UWP apps run in AppContainer sandboxes and may have restricted UIA access. Detect and return `PERM_DENIED` with guidance when UIA calls fail for containerized apps.

### Research Insights: UWP Considerations

**UIA read access works fine across AppContainer boundaries** — the UIA framework handles the security boundary transparently. `SendInput` may be unreliable to sandboxed UWP windows. `SetForegroundWindow` may require `AllowSetForegroundWindow` token.

**WinUI3 desktop apps** (Windows App SDK) run as standard Win32 processes — no sandbox, no special handling needed.

---

## Alternative Approaches Considered

| Approach | Rejected Because |
|---|---|
| Raw `windows` crate only | ~30% more boilerplate for UIA operations; BSTR handling, COM factory code, condition building all done manually (see brainstorm) |
| `uiautomation` crate only | Doesn't wrap SendInput, clipboard, screenshot, process lifecycle at the level we need |
| AccessBridge for Java apps | Out of scope for Phase 2; Java apps have their own accessibility bridge |
| MSAA (legacy API) | UIA is the modern replacement; MSAA has no CacheRequest, pattern matching, or structured tree walking |

## System-Wide Impact

### Interaction Graph

When a Windows command executes:
1. `main.rs` calls `build_adapter()` → `WindowsAdapter::new()` (no-op, stateless)
2. `dispatch.rs` calls `adapter.method()` → `WindowsAdapter` creates `UIAutomation::new()` (COM init)
3. Method performs UIA/Win32 operations, returns `Result<Value, AppError>`
4. `UIAutomation` instance drops → COM cleanup (automatic via `uiautomation`)
5. JSON envelope written to stdout

No callbacks, no observers, no event handlers. Fully synchronous, stateless per invocation.

### Error & Failure Propagation

| Layer | Error Type | Propagation |
|---|---|---|
| `uiautomation` | `uiautomation::Error` | Caught in adapter, mapped to `AdapterError` with HRESULT in `platform_detail` |
| `windows` crate | `windows::core::Error` | Caught in adapter, mapped to `AdapterError` with HRESULT |
| Win32 API | `GetLastError` | Retrieved via `windows::core::Error::from_win32()`, mapped to `AdapterError` |
| COM failure | HRESULT | Mapped to appropriate `ErrorCode` (see HRESULT mapping table in Phase 2 insights) |

### Research Insights: Error Handling Pattern

**Standardized error conversion functions:**
```rust
fn win_err_to_adapter(context: &str, e: windows::core::Error) -> AdapterError {
    AdapterError::action_failed(context)
        .with_platform_detail(format!("HRESULT 0x{:08X}: {}", e.code().0, e.message()))
}

fn uia_err_to_adapter(context: &str, e: uiautomation::Error) -> AdapterError {
    // Map specific HRESULT values to appropriate ErrorCode
    AdapterError::action_failed(context)
        .with_platform_detail(format!("UIAutomation: {}", e))
}
```

Place these in a shared `crates/windows/src/error_mapping.rs` (new file, ~60 LOC).

---

### State Lifecycle Risks

Minimal. The adapter is stateless:
- No COM objects stored across calls (created per-call, dropped before return)
- No persistent handles (NativeHandle in RefMap is re-resolved on use)
- No file locks (RefMap uses atomic write via temp + rename, already Windows-compatible)

Only risk: clipboard operations use `OpenClipboard/CloseClipboard` which is a global lock. Must always close in a `Drop` guard to prevent deadlock.

### Research Insights: RefMap on Windows

**File permissions.** RefMap save has `#[cfg(not(unix))]` branches but uses default permissions on Windows. Windows file ACLs are more complex than Unix modes. For Phase 2, default permissions are acceptable — the file is in `%USERPROFILE%\.agent-desktop\` which is already user-private. Document that enterprise environments may need additional ACL controls.

---

### API Surface Parity

All 50 CLI commands produce **identical JSON output** on macOS and Windows. The `version` command reports the platform. The `status` command reports platform-specific permission status. No command has different flags or arguments per platform.

### Research Insights: Parity Gaps

**Commands with undefined Windows behavior (must document):**
- `SnapshotSurface::Sheet` → return `ElementNotFound` (macOS-only concept)
- `SnapshotSurface::Popover` → return `ElementNotFound` (macOS-only concept)
- `AppInfo.bundle_id` → always `None` on Windows
- DoubleClick/TripleClick/RightClick → must use `SendInput` (UIA has no pattern for these)

**JSON schema should be ~95% identical.** The 5% difference is in platform-specific fields that are already `Option<T>` (like `bundle_id`).

---

## Acceptance Criteria

### Tier 1: Windows Adapter Core (must-pass for each phase merge)

These must pass before ANY Windows PR is merged. Validated per-phase:

**Phase 0 (pre-work PR):**
- [ ] `cargo clippy --all-targets -- -D warnings` passes with zero warnings
- [ ] `cargo test --lib --workspace` passes (all existing tests still green)
- [ ] `cargo tree -p agent-desktop-core` contains no platform crate names
- [ ] `cargo check` passes on macOS (cfg-gate stubs work)
- [ ] Windows CI job (`cargo check -p agent-desktop-windows`) passes
- [ ] `blocked_combos` method added to trait; `key_down`/`key_up` check it (safety fix)
- [ ] `WindowInfo.pid` and `AppInfo.pid` changed to `u64`
- [ ] Dead `permission_denied()` removed
- [ ] `error_mapping.rs` created with HRESULT→ErrorCode table

**Phase 1 (observation PR):**
- [ ] `snapshot --app Explorer` returns valid JSON with refs for all interactive elements
- [ ] `snapshot --app Notepad` returns editable textfield with ref
- [ ] `snapshot --app Settings` returns valid tree for modern Windows app
- [ ] `list-windows` returns all visible windows with titles and PIDs
- [ ] `list-apps` returns all running applications
- [ ] `find "Save" --role button` finds elements matching query
- [ ] `get text @eN` returns element's accessible name
- [ ] `is visible @eN` / `is enabled @eN` / `is checked @eN` return boolean
- [ ] `list-surfaces` detects menu bars, dialogs, and popups
- [ ] `screenshot` returns base64-encoded PNG
- [ ] `permissions` reports UIA availability status
- [ ] Explorer snapshot completes in < 2 seconds (with CacheRequest)
- [ ] No file in `crates/windows/` exceeds 400 LOC
- [ ] No `unwrap()` in non-test code

**Phase 2-3 (interaction + input PR):**
- [ ] `click @eN` invokes the element via InvokePattern
- [ ] `type @eN "hello"` types text into focused element
- [ ] `set-value @eN "world"` sets value via ValuePattern
- [ ] `press ctrl+c` sends Ctrl+C keystroke
- [ ] `press cmd+c` maps to Ctrl+C on Windows (cross-platform parity)
- [ ] `SendInput` return value checked — `PERM_DENIED` on UIPI failure with elevation guidance

**Phase 4-5 (system + polish PR):**
- [ ] `clipboard-get` / `clipboard-set` / `clipboard-clear` roundtrip correctly
- [ ] Clipboard operations use RAII guard — no system-wide lock on crash
- [ ] `launch notepad` opens Notepad and returns WindowInfo
- [ ] `close-app notepad` closes Notepad gracefully
- [ ] `close-app notepad --force` terminates Notepad process
- [ ] `CreateProcessW` used as primary launch method; `ShellExecuteW` fallback rejects metacharacters
- [ ] Protected processes return `PERM_DENIED` with explanation (not crash)

### Tier 2: Windows Adapter Complete (must-pass for Phase 2 sign-off)

These must ALL pass before Phase 2 is declared complete:

**Functional completeness:**
- [ ] `focus-window --app notepad` brings Notepad to foreground
- [ ] `minimize`, `maximize`, `restore` operate on target window
- [ ] `resize-window` and `move-window` change window geometry
- [ ] `wait --element @eN` polls until element exists
- [ ] `wait --window "Title"` polls until window appears
- [ ] `batch` executes multiple commands in sequence
- [ ] All P2 commands from PRD (hover, drag, mouse-*, key-down/up, window geometry, advanced waits) work
- [ ] Chromium apps with sparse trees get a version-aware warning (returned immediately, no hidden wait)
- [ ] `SnapshotSurface::Sheet` returns `ElementNotFound` with clear message
- [ ] `SnapshotSurface::Popover` returns `ElementNotFound` with clear message

**Non-functional:**
- [ ] JSON output schema identical to macOS for all commands
- [ ] Binary size < 15MB (release build with `opt-level = "z"`, LTO, strip)
- [ ] Notepad snapshot completes in < 500ms
- [ ] All errors carry `ErrorCode`, `message`, and `suggestion`
- [ ] Every `.rs` file has cfg-gate stubs for cross-platform compilation

**Quality gates:**
- [ ] Windows CI job passes on every PR
- [ ] Release binary builds for `x86_64-pc-windows-msvc`
- [ ] Integration tests pass on `windows-latest` runner (Explorer, Notepad, Settings, VS Code)
- [ ] `cargo test --lib -p agent-desktop-windows` passes

## Success Metrics

| Metric | Target |
|---|---|
| P2-O1: Windows adapter | `snapshot` on Explorer, Notepad, Settings returns valid trees with refs |
| P2-O3: Cross-platform JSON parity | Identical schema output on macOS and Windows for all commands |
| P2-O4: Phase 2 commands ship | hover, drag, mouse-*, key-down/up, window geometry, advanced waits all working |
| P2-O5: Cross-platform CI | GitHub Actions matrix: macOS + Windows |
| P2-O6: Performance | Explorer snapshot < 2s, Notepad snapshot < 500ms (with CacheRequest) |

## Dependencies & Prerequisites

| Dependency | Version | Purpose | Risk |
|---|---|---|---|
| `uiautomation` | 0.24+ | UIA tree, patterns, element finding | Single maintainer; escape hatch via `Into<IUIAutomationElement>` |
| `windows` | 0.62+ | Win32 APIs (SendInput, clipboard, GDI, process) | Microsoft-backed, low risk |
| `rustc-hash` | workspace | FxHashSet for cycle prevention in tree traversal | Already used by macOS adapter |
| Windows 10 1809+ | N/A | Minimum OS version (per PRD §2.3) | |
| `windows-latest` CI runner | N/A | GitHub Actions Windows runner | Available, no cost concern |

## Risk Analysis & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `uiautomation` crate abandoned | Low | Medium | Escape hatch to raw `windows` COM. Thin wrapper, migration path is incremental. |
| COM apartment conflicts | Medium | High | Stateless adapter, per-call init. No stored COM state. `new_direct()` fallback if MTA already initialized. |
| UWP/Store app sandboxing blocks UIA | Medium | Medium | UIA reads work across AppContainer. `SendInput` may fail — detect and return `PERM_DENIED`. |
| Chromium apps have empty trees | Medium | Medium | Chrome 138+ has native UIA. Electron apps need `--force-renderer-accessibility`. Version-aware detection. |
| Windows CI flakiness (GUI tests) | Medium | Low | `--test-threads=1`, use Notepad (not Calculator), explicit process cleanup. |
| Binary size exceeds 15MB with Windows deps | Low | Low | `windows` crate features are granular. Only enable needed features. |
| Cross-compile from macOS fails | Low | Medium | Use `x86_64-pc-windows-gnu` for `cargo check`. Full build on CI only. |
| UIPI blocks `SendInput` to elevated apps | High | Medium | Check return value, detect elevation, return `PERM_DENIED` with "run as administrator" guidance. |
| Hung/frozen app blocks UIA call indefinitely | Medium | High | Wrap UIA calls in thread+channel timeout (5s default). Return `TIMEOUT` error. |
| Cloaked windows pollute `list_windows` | Medium | Low | Filter via `DwmGetWindowAttribute(DWMWA_CLOAKED)`. |
| ApplicationFrameHost.exe PID mismatch | Medium | Medium | Detect UWP host process, resolve real app PID. Document limitation. |

## Testing Strategy

### Unit Tests (`crates/windows/`)

All unit tests use `#[cfg(target_os = "windows")]` guards — they only run on Windows CI.

- `tree/roles.rs` — ControlType → role mapping coverage (every mapped type, including new additions)
- `tree/builder.rs` — Tree depth limiting, cycle prevention
- `input/keyboard.rs` — Cmd→Ctrl modifier mapping, virtual key code mapping, blocked combo filtering
- `input/clipboard.rs` — Clipboard roundtrip (get/set/clear) with RAII guard verification
- `actions/dispatch.rs` — Action → Pattern mapping coverage
- `system/permissions.rs` — Permission check returns valid status
- `error_mapping.rs` — HRESULT → ErrorCode mapping for all known values (created in Phase 0, tested from Phase 0)

### Integration Tests (`tests/integration/`)

Run on `windows-latest` CI only:

- Snapshot Explorer — non-empty tree with refs
- Snapshot Notepad — textfield with editable value
- Snapshot Settings — valid tree for modern Windows app
- Click button in test app — verify action succeeded
- Type text into Notepad — verify content changed
- Clipboard get/set roundtrip
- Launch/close Notepad lifecycle
- List windows — at least one window returned
- List apps — at least one app returned
- Screenshot — returns non-empty base64 PNG
- Window operations (minimize, maximize, restore) on Notepad
- Snapshot VS Code — Electron app, validates Chromium edge case detection and large tree performance

### Research Insights: Testing

**CI test targets:**
| App | Why | Notes |
|-----|-----|-------|
| `notepad.exe` | Guaranteed available, simple UI, has textfield | Perfect for type/set-value/click tests |
| `explorer.exe` | Always running | Good for list-windows, snapshot |
| `mspaint.exe` | Available, simple UI | Good for click, toolbar tests |
| `code.exe` (VS Code) | Electron app, most common dev tool | Validates Chromium detection, large tree handling. **Mark optional on CI** — may not be pre-installed |
| `calc.exe` | UWP app | **May not be available on CI — skip or make optional** |

**Use `--test-threads=1`.** Concurrent integration tests fighting over foreground windows and focus cause flakiness.

**Process cleanup.** Always `Stop-Process -Name ... -ErrorAction SilentlyContinue` after tests. Test failures that leave processes running will contaminate subsequent test runs.

---

### Cross-Platform Tests

- JSON schema validation — same golden fixtures pass on both platforms
- Error format validation — error JSON structure matches on both platforms
- Command flag parsing — CLI argument handling is platform-independent (already in core tests)

## File Manifest

### Files Modified (Phase 0 Only)

| File | Change |
|---|---|
| `crates/core/src/adapter.rs` | Add `blocked_combos` method to `PlatformAdapter` |
| `crates/core/src/commands/press.rs` | Use `adapter.blocked_combos()` instead of const |
| `crates/core/src/commands/key_down.rs` | Add blocked combo check |
| `crates/core/src/commands/key_up.rs` | Add blocked combo check |
| `crates/core/src/error.rs` | Remove dead `permission_denied()` |
| `crates/core/src/node.rs` | Change `pid: i32` → `pid: u64` in `WindowInfo` and `AppInfo` |
| `crates/macos/src/adapter.rs` | Override `blocked_combos()` with macOS list; `as u64` PID casts |
| `.github/workflows/ci.yml` | Add `test-windows` job |
| `crates/windows/Cargo.toml` | Add dependencies |

### Files Created (Phase 0)

| File | Purpose | Est. LOC |
|---|---|---|
| `crates/windows/src/error_mapping.rs` | HRESULT/UIA error → `AdapterError` conversion | ~60 |

### Files Created (Phases 1-5)

| File | Purpose | Est. LOC |
|---|---|---|
| `crates/windows/src/lib.rs` | Module declarations, re-export `WindowsAdapter` | ~30 |
| `crates/windows/src/adapter.rs` | `PlatformAdapter` impl, delegation to submodules | ~200 |
| `crates/windows/src/tree/mod.rs` | Re-exports | ~10 |
| `crates/windows/src/tree/element.rs` | `UIElement` wrapper, attribute readers | ~150 |
| `crates/windows/src/tree/builder.rs` | `build_subtree()` via `UITreeWalker` + `UICacheRequest` | ~200 |
| `crates/windows/src/tree/roles.rs` | `ControlType` → role string mapping (24 types) | ~140 |
| `crates/windows/src/tree/resolve.rs` | Element re-identification via `UIMatcher` + `RuntimeId` | ~120 |
| `crates/windows/src/tree/surfaces.rs` | Surface detection (menus, dialogs; Sheet/Popover → error) | ~120 |
| `crates/windows/src/actions/mod.rs` | Re-exports | ~10 |
| `crates/windows/src/actions/dispatch.rs` | `Action` → UIA Pattern dispatch with fallback chain | ~220 |
| `crates/windows/src/actions/activate.rs` | Window/element activation chain | ~100 |
| `crates/windows/src/actions/extras.rs` | Selection, Scroll, Toggle pattern helpers | ~150 |
| `crates/windows/src/input/mod.rs` | Re-exports | ~10 |
| `crates/windows/src/input/keyboard.rs` | `SendInput` keyboard + Cmd→Ctrl + batching | ~220 |
| `crates/windows/src/input/mouse.rs` | `SendInput` mouse + DPI-aware coordinate translation | ~170 |
| `crates/windows/src/input/clipboard.rs` | Win32 clipboard with RAII `ClipboardGuard` + retry | ~140 |
| `crates/windows/src/system/mod.rs` | Re-exports | ~10 |
| `crates/windows/src/system/app_ops.rs` | `CreateProcessW`, sanitized `ShellExecuteW`, `TerminateProcess` | ~170 |
| `crates/windows/src/system/window_ops.rs` | `EnumWindows` (cloaked filter), `ShowWindow`, `SetWindowPos` | ~220 |
| `crates/windows/src/system/key_dispatch.rs` | App-targeted key press via `SendInput` | ~80 |
| `crates/windows/src/system/permissions.rs` | COM smoke test + UIPI advisory | ~70 |
| `crates/windows/src/system/screenshot.rs` | `PrintWindow(PW_RENDERFULLCONTENT)` + `BitBlt` fallback | ~220 |
| `crates/windows/src/system/wait.rs` | `WaitForInputIdle`, polling loops | ~100 |
| **Total** | | **~3,050** |

### Research Insights: LOC Budget

**Realistic budget: 3,500-4,000 LOC.** The per-file estimates above total ~3,050 for implementation code. Add:
- cfg-gate stubs: ~15-20 LOC per file × ~20 files = ~350 LOC
- Frozen app detection (`IsHungAppWindow`, `IUIAutomation2` timeout config): ~80 LOC
- UWP launch via `IApplicationActivationManager`: ~60 LOC
- Protected process detection: ~40 LOC
- Chromium version detection via `GetFileVersionInfoW`: ~60 LOC
- DPI coordinate helpers: ~40 LOC
- Unit tests within modules: ~200+ LOC

**Total budget: ~3,500-4,000 LOC.** Well within the 400 LOC per file limit even with additions.

**New file: `error_mapping.rs`.** Consolidates all HRESULT→ErrorCode and UIA error→AdapterError conversion in one place. Prevents duplicating error mapping logic across modules.

---

## Documentation Plan

- Update `CLAUDE.md` to reflect 22+ trait methods (currently says 12)
- Update `CLAUDE.md` PlatformAdapter example to show all 22 methods
- Update `docs/phases.md` to mark Phase 2 status as "In Progress"
- Add Windows-specific troubleshooting section to README
- Document Chromium `--force-renderer-accessibility` guidance (version-aware)
- Document UIPI limitations and "run as administrator" guidance
- Document `SnapshotSurface::Sheet`/`Popover` as macOS-only in command docs

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-02-25-windows-adapter-phase2-brainstorm.md](docs/brainstorms/2026-02-25-windows-adapter-phase2-brainstorm.md) — Key decisions carried forward: layered `uiautomation` + `windows` crate choice, Cmd→Ctrl mapping, platform-specific blocked combos, stateless per-call COM strategy

### Internal References

- PlatformAdapter trait: `crates/core/src/adapter.rs:114-216`
- macOS adapter pattern: `crates/macos/src/adapter.rs`
- BLOCKED_COMBOS: `crates/core/src/commands/press.rs:8-14`
- NativeHandle: `crates/core/src/adapter.rs:58-91`
- CI workflow: `.github/workflows/ci.yml`
- Dead code: `crates/core/src/error.rs:107-115` (`permission_denied()`)

### External References

- PRD v2.0 Section 7.2: Windows Adapter specification
- `uiautomation` crate: https://crates.io/crates/uiautomation (v0.24.3)
- `windows` crate: https://crates.io/crates/windows (v0.62+)
- Windows UIA documentation: https://learn.microsoft.com/en-us/windows/win32/winauto/entry-uiauto-win32
- UIA Control Types: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-controltype-ids
- UIA Control Patterns: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-controlpattern-ids
- UIA Caching: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-cachingforclients
- UIA Threading: https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-threading
- Chrome 138+ Native UIA: https://developer.chrome.com/blog/windows-uia-support-update
- UIPI: https://learn.microsoft.com/en-us/troubleshoot/power-platform/power-automate/desktop-flows/ui-automation/uipi-issues
- SendInput: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput
- UIA Security Research (Akamai, Dec 2024): https://www.akamai.com/blog/security-research/2024/dec/2024-december-windows-ui-automation-attack-technique-evades-edr
- `uiautomation-rs` GitHub: https://github.com/leexgone/uiautomation-rs
- `windows-rs` GitHub: https://github.com/microsoft/windows-rs
