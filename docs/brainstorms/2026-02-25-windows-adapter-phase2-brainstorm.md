# Brainstorm: Phase 2 — Windows Adapter

**Date:** 2026-02-25
**Status:** Draft
**Phase:** 2 (Windows Adapter)
**Participants:** Lahfir, Claude

---

## What We're Building

A Windows `PlatformAdapter` implementation that enables all 50 agent-desktop commands to work on Windows via the UI Automation COM API. The adapter uses `uiautomation` (v0.24+) for UIA operations and the `windows` crate for low-level Win32 APIs. It follows the same architecture as the macOS adapter: implements the 22-method `PlatformAdapter` trait, lives entirely within `crates/windows/`, and never touches core or macOS code.

## Why This Approach

### Foundation Assessment

The Phase 1 architecture was purpose-built for this exact expansion:

- **PlatformAdapter trait**: 22 methods, all with default `not_supported()` returns — enables incremental implementation
- **Core isolation**: Zero platform leaks, CI-enforced via `cargo tree` check
- **Dispatch wiring**: `build_adapter()` and target-gated deps already handle Windows — zero changes needed
- **Windows stub**: Already compiles, implements the trait, and is a workspace member
- **Error codes**: `PlatformNotSupported` and `ActionNotSupported` both available
- **RefMap**: Already handles `USERPROFILE` for Windows paths, has `#[cfg(not(unix))]` fallback

### Technology Choice: `uiautomation` + `windows` (Layered)

As specified in the PRD (Section 7.2), using `uiautomation` (v0.24+) for UI Automation operations, complemented by the `windows` crate for low-level Win32 APIs.

**Why layered, not either/or:**

| Responsibility | Crate | Rationale |
|---|---|---|
| Tree traversal, CacheRequest, UIA patterns, element finding | `uiautomation` | ~30% less code than raw COM, safe API, auto COM init, escape hatch via `Into<IUIAutomationElement>` |
| Input synthesis (SendInput), clipboard, screenshot, process lifecycle, window mgmt | `windows` | `uiautomation` doesn't wrap these at the level we need |

**Key properties of `uiautomation`:**
- v0.24.3 (Jan 2026), 184 stars, 0 open issues, quarterly releases
- Thin wrapper (~9K SLoC) over `windows` crate — no new dependency layer
- Covers all 31 UIA control patterns (Invoke, Value, ExpandCollapse, Selection, Scroll, Toggle, etc.)
- `UITreeWalker` with `UICacheRequest` for batch attribute retrieval
- `UIMatcher` fluent API for element finding and condition building
- Escape hatch: `UIElement: Into<IUIAutomationElement>` — can drop to raw COM for any edge case
- Auto COM initialization via `UIAutomation::new()` (calls `CoInitializeEx`)

**Why not raw `windows` for everything:** For UIA operations (tree traversal, patterns, element finding), `uiautomation` eliminates ~330 LOC of unsafe boilerplate, BSTR handling, control type mapping, condition building, and COM factory code.

**Why not `uiautomation` for everything:** It doesn't wrap `SendInput`, `BitBlt`, `CreateProcessW`, or the Win32 clipboard API at the level agent-desktop needs.

**Dependency config:**
```toml
# crates/windows/Cargo.toml
[target.'cfg(target_os = "windows")'.dependencies]
uiautomation = { version = "0.24", default-features = false, features = ["pattern", "control"] }
windows = { version = "0.62", features = [
    "Win32_Foundation",
    "Win32_System_Threading",
    "Win32_System_Com",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_DataExchange",
    "Win32_Graphics_Gdi",
    "Win32_System_Memory",
] }
```

## Key Decisions

### 1. Modifier::Cmd → Ctrl on Windows

**Decision:** `Cmd` maps to `Ctrl` on Windows.

**Rationale:** Agents write cross-platform automations. `cmd+c` (macOS copy) should "just work" as `ctrl+c` on Windows. The mapping happens in the Windows adapter's key synthesis layer, not in core. The core `Modifier::Cmd` enum variant stays unchanged.

### 2. Platform-Specific Blocked Combo Lists

**Decision:** Each platform adapter defines its own blocked combos. Core provides a validation hook but not the list.

**Current state:** `BLOCKED_COMBOS` in `crates/core/src/commands/press.rs` is macOS-specific (`cmd+q`, `cmd+shift+q`, etc.).

**Change needed:** Move the blocked list concept into platform adapters. The Windows adapter blocks: `alt+f4`, `ctrl+alt+delete`, `win+l`, etc. The core keeps a trait method or shared validation function signature, but the actual combos are platform-owned.

### 3. Surfaces Are Cross-Platform

**Decision:** `list-surfaces` is fully implementable on Windows. No commands are inherently macOS-only except `pinch`.

**Windows surface equivalents:**
| macOS Surface | Windows Equivalent |
|---------------|-------------------|
| Menu bar | Win32 menus / UIA Menu pattern |
| Sheet | Modal dialog |
| Alert/Dialog | MessageBox / Task Dialog |
| Popover | Flyout / Popup |

### 4. `pinch` Command Is macOS-Only

**Decision:** The PRD's `pinch <scale>` command (Section 5.5) is described as "Pinch zoom gesture (macOS trackpad)". On Windows, this returns `ACTION_NOT_SUPPORTED` with a clear message. Windows does not have trackpad gesture synthesis via accessibility APIs.

### 5. Implementation Priority: Observation First

**Decision:** Build observation commands first, then interactions.

**Priority order:**
1. **Observation tier:** `snapshot`, `list-windows`, `list-apps`, `find`, `get`, `is`, `list-surfaces`, `screenshot`
2. **Core interaction tier:** `click`, `type`, `set-value`, `focus`, `select`, `toggle`
3. **Extended interaction tier:** `double-click`, `triple-click`, `right-click`, `expand`, `collapse`, `check`, `uncheck`, `clear`
4. **Input tier:** `press`, `key-down`, `key-up`, `hover`, `drag`, `mouse-move`, `mouse-click`, `mouse-down`, `mouse-up`, `scroll`, `scroll-to`
5. **System tier:** `launch`, `close-app`, `focus-window`, `resize-window`, `move-window`, `minimize`, `maximize`, `restore`, `clipboard-get`, `clipboard-set`, `clipboard-clear`, `wait`
6. **Meta tier:** `status`, `permissions`, `version`, `batch` (most of these already work via core)

### 6. Development Workflow: Cross-Compile + Windows CI

**Decision:** Develop on macOS, cross-compile to verify builds, validate with Windows CI runner.

**Workflow:**
- Write code on macOS
- Cross-compile with `cargo check --target x86_64-pc-windows-gnu` (GNU target works without MSVC toolchain)
- Windows CI runner (`windows-latest`) executes actual UIA integration tests
- No Windows dev machine required for day-to-day work

### 7. Zero Core/macOS Changes

**Non-negotiable constraint:** Phase 2 touches only:
- `crates/windows/` — all new code
- `.github/workflows/ci.yml` — add Windows runner
- `docs/` — documentation updates

Nothing in `crates/core/`, `crates/macos/`, `crates/linux/`, or `src/` is modified. The one exception is if `BLOCKED_COMBOS` needs to move from core to platform adapters — this is a refactor that benefits all platforms and must be done carefully.

### 8. COM Threading / Send+Sync Strategy

**Decision:** Stateless `WindowsAdapter`, per-call COM initialization.

Both `UIElement` and `IUIAutomationElement` are `!Send, !Sync` — this is fundamental to COM, not a crate limitation. The strategy:
- `WindowsAdapter` stores zero COM state (no `UIAutomation` or `UIElement` fields)
- Each `PlatformAdapter` method creates a `UIAutomation` instance on entry
- All `UIElement` handles are consumed within the method call and dropped before returning
- `NativeHandle` continues to use `unsafe impl Send + Sync` with the raw pointer approach
- For `resolve_element`, store `(pid, control_type, name, bounds_hash)` in `RefEntry` and re-find via `UIMatcher` on each resolve

This is identical to how the macOS adapter works — `AXUIElementRef` is also `!Send`.

## Windows UIA Architecture Mapping

### PlatformAdapter → Windows Implementation

| Trait Method | Crate | Implementation |
|---|---|---|
| `list_windows` | `uiautomation` + `windows` | `EnumWindows` + `UIAutomation` root element for accessible properties |
| `list_apps` | `windows` | `EnumProcesses` + `OpenProcess` + `QueryFullProcessImageName` |
| `get_tree` | `uiautomation` | `UITreeWalker` with `UICacheRequest` for batch attribute retrieval |
| `execute_action` | `uiautomation` | `UIInvokePattern::invoke()`, `UIValuePattern::set_value()`, etc. |
| `resolve_element` | `uiautomation` | `UIMatcher` re-find by `(pid, control_type, name, bounds_hash)` |
| `check_permissions` | `windows` | COM security / UAC check (UIA doesn't require special perms for most apps) |
| `focus_window` | `windows` | `SetForegroundWindow` + `uiautomation` `UIElement::set_focus()` |
| `launch_app` | `windows` | `CreateProcessW` / `ShellExecuteW` |
| `close_app` | `windows` | `WM_CLOSE` / `TerminateProcess` (force) |
| `screenshot` | `windows` | `BitBlt` / `PrintWindow` via GDI |
| `get_clipboard` | `windows` | `OpenClipboard` + `GetClipboardData(CF_UNICODETEXT)` |
| `set_clipboard` | `windows` | `OpenClipboard` + `SetClipboardData(CF_UNICODETEXT)` |
| `focused_window` | `windows` | `GetForegroundWindow` → `UIAutomation::element_from_handle()` |
| `press_key_for_app` | `windows` | `SendInput` with `KEYBDINPUT` |
| `mouse_event` | `windows` | `SendInput` with `MOUSEINPUT` |
| `drag` | `windows` | `SendInput` sequence: mouse down → move → mouse up |
| `window_op` | `windows` | `ShowWindow` (minimize/maximize/restore), `SetWindowPos` (move/resize) |

### Windows Crate Folder Structure

```
crates/windows/src/
├── lib.rs              # mod declarations + re-exports only
├── adapter.rs          # PlatformAdapter trait impl
├── tree/
│   ├── mod.rs          # re-exports
│   ├── element.rs      # UIElement wrapper / attribute readers
│   ├── builder.rs      # build_subtree via UITreeWalker + UICacheRequest
│   ├── roles.rs        # ControlType → unified role mapping
│   ├── resolve.rs      # Element re-identification via UIMatcher
│   └── surfaces.rs     # Surface detection (menus, dialogs, popups)
├── actions/
│   ├── mod.rs          # re-exports
│   ├── dispatch.rs     # Action → UIA Pattern dispatch
│   ├── activate.rs     # Window/element activation
│   └── extras.rs       # SelectionPattern, ScrollPattern, TogglePattern
├── input/
│   ├── mod.rs          # re-exports
│   ├── keyboard.rs     # SendInput keyboard synthesis + Cmd→Ctrl mapping
│   ├── mouse.rs        # SendInput mouse synthesis
│   └── clipboard.rs    # Win32 clipboard API
└── system/
    ├── mod.rs          # re-exports
    ├── app_ops.rs      # CreateProcessW, TerminateProcess
    ├── window_ops.rs   # ShowWindow, SetWindowPos, EnumWindows
    ├── key_dispatch.rs # App-targeted key press via SendInput
    ├── permissions.rs  # COM security / UAC check
    ├── screenshot.rs   # BitBlt / PrintWindow screen capture
    └── wait.rs         # WaitForInputIdle, polling
```

## Pre-Work Required

Before implementing the adapter logic:

1. **Move BLOCKED_COMBOS to platform adapters** — refactor the current macOS-specific blocked list out of core
2. **Add Windows CI job** — `windows-latest` runner in CI with appropriate binary size check
3. **Create folder skeleton** — all `mod.rs` files and empty stubs in the structure above
4. **Add `uiautomation` + `windows` dependencies** — target-gated in `crates/windows/Cargo.toml`
5. **Generalize `AdapterError::permission_denied()`** — remove hardcoded macOS suggestion from core

## Resolved Questions

1. **UIA crate choice:** Use `uiautomation` (v0.24+) for UIA operations + `windows` crate for Win32 APIs. The PRD specified this, and research confirms it saves ~30% LOC for UIA operations while providing an escape hatch to raw COM.

2. **COM initialization:** Per-call via `UIAutomation::new()` which auto-calls `CoInitializeEx`. Stateless `WindowsAdapter` with zero stored COM state — same pattern as macOS adapter.

3. **NativeHandle on Windows:** Store raw pointer from `IUIAutomationElement`. Use `unsafe impl Send + Sync` with documented safety comment, same as macOS. For resolve, re-find via `UIMatcher` using `(pid, control_type, name, bounds_hash)` from `RefEntry`.

4. **Cross-compilation toolchain:** Use `x86_64-pc-windows-gnu` target for macOS cross-compilation (works without MSVC). Actual UIA integration tests run on Windows CI only.

5. **Chromium detection:** Surface in `snapshot` output. When snapshot detects a Chromium-based app with an empty/minimal tree, add a warning in the JSON response suggesting `--force-renderer-accessibility`. Agents see the guidance in context when it matters.

## Open Questions

None — all questions resolved.
