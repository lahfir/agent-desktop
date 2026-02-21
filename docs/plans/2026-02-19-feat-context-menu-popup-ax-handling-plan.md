---
title: "feat: Context Menu, Popup, and Overlay Handling via Accessibility APIs"
type: feat
date: 2026-02-19
---

# feat: Context Menu, Popup, and Overlay Handling via Accessibility APIs

## Overview

Context menus, sheets, dialogs, and popovers are transient surfaces that appear and disappear outside the normal window hierarchy. The current `snapshot` command misses them entirely because `list_windows` filters `kCGWindowLayer != 0` and `window_element_for` only queries `kAXWindowsAttribute`. This plan introduces targeted surface commands that read exactly the live transient surface — no full-tree traversal required.

## Problem Statement

1. **Context menus are invisible to snapshot.** Right-clicking in any app opens a menu at CG layer 8. `list_windows` rejects all `layer != 0` entries. `AXWindowsAttribute` does not include menus. An agent cannot read what is in the menu.

2. **Sheets and dialogs are not distinguished.** Save dialogs, alert panels, and popovers appear as children of windows but are not surfaced as distinct snapshot targets. An agent cannot tell if a modal is blocking interaction.

3. **No way to wait for a menu or popup.** `wait --element` polls the ref tree. There is no primitive to block until a context menu opens or a sheet appears.

4. **Comment noise.** Inline comments throughout the macOS crate explain implementation details that should be self-evident from names.

## Key Research Findings

### How macOS exposes transient surfaces via AX

| Surface | AX API | Notes |
|---------|--------|-------|
| Context menu (open) | `AXUIElementCopyAttributeValue(app, "AXMenus", …)` | Returns array of open `AXMenu` elements. Empty when closed. O(1) — no tree scan. |
| Context menu (notification) | `kAXMenuOpenedNotification` on the app element | Delivers the menu AXElement directly. |
| Focused window's sheet/dialog | `kAXFocusedWindowAttribute` on app element | Returns whatever window (or sheet) currently has focus. |
| Sheet attached to a window | Child with `AXSubrole == "AXSheet"` | Always a child of the parent window. |
| Alert/dialog | `kAXSubrole == "AXDialog"` or `kAXModalAttribute == true` | System alerts use subrole; app dialogs may use modal attribute. |
| Popover | `kAXSubrole == "AXPopover"` | Floats above the window hierarchy. |

### CG window layers (for reference, not for menu detection)

| Layer | Surface |
|-------|---------|
| 0 | Normal windows |
| 3 | Panels / popovers |
| 8 | Context menus |
| 20 | Alerts |

Using CG layers to find menus requires cross-referencing PID and is slower than reading `"AXMenus"` directly.

### `AXUIElementCreateSystemWide()`

Returns the system-wide accessibility element. `kAXFocusedApplicationAttribute` on it yields the currently focused app element. Useful when the caller has not already identified the target PID.

## Proposed Solution

### Phase 1 — `snapshot --surface` flag (new surface targeting)

Extend `SnapshotArgs` with `--surface <surface>` (default: `window`).

| `--surface` value | What is captured | Implementation |
|-------------------|-----------------|----------------|
| `window` | Current focused window (existing behavior) | unchanged |
| `focused` | Whatever `kAXFocusedWindowAttribute` returns (window, sheet, or dialog) | single AX attribute read, then build_subtree on result |
| `menu` | Open context menu on the target app | read `"AXMenus"` attribute on app element; error if no open menu |
| `sheet` | Sheet attached to focused window | walk `kAXChildrenAttribute` of focused window, find first `AXSubrole == "AXSheet"` |
| `popover` | Floating popover | walk children for `AXSubrole == "AXPopover"` |
| `alert` | Modal alert/dialog | walk children for `kAXModalAttribute == true` or `AXSubrole == "AXDialog"` |

All surfaces build their subtree with the existing `build_subtree` function. No new traversal logic needed.

### Phase 2 — `wait --menu` and `wait --popup`

Extend the `wait` command with surface-aware variants that use `AXObserver` notifications rather than polling:

| Flag | Notification | Timeout behavior |
|------|-------------|-----------------|
| `wait --menu` | `kAXMenuOpenedNotification` on app element | error `TIMEOUT` if menu does not appear within `--timeout` ms |
| `wait --menu-closed` | `kAXMenuClosedNotification` | waits until menu dismisses |
| `wait --popup` | poll `"AXMenus"` or watch for `AXSubrole==AXPopover` child | polling fallback (250ms interval) |

`AXObserver` requires a `CFRunLoop`. The implementation spins a dedicated thread, runs the loop for the duration of the timeout, then shuts it down. This is safe for single-shot CLI use — no persistent daemon needed in Phase 1.

### Phase 3 — `list-surfaces` command

Enumerate all currently visible transient surfaces for a given app:

```json
{
  "surfaces": [
    { "type": "menu", "title": "Edit", "item_count": 12 },
    { "type": "sheet", "title": "Save", "ref": "@e1" }
  ]
}
```

Implemented by reading `"AXMenus"`, inspecting `kAXFocusedWindowAttribute` children for sheets/popovers/dialogs, and returning a flat list. No tree traversal.

### Phase 4 — Comment cleanup

Remove all inline `//` comments from macOS crate files. Retain `///` doc-comments on public items where the function name alone is insufficient.

Files to clean: `tree.rs`, `adapter.rs`, `actions.rs`, `screenshot.rs`, `app_ops.rs`, `input.rs`, `roles.rs`, `permissions.rs`.

## Implementation Plan

### Step 1: Add `--surface` to `SnapshotArgs`

**File:** `src/cli.rs`

```rust
#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum Surface {
    #[default]
    Window,
    Focused,
    Menu,
    Sheet,
    Popover,
    Alert,
}

// In SnapshotArgs:
#[arg(long, value_enum, default_value_t = Surface::Window)]
pub surface: Surface,
```

**File:** `crates/core/src/adapter.rs` — extend `ScreenshotTarget` analog with a `SnapshotSurface` enum mirroring the CLI enum.

### Step 2: Surface resolution in macOS adapter

**File:** `crates/macos/src/tree.rs` — add `menu_element_for_pid`, `focused_surface_for_pid`, `sheet_for_window`:

```rust
/// Returns the first open context menu element for the given PID, if any.
pub fn menu_element_for_pid(pid: i32) -> Option<AXElement> { … }

/// Returns whatever kAXFocusedWindowAttribute points to (window, sheet, or dialog).
pub fn focused_surface_for_pid(pid: i32) -> Option<AXElement> { … }

/// Returns the first AXSheet child of the given window element.
pub fn sheet_for_window(win: &AXElement) -> Option<AXElement> { … }
```

These are all single attribute reads — O(1) AX calls before any tree traversal begins.

**File:** `crates/macos/src/adapter.rs` — update `get_tree` to dispatch on `SnapshotSurface`:

```rust
fn get_tree(&self, win: &WindowInfo, opts: &TreeOptions) -> Result<AccessibilityNode, AdapterError> {
    let el = match opts.surface {
        SnapshotSurface::Window => crate::tree::window_element_for(win.pid, &win.title),
        SnapshotSurface::Focused => crate::tree::focused_surface_for_pid(win.pid)
            .ok_or_else(|| AdapterError::internal("no focused surface"))?,
        SnapshotSurface::Menu => crate::tree::menu_element_for_pid(win.pid)
            .ok_or_else(|| AdapterError::not_found("no open context menu"))?,
        SnapshotSurface::Sheet => { … }
        SnapshotSurface::Popover => { … }
        SnapshotSurface::Alert => { … }
    };
    let mut visited = FxHashSet::default();
    crate::tree::build_subtree(&el, 0, opts.max_depth, opts.include_bounds, &mut visited)
        .ok_or_else(|| AdapterError::internal("empty tree"))
}
```

### Step 3: `wait --menu` via AXObserver

**File:** `crates/macos/src/wait.rs` (new file, ≤ 150 LOC)

```rust
/// Blocks until a context menu opens on the given PID or the timeout elapses.
pub fn wait_for_menu(pid: i32, timeout_ms: u64) -> Result<(), AdapterError> { … }

/// Blocks until the context menu on the given PID closes or the timeout elapses.
pub fn wait_for_menu_closed(pid: i32, timeout_ms: u64) -> Result<(), AdapterError> { … }
```

Implementation uses `AXObserverCreate`, `AXObserverAddNotification` with `kAXMenuOpenedNotification`/`kAXMenuClosedNotification`, then runs a `CFRunLoop` with a deadline timer on a dedicated thread. The main thread joins with the specified timeout.

**File:** `crates/macos/src/lib.rs` — `pub mod wait;`

### Step 4: Extend `WaitArgs` in CLI

**File:** `src/cli.rs`

```rust
/// Wait for a context menu to open (--menu) or close (--menu-closed)
#[arg(long)]
pub menu: bool,
#[arg(long)]
pub menu_closed: bool,
```

**File:** `crates/core/src/commands/wait.rs` — dispatch to `adapter.wait_for_surface(…)` with new `WaitSurface` enum.

### Step 5: `list-surfaces` command

**File:** `crates/core/src/commands/list_surfaces.rs`

```rust
pub struct ListSurfacesArgs {
    #[arg(long)]
    pub app: Option<String>,
}
```

Output:
```json
{
  "version": "1.0",
  "ok": true,
  "command": "list-surfaces",
  "data": {
    "surfaces": [
      { "type": "menu", "item_count": 8 },
      { "type": "sheet", "title": "Save" }
    ]
  }
}
```

### Step 6: Comment cleanup

Remove all inline `//` comments from `crates/macos/src/*.rs`. Rules:
- Delete comments that restate what the code does
- Delete comments that explain macOS constant values (constants are named)
- Keep `///` doc-comments on `pub` functions where the name alone is insufficient
- Keep `// SAFETY:` and `// kAX...` references where they provide non-obvious AX API context

## Acceptance Criteria

- [ ] `agent-desktop snapshot --app WhatsApp --surface menu` returns the open context menu tree within 0.5s (no full tree traversal)
- [ ] `agent-desktop snapshot --app Finder --surface sheet` returns the save sheet when one is open
- [ ] `agent-desktop snapshot --app Finder --surface focused` returns a focused dialog, sheet, or window — whichever has focus
- [ ] `agent-desktop wait --app TextEdit --menu --timeout 5000` blocks until a context menu opens and exits 0; exits with `TIMEOUT` error if none appears in 5s
- [ ] `agent-desktop list-surfaces --app Finder` lists all currently visible transient surfaces
- [ ] All existing 30 commands continue to pass
- [ ] No inline `//` comments remain in `crates/macos/src/`
- [ ] Snapshot of a context menu does NOT traverse the full app tree first
- [ ] `--surface menu` returns `APP_NOT_FOUND`-equivalent error (`ELEMENT_NOT_FOUND`) when no menu is open

## File Checklist

| File | Change |
|------|--------|
| `src/cli.rs` | Add `--surface`, `--menu`, `--menu-closed` args |
| `crates/core/src/adapter.rs` | Add `SnapshotSurface` enum to `TreeOptions`; add `wait_for_surface` trait method |
| `crates/core/src/commands/snapshot.rs` | Pass `surface` from args to `TreeOptions` |
| `crates/core/src/commands/wait.rs` | Add `WaitSurface` dispatch |
| `crates/core/src/commands/list_surfaces.rs` | New file |
| `crates/core/src/commands/mod.rs` | Register `list_surfaces` |
| `crates/macos/src/tree.rs` | Add `menu_element_for_pid`, `focused_surface_for_pid`, `sheet_for_window` |
| `crates/macos/src/adapter.rs` | Dispatch on `SnapshotSurface` in `get_tree`; implement `wait_for_surface` |
| `crates/macos/src/wait.rs` | New file — AXObserver-based surface waiting |
| `crates/macos/src/lib.rs` | `pub mod wait` |
| `crates/macos/src/*.rs` | Remove inline comments |

## Non-Goals

- Does NOT use `CGWindowListCreateImage` for menu capture (screenshot of a menu is separate from its AX tree)
- Does NOT implement persistent AXObserver across invocations (Phase 4 daemon scope)
- Does NOT handle Electron/custom-rendered menus that bypass the AX API
- Does NOT add Windows or Linux surface detection (Phase 2)
