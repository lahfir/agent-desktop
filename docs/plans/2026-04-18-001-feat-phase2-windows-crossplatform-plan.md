---
title: Phase 2 — Windows Adapter + Cross-Platform Feature Parity
type: feat
status: active
date: 2026-04-18
origin: docs/brainstorms/2026-04-18-phase2-windows-crossplatform-brainstorm.md
deepened: 2026-04-18
---

# Phase 2 — Windows Adapter + Cross-Platform Feature Parity

## Overview

Phase 2 brings agent-desktop to Windows and closes every cross-platform feature-parity gap surfaced after v0.1.13. A single release ships 18 objectives (P2-O1 … P2-O18) across ~15 implementation units. The core CLI, JSON envelope, and ref system are preserved; what grows is the adapter surface (new trait methods), the type surface (new `Action`/`ErrorCode`/`PermissionReport` variants, stable-selector fields), the FFI surface (deterministic codegen, `ad_abi_version`), the Windows shell-surface command surface, and the platform count (macOS + Windows, both x86_64 and ARM64). Version target: **v0.2.0** (breaking ABI + JSON schema).

## Headless-First Invariant (CORE PRINCIPLE)

**Every command in agent-desktop — existing and Phase 2 additions — must work headlessly inside the current user's active desktop session.** "Headless" means: (a) agent-desktop's own process has no GUI, no Dock icon, no visible window, no menu bar; (b) the target app does NOT need to be foregrounded or focused to be observed/driven; (c) no user-visible focus changes are side-effects of automation; (d) no physical cursor movement unless the caller explicitly invokes a mouse command. It does **not** mean bypassing the OS desktop model: Session 0, Server Core, secure desktop, locked desktop, and other-user sessions are unsupported for accessibility/capture and must return structured `PlatformNotSupported`, `PermDenied`, or `WindowNotFound` errors.

This is the core agent-automation contract. Any Phase 2 design choice that violates it is rejected.

### Headless rules by category

| Operation | Headless path | Forbidden |
|---|---|---|
| **macOS observation** | AX API (`AXUIElementCopy*`) — works on any process with granted Accessibility TCC; no focus needed | Reading AX from non-main thread (Apple DTS: all AX calls main-thread only) |
| **macOS action** | `AXUIElementPerformAction` (press, raise, show-menu) — headless on any visible or minimized window | Mouse-cursor synthesis when AX action is available; `NSApp.activate` as an action side-effect |
| **macOS events (`watch_element`)** | `AXObserver` bound to the **main-thread `CFRunLoop`** (bootstrapped by the CLI) | Worker-thread CFRunLoop with observer (unsupported by Apple per Topic-A research) |
| **macOS file delivery** | `NSWorkspace.open(urls:withApplicationAt:configuration:)` with `activates: false`, or pasteboard + `CGEventPostToPid(cmd+v)` to the target PID | `NSDraggingSession` (requires NSApp event loop + focus steal — not headless) |
| **macOS screenshot** | ScreenCaptureKit `SCScreenshotManager.captureImage` — headless, but requires Screen Recording TCC | `/usr/sbin/screencapture` subprocess (legacy only, behind `--screenshot-backend legacy`) |
| **Windows observation** | `IUIAutomation.ElementFromHandle(hwnd)` — works on same-user, same-session visible/minimized windows at an accessible integrity level without foregrounding | Cross-session, secure desktop, locked desktop, Session 0, Server Core |
| **Windows action** | UIA pattern invocation (`InvokePattern.Invoke`, `ValuePattern.SetValue`, `TogglePattern.Toggle`, `ExpandCollapsePattern.Expand`, `SelectionItemPattern.Select`) — all focus-independent | `SendInput` as a primary path — it IS focus-dependent; only fallback when no UIA pattern applies, and gated by `AttachThreadInput + SetFocus` worker-thread dance |
| **Windows events (`watch_element`)** | UIA event handler on dedicated MTA apartment thread — UIA explicitly supports cross-thread event delivery per Microsoft's 2025 threading doc | Caching `IUIAutomationElement` across apartment boundaries (apartment-affine handles invalidate) |
| **Windows file delivery** | App/shell delivery first: app URI handlers, `ShellExecuteEx`, `IFileOperation` for filesystem destinations, and `CF_HDROP` clipboard paste where accepted | Cursor-synthesized drag as primary path; using `IDataObject + DoDragDrop` before a policy-gated spike proves target behavior |
| **Windows screenshot** | `windows-capture` (`Windows.Graphics.Capture` — active interactive DWM session, Windows 10 1903+) | Session 0 / Server Core / locked or secure desktop capture; `PrintWindow` is fallback-only |
| **Skeleton traversal** | Core `snapshot_ref.rs` + `adapter.get_subtree(handle, opts)` is platform-agnostic — skeleton works on any window without focus, on both platforms | — |
| **Clipboard** | `NSPasteboard.general` (macOS) / `OpenClipboard` (Windows, no HWND required when passing `NULL`) | — |

### Verification

Every Phase 2 integration test MUST assert headless-ness explicitly:
- Target window is **NOT** the focused window at test entry (send test driver to background first).
- agent-desktop CLI runs with stdin/stdout/stderr as pipes (no TTY) where possible.
- Before and after the command, `list-windows --focused-only` returns the SAME focused window — no focus steal.
- Cursor position is unchanged for commands that aren't `hover`/`drag`/`mouse-*`. Click uses semantic accessibility paths by default; coordinate clicking requires an explicit physical path.

### Skeleton traversal invariant

The `--skeleton` / `--root @ref` progressive traversal pattern (P2-O-skeleton, shipped in v0.1.11 for macOS) is preserved and extended to Windows in Unit 3. The contract:
- `snapshot --skeleton` clamps depth to 3 and annotates truncated containers with `children_count`.
- Named / described containers at the depth boundary receive refs as drill-down targets.
- `snapshot --root @ref` walks from a previous-snapshot ref with **scoped invalidation** (only that ref's subtree refs are replaced on re-drill).
- Refmap write-side 1 MB guard prevents runaway ref counts.
- Works on unfocused windows on both platforms (Windows `ElementFromHandle(hwnd)` + macOS `AXUIElementCreateApplication(pid)` are both focus-independent).

Windows implementation notes (research-driven):
- Use **`ControlViewWalker`** (NOT `RawViewWalker` or `ContentViewWalker`) — `IsControlElement` auto-filters layout noise and complements the Electron depth-skip in Unit 4.
- `children_count` via `FindAll(TreeScope_Children, TrueCondition)` — single COM round-trip, no per-child property fetch.
- Fresh `UICacheRequest` per drill-down call — cached elements do not survive CLI process boundaries.
- Expected token savings on VS Code / Slack Electron track macOS's 50-100× once U4's `--force-electron-a11y` + empty-`UIA_Group`/`UIA_Custom` depth-skip are in place.

## Problem Frame

Three orthogonal problems share one release (see origin §What Phase 2 is solving):

1. **Three-platform reach.** macOS is the only shipping platform. Phase 2 brings Windows online with an identical command surface and JSON contract, ahead of Linux in Phase 3.
2. **Identifier instability.** Today every ref resolves via `(pid, role, name, bounds_hash)`. Electron trees, localized apps, and custom-rendered controls fray this and inflate `STALE_REF` rates. Stable-selector fields (`identifier`, `subrole`, `role_description`, `placeholder`, `dom_id`, `dom_classes`) — free on macOS and native to UIA — collapse the churn.
3. **Polling-shaped waiting.** `wait --element` polls every 100 ms. `watch_element` replaces it with sub-500 ms push notifications on both platforms.

Five smaller gaps are cheap individually but collectively move agent-desktop from "macOS-only observation tool" to "cross-platform agent automation runtime": modern screenshot APIs (ScreenCaptureKit / windows-capture), text-range primitives, new `Action` variants, new surfaces (Toolbar / Spotlight / Dock / MenuBarExtras / Windows shell surfaces), tri-state permission probing, and an FFI registry migration that makes the Phase 4 MCP crate trivial to ship.

Nothing is deferred to Phase 3 that was in the Phase 2 brainstorm scope — the earlier 2a/2b split was rejected (see origin §D1).

## Requirements Trace

Each requirement maps 1:1 to a `phases.md` P2-O* objective (see origin §Acceptance criteria). GA ships when every requirement's metric is green.

- **R1 (P2-O1)** — Windows adapter: `snapshot --app Explorer` returns a valid tree with refs; same for Notepad, Settings, VS Code, Edge.
- **R2 (P2-O2 — review-refined)** — Cross-platform parity on a structurally-identical app (Calculator on both platforms): `role` set jaccard ≥ 0.85; `identifier` equality where non-empty on both sides; ref count within ±15%; `available_actions` set union is a superset of common-actions across the role map. "Structurally identical" replaced the earlier byte-identical aspiration, which UIA↔AX role-mapping cannot guarantee.
- **R3 (P2-O3)** — Windows input: `click @e5`, `type @e2 "hello"`, `press ctrl+c`, every mouse command succeed against a test app.
- **R4 (P2-O4)** — Windows screenshot: `screenshot --app Notepad` produces a valid PNG via `windows-capture`, SSIM-matches `PrintWindow` fallback.
- **R5 (P2-O5)** — Windows clipboard: get/set/clear roundtrip for ASCII and Unicode.
- **R6 (P2-O6)** — Windows CI: `windows-latest` runs build, clippy, unit, contract, and non-interactive tests on every PR. UIA/shell integration tests that require Explorer, Start, Action Center, Quick Settings, or an unlocked desktop run on a labeled interactive/self-hosted Windows job or assert structured unsupported behavior when the shell is absent.
- **R7 (P2-O7)** — Windows release: x86_64 + aarch64 `.exe` and FFI archives ship with the Phase 2 tag; `npm install` works on both.
- **R8 (P2-O8)** — Stable-selector fields populated on both platforms; measurable `STALE_REF` rate drop vs Phase 1 baseline.
- **R9 (P2-O9 — research-refined)** — Action variants (`LongPress`, `ForceClick`, `ShowMenu`, `DeliverFiles` (renamed from `FileDrop` because `NSDraggingSession` is not headless-compatible — see Unit 12), `WindowRaise`, `Cancel`, `SelectRange`, `InsertAtCaret`) exposed via CLI and each green in a platform-appropriate integration test. Semantic actions assert no focus steal. Explicit focus/window/physical actions assert the side effect is policy-authorized and documented.
- **R10 (P2-O10)** — ErrorCode variants (`PermissionRevoked`, `ResourceExhausted`, `AxMessagingTimeout`, `AutomationPermissionDenied`) each have a runtime producer.
- **R11 (P2-O11)** — `watch --event value-changed --ref @e5 --timeout 3000` receives an event within 500 ms of a programmatic value change on both platforms.
- **R12 (P2-O12)** — `text select-range` + `text get-selection` roundtrip; `text insert-at-caret` advances caret correctly on both platforms.
- **R13 (P2-O13)** — Modern screenshot cold-latency <50 ms on both platforms vs ~300 ms macOS subprocess baseline; default is modern, legacy behind `--screenshot-backend legacy`.
- **R14 (P2-O14)** — `snapshot --surface toolbar` on Safari (macOS) and Edge (Windows) works; macOS lists Spotlight / Dock / MenuBarExtras; Windows lists present shell surfaces (`Taskbar`, `SystemTray`, `SystemTrayOverflow`, `StartMenu`, `ActionCenter`, `QuickSettings`); tray commands function.
- **R15 (P2-O15)** — Electron compat on Windows: VS Code snapshot with `--force-electron-a11y` exposes ≥100 refs at default depth.
- **R16 (P2-O16)** — FFI registry: adding a command requires only a new file under `crates/core/src/commands/`; CLI, FFI wrappers, and (future) MCP tools auto-register; `ad_abi_version()` exported; `ad_set_log_callback` receives tracing output during `ad_get_tree`.
- **R17 (P2-O17)** — Permission tri-state: `permissions` output shows AX, Screen Recording, Automation independently on macOS. Denied Screen Recording returns `PermDenied` with Screen-Recording-specific suggestion; denied Automation for a target app returns `AutomationPermissionDenied`.
- **R18 (P2-O18)** — Windows shell coverage: Start menu/search, taskbar, system tray/overflow, Action Center/notification center, Quick Settings, multi-monitor/DPI, virtual desktop detection, UAC/elevated targets, RDP/locked-session behavior, and Explorer-specific file destinations are explicitly covered by commands, surfaces, tests, or documented `PLATFORM_NOT_SUPPORTED` behavior. Windows-only commands still live in core command files with adapter defaults.

## Scope Boundaries

- **Included**: every P2-O* objective in `docs/phases.md §Phase 2`; v0.2.0 breaking ABI + JSON schema bump; MSRV bump to 1.82; ARM64 Windows (build-only until GH runner arrives); tri-state permission model on macOS; registry-driven FFI codegen; `ad_abi_version()` export.

### Deferred to Separate Tasks

- **Linux adapter** — Phase 3 (separate plan). Trait methods ship with default `not_supported()` implementations in U1 so Linux mirrors later without re-opening core.
- **MCP server mode** — Phase 4.
- **Daemon, sessions, audit log, policy engine, OCR fallback** — Phase 5.
- **Streamable HTTP transport** — Phase 4 (stdio confirmed sufficient for MS Agent Framework 1.0).
- **Package-manager distribution (brew/winget/snap)** — Phase 5.
- **Async FFI** — Phase 4 once MCP streaming arrives (see origin §D9).
- **`ForceClick` on Linux** — permanent per-platform divergence (returns `ActionNotSupported`), see origin §D8, R9.

## Context & Research

### Relevant Code and Patterns

- `crates/core/src/node.rs` — `AccessibilityNode` (10 fields today, L3–L33). Unit 1 nests new selectors via `#[serde(flatten)]`.
- `crates/core/src/error.rs` — `ErrorCode` enum (12 variants, L4–L19; not yet `#[non_exhaustive]`). Paired with `AdResult` variant-count `const _: () = assert!(…)` in `crates/ffi/src/error.rs:57`.
- `crates/core/src/action.rs` — `Action` enum (already `#[non_exhaustive]`, 21 variants). Unit 1 adds 8 new variants.
- `crates/core/src/adapter.rs` — `PlatformAdapter` trait (~28 methods with `not_supported()` defaults, L117–L278). New methods land as additive defaults.
- `crates/core/src/refs.rs` — `RefEntry` (9 fields, L13–L28). Unit 1 adds `identifier: Option<String>` for selector-preferred resolution.
- `crates/core/src/commands/` — 54 files (53 commands + `helpers.rs`), one per command; `click.rs` (14 L), `list_windows.rs` (18 L), `type_text.rs` (24 L), `snapshot.rs` (226 L), `wait.rs` (229 L) are the representative patterns for Unit 6 / Unit 7 / Unit 8 new commands. The command count is used throughout this plan (document-review note).
- `crates/macos/src/tree/element.rs` (404 L) — where Unit 5 stable-selector reads land. File is at the 400-LOC cap; plan mandates a 2-way split into `element.rs` (core) + `element_selectors.rs` (new selector readers) before adding reads.
- `crates/macos/src/tree/resolve.rs:20` — `resolve_depth = 50` already matches the `ABSOLUTE_MAX_DEPTH` target Unit 4 mirrors on Windows.
- `crates/ffi/build.rs` — existing cbindgen invocation + header-path stamp pattern; Unit 2 extends this, it does not replace it.
- `crates/ffi/src/error.rs:5-60` — the parity `const` assertion pair; Unit 1 updates both arrays atomically with new `ErrorCode` variants.
- `crates/ffi/include/agent_desktop.h` — committed ABI contract; drift check at `.github/workflows/ci.yml` step "FFI header drift check".
- `.github/workflows/release.yml` — FFI matrix already includes `x86_64-pc-windows-msvc`; Unit 13 adds the `aarch64-pc-windows-msvc` FFI row, the Windows CLI binary row, and npm postinstall branches.
- `.github/workflows/ci.yml` — `test` job runs only on `macos-latest` today; Unit 13 adds a sibling `test-windows` job.
- `.githooks/pre-commit` — runs `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib --workspace` when Rust/TOML files are staged; awareness only, no change required.
- `skills/agent-desktop/SKILL.md` (227 L) and `skills/agent-desktop-ffi/SKILL.md` (78 L) exist; `skills/agent-desktop-macos/` **does not** exist — macOS content lives at `skills/agent-desktop/references/macos.md`. Unit 14 creates `skills/agent-desktop-windows/` as a sibling and updates the core skill to three-platform.

### Institutional Learnings

- `docs/solutions/best-practices/deduplicate-ref-allocator-via-config-struct-2026-04-14.md` — **DRY via config structs, threshold "4+ positional params + 1 new distinguishing value."** Extraction bar: shared parameter types at trait boundaries and shared policy (not shared implementations). Phase 2 extracts **4** shared types in `crates/core`:
  - `ActionDispatchConfig` — parameter type for action execution. Carries policy only: `fallback_to_cursor: bool`, `timeout_ms`, `blocked_combos: &'static [KeyCombo]`. **Phantom DRY warning:** the dispatch chain itself (AX action strings on macOS vs UIA pattern interfaces on Windows) is categorically different types — do NOT extract a shared dispatch chain. Share only the policy.
  - `WatchElementConfig` — `watch_element` trait method parameter. Carries `events: &[EventKind]`, `timeout_ms`, `max_subscriptions`, `hard_join_multiplier` (default 2.0).
  - `TextRangeConfig` — text primitive trait method parameter. Carries `utf16_semantics: true` marker + `check_password_field: bool` (default true; see Unit 8 security gate) + per-platform text-pattern entrypoint hints.
  - `ScreenshotBackendConfig` — `get_screenshot_with_backend` parameter. Carries `backend: ScreenshotBackend`, `dimensions: Option<(u32, u32)>`, `pixel_format`, `encoding`.
  **Rejected (reviewed and dropped — YAGNI and duplication risk):**
  - `TreeWalkConfig` — duplicates the existing `TreeOptions` struct at `crates/core/src/adapter.rs:27`. Extend `TreeOptions` with `force_electron_a11y` instead of creating a parallel struct.
  - `SurfaceDetectionConfig` — per-platform surface policy (window-class lists, AX-role overrides) should not leak platform specifics into core types. Keep platform-local in `crates/macos/src/tree/surfaces.rs` and `crates/windows/src/tree/surfaces.rs`.
  - `SelectorReadConfig` — the 6 selector attribute names are categorically different types across platforms (macOS AX string constants vs Windows UIA property IDs). No shared shape exists to extract.
  - `EventWorker` trait — the "spawn → attach → mpsc → timeout → join" shell is ~15 LOC and the teardown primitives (`CFRunLoopStop` vs `PostThreadMessage(WM_QUIT)`) are categorically different. Inline in each platform's `watch_element` implementation. Revisit at Phase 3 when a third platform proves the shape.
  - `NotificationSession` trait — macOS `nc_session.rs` and Windows `action_center.rs` have superficially-similar "open/dismiss/close" lifecycles but radically different state (NSUserNotificationCenter observer vs UIA tree walk). No code is generic over both. Document shape alignment in code comments; revisit at Phase 3.
- `docs/solutions/logic-errors/progressive-snapshot-review-contract-2026-04-16.md` — **Separate `INVALID_ARGS` (bad selector syntax), `STALE_REF` (valid syntax, gone element), and `TIMEOUT`.** Apply to `watch_element` in Unit 7 and text-range commands in Unit 8. Boundary-node pattern at subscription caps.
- `docs/solutions/best-practices/deterministic-build-artifact-marker-2026-04-16.md` — **`build.rs` stamps absolute paths for generated artifacts; committed copy is the ABI contract; never self-heal.** Apply to Unit 2's generated `ad_*` wrappers and JSON schemas, and to Unit 1's `ad_abi_version` constant. CI drift-checks each.
- `docs/solutions/best-practices/identity-fingerprint-against-os-reorder-2026-04-16.md` — **Stable identifiers are optional fingerprints carried alongside index/handle; tri-state UTF-8 decode at FFI boundary.** Directly applies to Unit 5 (`identifier` field) and Unit 10 (tray/notification indices on Windows).
- **Private-memory rule: port `electron-compat.md` and `macos-ax-gotchas.md` from `~/.claude/.../memory/` into `docs/solutions/` before Unit 4 opens.** Windows contributors must inherit the Electron depth-skip rules and TCC traps without relying on private auto-memory. Tracked in Unit 14's doc cleanup.

### External References

- UIA + `uiautomation 0.24` crate docs (framework-docs-researcher cache): `UITreeWalker`, `UICacheRequest`, pattern interface set (`InvokePattern`, `ValuePattern`, `ExpandCollapsePattern`, `SelectionItemPattern`, `TogglePattern`, `ScrollPattern`, `TextPattern`, `TextRange`).
- `windows 0.62.2` crate: `Windows.Graphics.Capture`, `Direct3D11CaptureFramePool`, `SendInput`, `OpenClipboard`, `IDataObject`.
- `windows-capture 1.5.4`: pinned exactly because newer majors may move the capture API surface — re-evaluate post-Phase 2. `Capture::start` + frame-callback API validated against 1.5.4.
- ScreenCaptureKit via `screencapturekit 1.5` (doom-fish fork) — `SCShareableContent.windows`, `SCScreenshotManager.captureImage(contentFilter:config:)`.
- `objc2 0.6` replaces ad-hoc `objc` message sends, scoped to `system/screenshot.rs` + `system/permissions.rs`.
- **`inventory` and `linkme` both rejected** (research Topic B). Replaced with `build.rs` filesystem enumeration of `crates/core/src/commands/*.rs` — deterministic, cdylib-safe across ld64, ld-prime, GNU ld, lld, MSVC link.exe, zero linker magic.
- `schemars 1.2` — required by P2-O16 for command-arg JSON Schema generation.
- MS Agent Framework 1.0 MCP transport reference — stdio is sufficient for Phase 4; no streamable HTTP required.

## Key Technical Decisions

Carried forward from origin §Decisions (D1–D17). Each is pinned here with its unit anchor so planning enforces them.

- **KD1 (D1)** Single release, no 2a/2b split. Scope managed via units, not sub-phases. *Enforced by:* every unit has a Phase 2 GA gate.
- **KD2 (D2)** ~15 implementation units, one PR each; dependencies flow left-to-right in the graph (§High-Level Technical Design).
- **KD3 (D3)** `WindowInfo.pid` stays `i32`; Windows adapter narrow-casts `u32 → i32` at the adapter boundary via a helper `fn narrow_pid(dword: u32) -> Result<i32, AdapterError>` that returns `ErrorCode::ResourceExhausted` (reusing the new variant added in Unit 1) with `platform_detail: "PID exceeds i32::MAX; Windows kernel should never produce this value"` for values above `i32::MAX`. `Internal` is wrong here because this is a boundary failure, not a bug in agent-desktop; `ResourceExhausted` costs zero additional ABI surface and is semantically correct (PID space exhausted relative to `i32` representation). Documented in Unit 1; implemented in Unit 3.
- **KD4 (D4)** `ErrorCode` gains `#[non_exhaustive]` as the **first** sub-PR in Unit 1, before any variant addition. The parity `const _: () = assert!(…)` in `crates/ffi/src/error.rs` is updated atomically with each new variant — the assertion is the compile-time gate.
- **KD5 (D5)** Registry migration (Unit 2) ships before the Windows adapter so every new Phase 2 command is born in the registry.
- **KD6 (D6)** Modern screenshot is the **default** on both platforms. `--screenshot-backend legacy` reaches the Phase 1 subprocess (macOS) / `PrintWindow` (Windows) path for WDA-protected windows and restricted environments.
- **KD7 (D7)** All 6 stable-selector fields ship, nested as a `StableSelectors` sub-struct on `AccessibilityNode` with `#[serde(flatten)]` + per-field `skip_serializing_if`. JSON wire shape preserved. Rust field count on `AccessibilityNode` becomes 11; grandfathered (KD15).
- **KD8 (D8)** All 9 new `Action` variants ship. `Watch(WatchSpec)` is **not** an `Action` — `watch_element` is an adapter method (origin §D9). `ForceClick` returns `ActionNotSupported` on Linux (Phase 3) — legitimate platform divergence, not a deferral.
- **KD9 (D9 + review-refined)** `watch_element` is synchronous at the public API. Per-call worker thread (macOS: `CFRunLoop` + `AXObserver`; Windows: MTA thread + UIA event handler) → `std::sync::mpsc` → caller. Trait method takes `&RefEntry` (not `&NativeHandle`) — worker re-resolves the element fresh on its own thread, preserving the `NativeHandle` `!Send`/`!Sync` invariant. **The orchestration shell (~15 LOC: spawn → attach → mpsc → timeout → join) is inlined in each platform's `watch_element` implementation** — no shared `EventWorker` trait. Rationale: with N=2 platforms and categorically different teardown primitives (`CFRunLoopStop` vs `PostThreadMessage(WM_QUIT)`), a trait with one abstract method per side is a framework, not DRY. Revisit at Phase 3 when AT-SPI proves the three-way shape. **AX threading invariant:** `AXObserverCreate` and callback dispatch on the observer's CFRunLoop thread is Apple-blessed for non-main threads; `AXUIElementCopy*` calls from the worker are validated by Unit 7's opening spike against macOS 14/15. If the spike shows thread-hostility on specific calls, the worker hops to main via a `with_main_thread(|| …)` helper that uses `dispatch_sync` to the main queue. Same pattern on Windows: COM apartment hostility escapes via `SendMessage` to a main-thread windowless receiver. Hard-join timeout = `2 × user_timeout_ms`; refuse-to-exit worker returns `ErrorCode::Internal`. Async surfaces are Phase 4's problem. Thread pool not shared with Unit 9.
- **KD10 (D10)** `PermissionReport` migrates to a struct with `accessibility`, `screen_recording`, `automation` tri-state fields. Breaking JSON schema on the `permissions` command and breaking FFI ABI. Lands atomically with `ad_abi_version()` bump in Unit 1.
- **KD11 (D11)** `BLOCKED_COMBOS` becomes a `PlatformAdapter` trait method `fn blocked_combos(&self) -> &'static [KeyCombo]`. macOS blocks `cmd+q`, `cmd+shift+q`. Windows blocks `alt+f4`, `ctrl+alt+delete`, `win+l`. `key-down` / `key-up` safety check rejects any combo that matches `blocked_combos()` as a whole — **no persistent modifier-state file** (keeps the tool stateless; rejecting solo modifiers is never needed because blocked combos always require at least one non-modifier key).
- **KD12 (D12)** Electron/WebView2 compat on Windows mirrors macOS: depth-skip for non-semantic wrappers, resolver depth 50, surface detection that treats the focused window AS the target surface, `--force-electron-a11y` CLI override.
- **KD13 (D13)** ARM64 Windows ships in Phase 2 alongside x86_64. `aarch64-pc-windows-msvc` is build-only until a GH runner arrives (test matrix adds it post-hoc in a follow-up PR). `npm/scripts/postinstall.js` gains `win32-x64` and `win32-arm64` branches.
- **KD14 (D14)** CI matrix = macOS + Windows for full tests; Ubuntu for fmt. `cargo tree -p agent-desktop-core` isolation check runs on both.
- **KD15 (D15 + research-refined)** Dependencies:
  - Windows: `uiautomation 0.24`, `windows 0.62.2` (matches `windows-capture 1.5`'s own pin), `windows-capture = "1.5.4"` (latest stable, published crates.io)
  - macOS: `objc2 0.6`, `screencapturekit = "1.5"` (**published crates.io** — the doom-fish fork is the canonical maintained crate as of Q1 2026; NOT a git-SHA pin)
  - Cross-platform: `schemars 1.2` (deferred to Phase 4 — see KD16 below)
  - **NO `inventory` / `linkme`** (research Topic B — link-GC risk across ld64/ld-prime/GNU ld/lld/MSVC is real; both crates' ctor-based / linker-section patterns are unreliable for cdylib consumers)
  - Command registry uses `build.rs` filesystem enumeration of `crates/core/src/commands/*.rs` — deterministic, cdylib-safe, zero linker magic (research Topic B recommendation)
  - MSRV: `1.82` (required by `windows 0.62.2`). `AccessibilityNode` 10-field grandfather preserved — only new selectors nest into `StableSelectors`.
- **KD16 (D16)** Cross-compile-first workflow holds: macOS dev → `cargo check --target x86_64-pc-windows-msvc` → Windows CI integration. Pre-commit hook runs the cross-check best-effort (warn, never fail).
- **KD17 (D17 + review-refined)** Pre-1.0 FFI policy, published at `crates/ffi/README.md` during the v0.1.14 prep release (see §Phased Delivery). Policy matrix:
  - **`#[non_exhaustive]` enum variant addition (additive)** → **no major bump**. C consumers MUST use `default:` / wildcard fallthrough; library documents this as a hard contract. Defense-in-depth: a reserved `AD_RESULT_UNKNOWN = -99` sentinel is exported so consumers can map any unrecognized integer to a known value explicitly. Rust never produces this sentinel; it exists purely for consumer dispatch tables. Guarded by the compile-time `const _: () = assert!(ErrorCode vs AdResult parity)` gate.
  - **FFI struct layout change (field add/remove/reorder)** → **major bump**.
  - **FFI function signature change (parameter or return type)** → **major bump**.
  - **New `extern "C" ad_*` function (additive)** → **no bump**; consumers must check symbol presence via `dlsym`.
  - **Removal of any exported symbol** → **major bump**.
  Consumer version handshake: `ad_init(expected_major: u32) -> AdResult` — the ONLY FFI function that must be called before any other `ad_*`. Fails closed with `AdResult::ErrInvalidArgs` if `expected_major != AD_ABI_VERSION_MAJOR`. Without this call, subsequent `ad_*` calls fail closed with `ErrInternal + "ad_init not called"`. Converts the "consumers SHOULD check" advisory into an enforced handshake. `ad_init` ships in v0.1.14 returning `MAJOR = 1`.
  **v0.1.14 prep release** ships `#[non_exhaustive]` + `ad_abi_version()` + `ad_init()` + `AD_RESULT_UNKNOWN` sentinel + `crates/ffi/README.md` policy doc, with **no variant additions, no struct-layout changes**. v0.2.0 ships `AD_ABI_VERSION_MAJOR = 2` atomically with the `PermissionReport` tri-state layout change in sub-PR 1g. Phase 3 Linux adapter adds variants additively — no bump.

## Open Questions

### Resolved During Planning

- **StableSelectors shape** (origin open question): Use an inline `StableSelectors` sub-struct with `#[serde(flatten)]` + per-field `#[serde(skip_serializing_if = "…")]`. No `Option<StableSelectors>` wrapper. Rationale: preserves exact JSON wire shape; each selector is individually skippable when empty; cbindgen emits cleaner output for a nested struct than 6 more optional top-level fields. Landed in Unit 1.
- **Modifier-state tracking for `key-down` / `key-up` safety check** (origin open question): Do **not** persist modifier state. The check evaluates the combo passed to `key-down` / `key-up` directly and rejects if it matches any entry in `blocked_combos()` as a whole combo. Blocked combos always include a non-modifier key (e.g., `cmd+q`, `alt+f4`), so solo modifier key-downs are never rejected. Keeps the tool stateless — matches the Phase 1 invariant. Landed in Unit 1 / Unit 3.
- **Thread-pool consolidation** (origin open question): Keep per-call worker threads in Units 7 and 9. Different lifetimes (watch_element: seconds-to-tens-of-seconds observer loop; screenshot: sub-second single-shot capture). Re-evaluate when Phase 4 daemon arrives.
- **`wait --event` CLI shape** (origin open question): `wait --event <kind> --ref @e5 --timeout 3000`. `--event` repeats for multi-subscription (`--event value-changed --event selection-changed`). `<kind>` accepts the 10 `EventKind` variants named in `docs/phases.md §Phase 2`: `focus-changed`, `value-changed`, `selection-changed`, `children-changed`, `window-opened`, `window-closed`, `menu-opened`, `menu-closed`, `notification-posted`, `element-destroyed`. Event filter expressions beyond kind + ref are deferred.
- **`AccessibilityNode` 10-field grandfather** (origin open question): Do not refactor existing flat fields. Only new `StableSelectors` is nested. Smaller blast radius; pre-existing JSON consumers unaffected.
- **`inventory` vs `linkme`** (origin §D15 ambiguity — **research-resolved**): **Neither.** Research Topic B found neither crate reliably survives link-GC across ld64, ld-prime, GNU ld, lld, and MSVC link.exe for cdylib consumers; `deterministic registry metadata` ctor sites are stripped when an `rlib` is linked into a binary that never references a symbol from that rlib. Instead, the command registry is built at compile time via `crates/core/build.rs` that enumerates `crates/core/src/commands/*.rs` (one-command-per-file is already a CLAUDE.md invariant) and codegens a `pub fn descriptors() -> &'static [CommandDescriptor]` static. Deterministic, cdylib-safe, zero linker magic. FFI wrapper codegen in `crates/ffi/` uses the same `build.rs` approach reading the same source listing. Simplification: no xtask crate needed.

### Deferred to Implementation

- **Exact `uiautomation` crate API surface** for `UICacheRequest` batching — requires a spike against real apps on Windows CI before Unit 3 freezes.
- **Exact `AXObserver` teardown sequence on `CFRunLoop` stop** — named as Unit 7's opening spike. Validates non-main-thread AX observer behavior against Finder, TextEdit, VS Code on macOS 14/15.
- **`windows-capture 1.5.4` API against `windows 0.62.2`** — verify `Capture::start` + frame-callback API compiles together before merging Unit 9. Future patch bumps require the same spike before changing the pin.
- **`SCShareableContent` windowing** — exact API shape for filtering to a `CGWindowID` via `SCContentFilter` on macOS 14 vs 15. Unit 9's macOS spike.
- **Event handler lifetime on Windows** — `IUIAutomation.AddAutomationEventHandler` must be removed before thread exit, confirmed by Unit 7 spike.
- **`DeliverFiles` per-app URL scheme registry for Tier 1** — Unit 12 builds a small `crates/macos/src/actions/deliver_files_registry.rs` mapping known bundle IDs to their CLI/URL scheme. Initial entries (VS Code, Finder, Preview, TextEdit, Safari, Chrome) are defined at implementation time; the registry is extensible per-release.
- **Exact `AEDeterminePermissionToAutomateTarget` bundle-id argument** for Unit 11 — depends on which target app the user is automating; solved at call site, not trait signature.

## Output Structure

New/rewritten directory layouts. Paths shown are repo-relative.

```
crates/
├── core/
│   └── src/
│       ├── node.rs                    # +StableSelectors sub-struct; flatten into AccessibilityNode
│       ├── error.rs                   # +4 ErrorCode variants; +#[non_exhaustive]
│       ├── action.rs                  # +8 Action variants
│       ├── adapter.rs                 # +blocked_combos, watch_element, text-range, get_screenshot_with_backend; PermissionReport → tri-state struct
│       ├── refs.rs                    # +identifier field on RefEntry
│       ├── event.rs                   # NEW — EventKind, ElementEvent, WatchSpec (supporting types for watch_element)
│       ├── text_range.rs              # NEW — TextRange, TextSelection (supporting types for text primitives)
│       ├── screenshot_backend.rs      # NEW — ScreenshotBackend enum (Modern / Legacy)
│       ├── permission.rs              # NEW — Tri-state PermissionReport struct, extracted from adapter.rs
│       ├── commands/
│       │   ├── watch.rs               # NEW — Unit 7 command
│       │   ├── text_get_selection.rs  # NEW — Unit 8 command
│       │   ├── text_select_range.rs   # NEW — Unit 8 command
│       │   ├── text_insert_at_caret.rs # NEW — Unit 8 command
│       │   ├── text_at_offset.rs      # NEW — Unit 8 command
│       │   ├── list_tray_items.rs     # NEW — Unit 3b / Unit 10 command
│       │   ├── click_tray_item.rs     # NEW — Unit 3b / Unit 10 command
│       │   └── open_tray_menu.rs      # NEW — Unit 3b / Unit 10 command
│       └── registry.rs                # NEW — Unit 2: CommandDescriptor type + include!(registry.rs from $OUT_DIR)
├── windows/
│   └── src/
│       ├── lib.rs                     # mod + re-exports (rewritten)
│       ├── adapter.rs                 # WindowsAdapter: PlatformAdapter impl (Unit 3)
│       ├── tree/                      # element, builder (UITreeWalker + UICacheRequest), roles, resolve, surfaces
│       ├── actions/                   # dispatch, activate (smart chain), extras, file_drop (U12), force_click (U12)
│       ├── input/                     # keyboard (SendInput), mouse (SendInput), clipboard (Win32)
│       ├── events/                    # NEW — watch (U7): MTA thread, UIA event handlers
│       ├── text/                      # NEW — U8: TextPattern helpers
│       ├── notifications/             # U3a: list, dismiss, interact
│       ├── tray/                      # U3b: list, interact (Shell_TrayWnd UIA)
│       └── system/                    # app_ops, window_ops, key_dispatch, permissions, screenshot (U9 modern + legacy), wait
├── macos/
│   └── src/
│       ├── tree/
│       │   ├── element.rs             # SPLIT (was 404 L) — keep attribute reads
│       │   └── element_selectors.rs   # NEW — Unit 5: AXIdentifier / Subrole / RoleDescription / PlaceholderValue / DOMIdentifier / DOMClassList readers
│       ├── events/                    # NEW — Unit 7: AXObserver + CFRunLoop worker
│       ├── text/                      # NEW — Unit 8: parameterized-attribute helpers
│       └── system/
│           ├── screenshot.rs          # Unit 9: ScreenCaptureKit default, subprocess legacy (split into modern.rs + legacy.rs if LOC pressure)
│           └── permissions.rs         # Unit 11: tri-state (AX + Screen Recording + Automation)
├── ffi/
│   ├── build.rs                       # Unit 2: extend — uses build-helpers::enumerate_commands to generate ad_* wrappers alongside cbindgen header
│   └── src/
│       ├── generated/                 # NEW — include!() target for build.rs output
│       │   └── wrappers.rs            # generated from registry; committed-and-drift-checked like include/agent_desktop.h
│       ├── abi_version.rs             # NEW — Unit 1: ad_abi_version() export + AD_ABI_VERSION_MAJOR cbindgen define
│       ├── log_callback.rs            # NEW — Unit 2: ad_set_log_callback installs a tracing_subscriber layer
│       └── ...                        # existing adapter.rs, error.rs, ffi_try.rs, etc. unchanged
skills/
├── agent-desktop/                     # Unit 14: update core skill for three-platform
├── agent-desktop-ffi/                 # Unit 14: update for ad_abi_version + ad_set_log_callback
└── agent-desktop-windows/             # NEW — Unit 14: SKILL.md + references/uia.md, references/windows-permissions.md, references/chromium.md
.github/workflows/
├── ci.yml                             # Unit 13: +test-windows job
└── release.yml                        # Unit 13: +aarch64-pc-windows-msvc + Windows CLI row
npm/
└── scripts/
    └── postinstall.js                 # Unit 13: +win32-x64 + win32-arm64 branches
```

The implementer may adjust subfolder shape during implementation if it improves clarity; the per-unit `**Files:**` sections are authoritative for what each unit creates.

## High-Level Technical Design

> *The diagrams below illustrate intended approach and are directional guidance for review, not implementation specification.*

### Dependency graph across units

```mermaid
flowchart LR
  U1[U1: Core pre-work<br/>types + trait methods + MSRV]
  U2[U2: Registry migration<br/>build.rs filesystem enumeration codegen]
  U3[U3: Windows adapter<br/>UIA tree/actions/input/system]
  U3a[U3a: Windows notifications]
  U3b[U3b: Windows tray]
  U4[U4: Windows Electron compat]
  U5[U5: Stable-selector population]
  U6[U6: Action variants<br/>LongPress/ShowMenu/WindowRaise/Cancel]
  U7[U7: watch_element<br/>AXObserver + UIA events]
  U8[U8: Text range primitives]
  U9[U9: Modern screenshot]
  U10[U10: New surfaces]
  U11[U11: Permission tri-state macOS]
  U12[U12: DeliverFiles + ForceClick]
  U13[U13: Windows CI + release matrix]
  U14[U14: Skills + README + phases.md sync]

  U1 --> U2
  U1 --> U3
  U1 --> U4
  U1 --> U5
  U1 --> U6
  U1 --> U7
  U1 --> U8
  U1 --> U9
  U1 --> U10
  U1 --> U11
  U1 --> U12
  U2 --> U3
  U2 --> U5
  U2 --> U6
  U2 --> U7
  U2 --> U8
  U2 --> U9
  U2 --> U10
  U2 --> U11
  U2 --> U12
  U3 --> U3a
  U3 --> U3b
  U3 --> U4
  U3 --> U7
  U3 --> U8
  U3 --> U9
  U3 --> U10
  U3 --> U12
  U13 -.parallel to U3..U12.-> U3
  U3 --> U13
  U13 --> U14
  U3a --> U14
  U3b --> U14
  U4 --> U14
  U5 --> U14
  U6 --> U14
  U7 --> U14
  U8 --> U14
  U9 --> U14
  U10 --> U14
  U11 --> U14
  U12 --> U14
```

### `watch_element` lifecycle (directional)

```
CLI: wait --event value-changed --ref @e5 --timeout 3000
  │
  ▼
commands/watch.rs
  │  resolve ref → NativeHandle
  ▼
PlatformAdapter::watch_element(&handle, &[ValueChanged], 3000ms)
  │
  ├─ macOS: spawn worker → CFRunLoopRun
  │     AXObserverCreate(pid) → AddNotification(kAXValueChangedNotification)
  │     callback pushes ElementEvent into mpsc::Sender
  │     timeout: CFRunLoopStop + JoinHandle::join
  │
  └─ Windows: spawn worker → CoInitializeEx(MTA)
        IUIAutomation.AddPropertyChangedEventHandler
        handler pushes ElementEvent into mpsc::Sender
        timeout: RemoveEventHandler + JoinHandle::join
  │
  ▼
main thread: recv with Duration until deadline → Vec<ElementEvent>
  │
  ▼
JSON envelope with events array
```

### Registry → codegen flow (Unit 2 — research-refined: build.rs filesystem enumeration)

```
crates/core/src/commands/<cmd>.rs     ← source of truth (file system)
  │
  │  top-of-file marker:
  │    ///! command_meta { name = "click", summary = "Click an element by ref" }
  │
  │  body:
  │    pub fn descriptor() -> CommandDescriptor { … }
  ▼
build-helpers::enumerate_commands(Path)  ← pure file walk + regex, zero linker magic
  │
  ├─ crates/core/build.rs → emit $OUT_DIR/registry.rs
  │    pub static DESCRIPTORS: &[CommandDescriptor] = &[ click::descriptor(), … ];
  │
  ├─ crates/ffi/build.rs  → emit $OUT_DIR/wrappers.rs
  │    #[no_mangle] pub extern "C" fn ad_click(…) -> AdResult { … }
  │    #[no_mangle] pub extern "C" fn ad_type_text(…) -> AdResult { … }
  │    …
  │
  ├─ src/dispatch.rs (CLI) → DESCRIPTORS.iter().find(|d| d.name == name)
  │
  └─ crates/mcp/ (Phase 4) → same build-helpers::enumerate_commands →
                              emit rmcp #[tool] per descriptor

NO deterministic registry metadata, NO linkme, NO xtask, NO ctor sites,
NO link-GC mitigation needed — extern "C" symbols are directly exported
from the cdylib and visible via nm -g.
```

## Implementation Units

Each unit is one reviewable PR unless explicitly flagged as multi-sub-PR (Unit 1 and Unit 2 are multi-sub-PR due to blast radius). Dependencies follow §High-Level Technical Design. The checkbox syntax drives progress tracking.

### - [ ] Unit 1: Core pre-work — types, trait method stubs, MSRV bump, FFI ABI version

**Goal:** Land every additive type change, trait method stub, and MSRV bump before any adapter code opens. Every P2-O* objective that mutates core types resolves its type surface here.

**Requirements:** R8, R9, R10, R11, R12, R13, R14, R16, R17

**Dependencies:** None (first unit)

**Files:**
- Modify: `Cargo.toml` (workspace `rust-version = "1.82"`)
- Modify: `rust-toolchain.toml` (no target change yet; that lives in U13)
- Modify: `crates/core/src/error.rs` (add `#[non_exhaustive]`; add `PermissionRevoked`, `ResourceExhausted`, `AxMessagingTimeout`, `AutomationPermissionDenied`)
- Modify: `crates/core/src/action.rs` (add `LongPress { duration_ms: u64 }`, `ForceClick`, `ShowMenu`, `DeliverFiles(Vec<std::path::PathBuf>)` (renamed from `FileDrop` per research), `WindowRaise`, `Cancel`, `SelectRange { start: u32, length: u32 }`, `InsertAtCaret(String)`)
- Modify: `crates/core/src/node.rs` (introduce `StableSelectors` struct; add `#[serde(flatten)] pub selectors: StableSelectors` field on `AccessibilityNode`)
- Modify: `crates/core/src/refs.rs` (add `identifier: Option<String>` to `RefEntry`; populate in allocator when available; prefer-identifier logic is Unit 5)
- Modify: `crates/core/src/adapter.rs` (extract `PermissionReport` into `crates/core/src/permission.rs` as tri-state struct; add trait methods `blocked_combos`, `watch_element`, `get_text_selection`, `set_text_selection`, `get_text_at`, `insert_text_at_caret`, `get_screenshot_with_backend`; all default to `not_supported()`)
- Create: `crates/core/src/event.rs` (`EventKind` enum with 10 variants; `ElementEvent` struct; `WatchSpec`)
- Create: `crates/core/src/text_range.rs` (`TextRange`, `TextSelection`)
- Create: `crates/core/src/screenshot_backend.rs` (`ScreenshotBackend { Modern, Legacy }`)
- Create: `crates/core/src/permission.rs` (tri-state `PermissionReport`; `TriState { Granted, Denied { suggestion: String }, Unknown }`)
- Modify: `crates/core/src/lib.rs` (re-exports)
- Modify: `crates/ffi/src/error.rs` (extend both `const fn` variant-count arrays with the 4 new variants; add matching `AdResult::Err*` discriminants preserving existing ordering; update `error_code_to_result` match)
- Modify: `crates/ffi/src/adapter.rs` (FFI-facing `AdPermissionReport` struct mirroring tri-state; FFI conversion helper)
- Create: `crates/ffi/src/abi_version.rs` (`pub const AD_ABI_VERSION_MAJOR: u32 = 1;` at v0.1.14 anchor; bumped to `2` at sub-PR 1g when layout actually changes) + `ad_abi_version()` extern "C" returning `u32` + `ad_init(expected_major: u32) -> AdResult` enforced version-negotiation handshake + `pub const AD_RESULT_UNKNOWN: i32 = -99;` sentinel exported to cbindgen
- Modify: `crates/ffi/cbindgen.toml` (add `AD_ABI_VERSION_MAJOR` to `[defines]`)
- Modify: `crates/ffi/include/agent_desktop.h` (regenerated, committed)
- Modify: `src/cli_args.rs` (new arg structs are stubbed/empty until U6, U7, U8 fill; cli.rs arm names reserved to prevent renumbering)
- Modify: `src/dispatch.rs` (map reserved arms to new command modules with `unimplemented!()` gated behind test-only until U6/U7/U8 land — **except** the Unit 2 dispatcher variant, which lands in U2 and never uses `unimplemented!()` in shipped binaries)
- Test: `crates/core/src/node.rs` (new tests: flatten shape, serde roundtrip, skip_serializing_if per selector field)
- Test: `crates/core/src/error.rs` (new variants serialize to SCREAMING_SNAKE_CASE)
- Test: `crates/core/src/action.rs` (new variants serde roundtrip including `PathBuf` in `DeliverFiles`)
- Test: `crates/core/src/permission.rs` (tri-state struct serde; JSON shape matches spec)
- Test: `crates/core/src/event.rs`, `text_range.rs`, `screenshot_backend.rs` (serde roundtrips)
- Test: `crates/ffi/tests/abi_version.rs` (ad_abi_version returns `AD_ABI_VERSION_MAJOR`)
- Test: `crates/ffi/src/error.rs` (parity-count assertion passes; `ErrorCode::PermissionRevoked` → `AdResult::ErrPermissionRevoked`)

**Approach:**
**Sub-PR ordering (refined by deepening pass — `ad_abi_version` first so every later ABI-affecting sub-PR can CI-assert it bumped):**

- **Sub-PR 1a: `ad_abi_version()` export + FFI policy publication (document-review refinement).** Add `crates/ffi/src/abi_version.rs` with `pub const AD_ABI_VERSION_MAJOR: u32 = 1;` (anchor Phase 1 implicit value explicitly — **not** 2 yet, to avoid the mid-series lie where consumers see `ad_abi_version() = 2` while the struct layout is still v1 shape) and `ad_abi_version()` extern "C". Add the `AD_ABI_VERSION_MAJOR` cbindgen `[defines]` entry. Create `crates/ffi/README.md` documenting the policy matrix from KD17. Regenerate committed header. This lands **first** so every subsequent sub-PR in 1b–1j can assert via CI grep: "if `crates/ffi/src/error.rs` OR `crates/ffi/src/generated/` OR `crates/core/src/permission.rs` is touched and the change is ABI-breaking, `AD_ABI_VERSION_MAJOR` must have bumped since the last main-branch commit."
- Sub-PR 1b: `ErrorCode` gets `#[non_exhaustive]` alone. No variant additions. Parity const assertions unchanged. Breaks nothing. Per KD17, `#[non_exhaustive]` addition does NOT bump `AD_ABI_VERSION_MAJOR`.
- Sub-PR 1c: Add 4 new `ErrorCode` variants atomically with matching `AdResult::Err*` discriminants and the `error_code_to_result` arm. Parity assertion passes — this is the gate. Per KD17, additive variants under `#[non_exhaustive]` do NOT bump `AD_ABI_VERSION_MAJOR`.
- Sub-PR 1d: Add 8 new `Action` variants. Platform adapters' existing `execute_action` arms fall through to `not_supported()` via the `#[non_exhaustive]` default — no adapter changes yet.
- Sub-PR 1e: Introduce `StableSelectors` on `AccessibilityNode` with `#[serde(flatten)]`. Construct `StableSelectors::default()` everywhere an `AccessibilityNode` is built today (macOS `tree/builder.rs`, test fixtures). Wire shape unchanged.
- Sub-PR 1f: `RefEntry.identifier: Option<String>` with serde `skip_serializing_if`. Populated with `None` for now — U5 adds the reader logic.
- Sub-PR 1g: `PermissionReport` extracted to `crates/core/src/permission.rs` as tri-state struct. The `permissions` command JSON output changes shape — **breaking**. This sub-PR **atomically bumps `AD_ABI_VERSION_MAJOR` from 1 → 2** (document-review refinement — the version bumps only when the layout actually changes, avoiding mid-series lies). macOS adapter's existing single-state response maps to `accessibility: Granted | Denied`; `screen_recording` and `automation` start `Unknown` until U11.
- Sub-PR 1h: Add trait methods with `not_supported()` defaults. Signatures:
  - `fn blocked_combos(&self) -> &'static [KeyCombo]`
  - `fn watch_element(&self, entry: &RefEntry, spec: &WatchSpec) -> Result<Vec<ElementEvent>, AdapterError>` — takes `&RefEntry`, **not** `&NativeHandle`, so the worker thread can re-resolve on its own thread and the `NativeHandle` `!Send`/`!Sync` invariant is preserved (deepening-pass refinement — KD9).
  - `fn get_text_selection(&self, handle: &NativeHandle, config: &TextRangeConfig) -> Result<TextSelection, AdapterError>`
  - `fn set_text_selection(&self, handle: &NativeHandle, config: &TextRangeConfig, range: TextRange) -> Result<(), AdapterError>`
  - `fn get_text_at(&self, handle: &NativeHandle, config: &TextRangeConfig, range: TextRange) -> Result<String, AdapterError>`
  - `fn insert_text_at_caret(&self, handle: &NativeHandle, config: &TextRangeConfig, text: &str) -> Result<(), AdapterError>`
  - `fn get_screenshot_with_backend(&self, target: ScreenshotTarget, config: &ScreenshotBackendConfig) -> Result<ImageBuffer, AdapterError>` — takes `&ScreenshotBackendConfig`, **not** just `ScreenshotBackend`, so per-backend options (dimensions, pixel format, encoding) travel through one argument (deepening-pass refinement).
- Sub-PR 1i: Create `event.rs`, `text_range.rs`, `screenshot_backend.rs` supporting types + `ActionDispatchConfig`, `WatchElementConfig`, `TextRangeConfig`, `ScreenshotBackendConfig` (the 4 shared parameter types per §Context & Research). Extend `TreeOptions` with `force_electron_a11y: bool`. Serde roundtrip tests. No `EventWorker` or `NotificationSession` traits — both dropped as premature abstraction per scope review.
- Sub-PR 1j: Reserve CLI arms for U6/U7/U8 commands — `src/cli.rs` gains variants marked `#[clap(hide = true)]` (document-review refinement — so unimplemented commands do NOT appear in `agent-desktop --help` output during the Phase 2 intermediate window). Their `dispatch.rs` arms route to `AppError::invalid_input("command not yet implemented")` (NOT `unimplemented!()`) until the later units fill them. Each unit that implements a reserved command also flips `hide = false`. This reserves name-ordering so each subsequent unit's PR is smaller, without leaking broken commands into user-facing help.

**Execution note:** Land sub-PRs serially (1a → 1j). Each must keep `cargo test --workspace` green. The `const _: () = assert!(…)` parity gate in `crates/ffi/src/error.rs` **is the atomic safeguard** — breaking either side of the pair fails the build at sub-PR 1b.

**Technical design:**

```rust
// node.rs — directional guidance, not final
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StableSelectors {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subrole: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dom_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dom_classes: Vec<String>,
}

pub struct AccessibilityNode {
    // existing 10 fields unchanged
    #[serde(flatten, default)]
    pub selectors: StableSelectors,
}
```

```rust
// permission.rs — tri-state
pub enum TriState {
    Granted,
    Denied { suggestion: String },
    Unknown,
}
pub struct PermissionReport {
    pub accessibility: TriState,
    pub screen_recording: TriState,
    pub automation: TriState,
}
```

**Patterns to follow:**
- Parity `const _: () = assert!(…)` pair in `crates/ffi/src/error.rs:5-60` — extend both arrays symmetrically.
- Serde `skip_serializing_if` idiom used throughout `crates/core/src/node.rs`.
- `#[non_exhaustive]` on `Action` (existing) — mirror for `ErrorCode`.

**Test scenarios:**
- **Happy path:** `AccessibilityNode` with populated `StableSelectors` serializes with `identifier`, `subrole`, `dom_classes` as flat top-level fields in JSON.
- **Edge case:** `AccessibilityNode` with `StableSelectors::default()` emits no selector fields in JSON (roundtrip preserves `None`/empty).
- **Happy path:** `PermissionReport { accessibility: Granted, screen_recording: Denied { ... }, automation: Unknown }` serializes to the expected tri-state object shape.
- **Happy path:** `ErrorCode::PermissionRevoked.as_str() == "PERMISSION_REVOKED"`; ditto the other three new variants.
- **Integration:** parity `const _: () = assert!(…)` in `crates/ffi/src/error.rs` compiles (gate).
- **Integration:** `ad_abi_version()` extern "C" returns `2u32`; cbindgen header exports `AD_ABI_VERSION_MAJOR 2` as a preprocessor define.
- **Integration:** every existing macOS unit test still passes after `PermissionReport` tri-state migration (accessibility field carries the legacy boolean behavior; others `Unknown`).
- **Error path:** `Action::DeliverFiles(vec![PathBuf::from("relative/path")])` serde roundtrips preserving `PathBuf`.

**Verification:**
- `cargo test --workspace` green.
- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo build -p agent-desktop-ffi` green; `scripts/update-ffi-header.sh` shows no drift beyond the intended additions.
- `cargo tree -p agent-desktop-core` still free of platform crates.

---

### - [ ] Unit 2: Registry migration — `build.rs` filesystem enumeration + codegen for FFI wrappers

**Goal:** Migrate the existing `ad_*` FFI wrappers from hand-written to codegen. Each command's `commands/<name>.rs` file is the source of truth; `build.rs` enumerates the filesystem at compile time and emits (a) a `CommandDescriptor` static array in `crates/core/src/generated/registry.rs` and (b) one `extern "C" fn ad_<name>` wrapper per command in `crates/ffi/src/generated/wrappers.rs`. This is P2-O16.

**Research-driven architecture shift:** the origin brainstorm's `inventory 0.3` + xtask proposal is REPLACED with pure `build.rs` filesystem enumeration (research Topic B — `inventory`/`linkme` link-GC is unreliable across ld64, ld-prime, GNU ld, lld, MSVC for cdylib consumers; `build.rs` is deterministic and has zero linker dependencies). The "one command per file" CLAUDE.md invariant is now load-bearing — it's the codegen contract.

**Scope (single-concern):** `ad_set_log_callback` is Unit 2.5; Phase 1.5 FFI backfill is Unit 2.6.

**Requirements:** R16

**Dependencies:** Unit 1 (needs `ad_abi_version`, new trait method stubs, tri-state `PermissionReport`, and the 4 shared config structs).

**Files:**
- Create: `build-helpers/` (new workspace member — tiny crate exposing `fn enumerate_commands(dir: &Path) -> Vec<CommandMeta>`. No runtime deps; pure file I/O + regex.)
- Modify: workspace `Cargo.toml` (add `build-helpers` to `[workspace.members]`; **NO** `inventory`, **NO** `linkme`, **NO** `schemars` — deferred to Phase 4 MCP)
- Create: `crates/core/build.rs` (uses `build-helpers::enumerate_commands`; emits `$OUT_DIR/registry.rs` with `pub static DESCRIPTORS: &[CommandDescriptor] = &[…];`)
- Create: `crates/core/src/registry.rs` (`pub struct CommandDescriptor { name, dispatch_fn, args_parse_fn }` + `include!(concat!(env!("OUT_DIR"), "/registry.rs"))`)
- Modify: `crates/core/src/commands/*.rs` (each file gets a top-of-file `///! command_meta { name = "...", summary = "..." }` marker parsed by `build-helpers` + a `pub fn descriptor() -> CommandDescriptor { … }` function. ONE registration point per command file — no macros, no inventory, no ctor.)
- Modify: `crates/core/src/lib.rs` (re-export `registry::DESCRIPTORS`)
- Modify: `src/dispatch.rs` (replace the giant `match` with `DESCRIPTORS.iter().find(|d| d.name == cmd_name).map(|d| (d.dispatch_fn)(args, adapter))`; retain clap-to-descriptor bridge in `src/cli.rs`)
- Modify: `crates/ffi/build.rs` (extend: uses `build-helpers::enumerate_commands` — SAME enumeration as core's build.rs; emits `$OUT_DIR/wrappers.rs` with one `extern "C" fn ad_<name>(...)` per command; stamp path in `target/ffi-wrappers-path.txt` — same pattern as cbindgen header stamp)
- Create: `crates/ffi/src/generated.rs` (`include!(concat!(env!("OUT_DIR"), "/wrappers.rs"));`)
- Modify: `crates/ffi/src/lib.rs` (wire `mod generated;` after hand-written wrappers are removed)
- Delete: hand-written `ad_<name>` wrappers across `crates/ffi/src/{actions,apps,input,notifications,observation,screenshot,surfaces,tree,windows}/`. Marshaling primitives in `crates/ffi/src/convert/`, `adapter.rs`, `error.rs`, `ffi_try.rs`, `main_thread.rs`, `pointer_guard.rs`, `types/`, `enum_validation.rs` stay.
- Modify: `crates/ffi/include/agent_desktop.h` (regenerated with generated wrappers in identical order)
- Modify: `.github/workflows/ci.yml` (add a "FFI header drift check" step — already present — plus the registry enumeration runs identically across all build profiles because it's pure file-walk, no linker involved)
- Modify: `scripts/update-ffi-header.sh` → rename to `scripts/update-ffi.sh` (shim for old name kept for backward compat); regenerates the cbindgen header
- Test: `crates/core/tests/registry_coverage.rs` (asserts `DESCRIPTORS.len()` equals the count of `.rs` files in `crates/core/src/commands/` excluding `mod.rs`/`helpers.rs`; fails loudly if a command file exists without a descriptor or vice versa)
- Test: `crates/core/tests/cli_registry_parity.rs` (cross-checks that every CLI subcommand name enumerated from clap has a matching `CommandDescriptor` AND vice versa; closes rename-detection gap)
- Test: `tests/integration/per_command_fixture_diff.rs` (for each migrated command, pre-migration JSON fixture vs post-migration byte-diff — empty diff or whitelisted with justification)

**Approach:**
- Migrate one command category at a time: **observation → interaction → system → clipboard → notifications → batch**. Each sub-PR keeps `cargo test --workspace` + `cargo test -p agent-desktop-ffi --tests` green.
- `CommandDescriptor` carries: `name: &'static str`, `dispatch_fn`, `args_parse_fn` (from `&clap::ArgMatches`). No schemars — deferred to Phase 4 MCP where schemas are actually consumed.
- **Codegen mechanism (research-refined):** pure `build.rs` filesystem enumeration of `crates/core/src/commands/*.rs`, NOT `inventory` / `linkme` / xtask. Research Topic B found neither inventory nor linkme survives link-GC across ld64, ld-prime, GNU ld, lld, and MSVC link.exe for cdylib consumers; `deterministic registry metadata` ctor sites are stripped when an `rlib` is linked into a binary that never references a symbol from that rlib. `build.rs` reads source files at compile time, parses a `///! command_meta { … }` block, and emits a deterministic static array. Zero linker magic, zero ctor sites, cdylib-safe by construction.
- The opening **spike sub-PR** validates the codegen on ONE command (`click`): builds `build-helpers::enumerate_commands`, emits generated `ad_click`, passes fixture byte-diff against hand-written `ad_click`, ships alone. Only after the spike merges does the category migration open.
- `build-helpers` workspace crate owns the enumeration logic (used by both `crates/core/build.rs` and `crates/ffi/build.rs`). Single source of truth for "what is a command".
- **Per-command fixture diff gate:** before migrating a category, the sub-PR captures pre-migration JSON fixtures for every command in that category. Post-migration, `per_command_fixture_diff.rs` asserts byte-equivalent output (or whitelisted + justified diff). "Green tests" alone is not sufficient gating.
- **No link-GC mitigation needed:** generated `extern "C" fn ad_<name>` symbols are directly exported from the cdylib (`nm -g` shows them). Cargo's default visibility for `pub extern "C"` items in cdylib targets keeps them live. Zero `#[used]` annotations, zero `--whole-archive` flags, zero CI matrix for link profiles.

**Execution note:** The spike PR opens first, on `click` only. Capture the codegen decision (build.rs filesystem vs alternatives) in the spike's commit message so reviewers see the chosen mechanism and its rationale (research Topic B).

**Patterns to follow:**
- `crates/ffi/build.rs` stamp-path idiom (learning 3).
- `crates/ffi/src/error.rs` `const` parity assertion — mirror for registry count: `const _: () = assert!(ffi_wrapper_count() == command_count(), "…");`.
- `crates/ffi/src/convert/` marshaling helpers stay per-type (strings, rects, windows, notifications). Generated wrappers call them.

**Test scenarios:**
- **Happy path:** `DESCRIPTORS.len() == 53` after migration; asserted by `crates/core/tests/registry_coverage.rs`.
- **Happy path:** every existing CLI command dispatches via registry; `cargo test --lib --workspace` preserves all existing assertions.
- **Happy path:** `ad_<name>` extern "C" symbols exist for every `CommandDescriptor` — checked by `nm -g target/debug/libagent_desktop_ffi.*` against the file enumeration count.
- **Integration:** `ad_click @e5` passes identical JSON to the generated wrapper as the hand-written wrapper did (pre/post-migration fixture byte-diff).
- **Integration:** `ad_abi_version()` still returns `AD_ABI_VERSION_MAJOR` unchanged through the migration.
- **Error path:** malformed JSON input to any generated `ad_<name>` wrapper returns `AdResult::ErrInvalidArgs` with a `set_last_error` message — not a panic or crash.
- **Integration:** CI header-drift check green after regenerating on migration PRs.
- **Edge case:** adding a new file `crates/core/src/commands/hello_world.rs` with a descriptor auto-registers — no other file needs editing; CLI, FFI wrapper, and registry all pick it up on next build.
- **Edge case:** removing a command file removes the descriptor and the `ad_<name>` wrapper without a runtime check (compile-time enumeration).

**Verification:**
- `cargo test --workspace` + `cargo test -p agent-desktop-ffi --tests` green.
- Generated artifacts under `$OUT_DIR/` regenerate deterministically (same input = same output, same byte ordering).
- `crates/ffi/include/agent_desktop.h` regenerated with identical symbol ordering as before (ordering test pins this).
- No workspace-level `inventory` / `linkme` / `xtask` dependencies introduced.

---

### - [ ] Unit 2.5: `ad_set_log_callback` with redaction layer

**Goal:** Ship the FFI log-forwarding callback with a mandatory redaction layer, as a small standalone unit. Split out from Unit 2 by review — three concerns in one unit was rejected.

**Requirements:** R16 (partial — log callback portion)

**Dependencies:** Unit 2 (registry migration lands first so the FFI crate is stable)

**Files:**
- Create: `crates/ffi/src/log_callback.rs` (`ad_set_log_callback(cb: extern "C" fn(level: i32, msg: *const c_char))` installs a filtered `tracing_subscriber::Layer`)
- Create: `crates/ffi/src/log_redaction.rs` (**security-critical**: filters tracing events before emission to the callback)
- Modify: `crates/ffi/src/lib.rs` (export `ad_set_log_callback`)
- Modify: `skills/agent-desktop-ffi/references/threading.md` (callback lifetime invariants)
- Modify: `crates/ffi/README.md` (callback security contract — section "Log callback responsibilities")
- Test: `crates/ffi/tests/log_callback.rs` (invoking any `ad_*` emits a tracing event; callback receives a redacted message)
- Test: `crates/ffi/tests/log_redaction.rs` (tracing events containing `value=<secret>`, `password=<secret>`, `clipboard=<secret>`, `token=<secret>` arrive at the callback with values replaced by `<redacted>`)
- Test: `crates/ffi/tests/log_callback_reentry.rs` (setting the callback twice returns `ErrorCode::InvalidArgs` with the "callback already installed" suggestion)

**Approach:**
- Global `OnceCell<extern "C" fn(i32, *const c_char)>` stores the callback; second registration fails closed with `InvalidArgs`.
- The redaction layer sits between `tracing_subscriber` and the callback. It filters:
  1. **Field-name allowlist/denylist:** any event field with a name matching `value`, `text`, `content`, `clipboard`, `password`, `secret`, `token`, `credential`, `auth` (case-insensitive) is replaced with `<redacted>` in the emitted string.
  2. **Level filter:** `TRACE`-level events are dropped by default; consumers opt in via `AGENT_DESKTOP_LOG_TRACE=1` env var.
  3. **Size cap:** any single event message longer than 4 KB is truncated with a `[truncated]` marker to prevent memory amplification attacks.
- Callback invocation is wrapped in `catch_unwind` — a consumer-side panic does not propagate back through the FFI boundary.
- Documented contract: "Callbacks must not persist received events to storage or network without explicit user consent. agent-desktop ships redaction as defense-in-depth; the primary trust boundary is the FFI consumer."

**Patterns to follow:**
- Existing `crates/ffi/src/ffi_try.rs` `trap_panic` wrapper shape.
- `crates/ffi/src/main_thread.rs` for global-state access discipline.

**Test scenarios:**
- **Happy path:** `ad_set_log_callback(cb)` + any `ad_*` call → `cb` receives a C-string message.
- **Security:** `type-text @e5 "hunter2"` emits a tracing event; the callback receives `type-text ref=@e5 text=<redacted>` (not the literal password).
- **Security:** clipboard-set emits `clipboard=<redacted>` regardless of content.
- **Edge case:** message > 4 KB → truncated with marker.
- **Edge case:** `TRACE` events dropped unless env var set.
- **Edge case:** callback that panics → `catch_unwind` isolates; library continues.
- **Error path:** second `ad_set_log_callback` call returns `InvalidArgs`.

**Verification:**
- All tests green.
- `crates/ffi/README.md` section on callback responsibilities published and linked from `skills/agent-desktop-ffi/SKILL.md`.

---

### - [ ] Unit 2.6: Phase 1.5 FFI backfill — `ad_snapshot` / `ad_execute_by_ref` / `ad_wait` / `ad_version` / `ad_status`

**Goal:** Backfill the five FFI wrappers identified as missing in `docs/plans/2026-04-16-001-fix-ffi-safety-abi-correctness-plan.md`. Originally bundled into Unit 2; split out by review so Unit 2 stays single-concern.

**Requirements:** R16 (Phase 1.5 backfill portion)

**Dependencies:** Unit 2 (registry migration) — the 5 backfilled wrappers are new `CommandDescriptor` entries, not hand-written wrappers.

**Files:**
- Modify: `crates/core/src/commands/snapshot.rs` (confirm the descriptor — added by U2 via build.rs filesystem enumeration — carries refmap-pipeline args correctly; includes `skeleton` and `root_ref` CLI bindings)
- Modify: `crates/core/src/commands/status.rs`, `version.rs`, `wait.rs` (same — verify descriptors are correct)
- Create: `crates/core/src/commands/execute_by_ref.rs` (new thin wrapper that takes `(ref_id, action_json)` and dispatches via `PlatformAdapter::execute_action`)
- Modify: `src/cli.rs` + `src/dispatch.rs` + `src/cli_args.rs` (add `ExecuteByRef` command arm)
- Test: `crates/ffi/tests/backfill_gaps.rs` (asserts `ad_snapshot`, `ad_execute_by_ref`, `ad_wait`, `ad_version`, `ad_status` are all exported and callable; closes the v0.1.13 known-gaps list)
- Test: `crates/core/src/commands/execute_by_ref.rs` (unit — resolves ref then dispatches action; propagates STALE_REF and INVALID_ARGS correctly)

**Approach:**
- `execute_by_ref` accepts a serialized `Action` JSON + ref id. Resolves the ref via `RefMap::load`, then calls `adapter.execute_action`. This is the CLI-shell-independent agent primitive that FFI consumers use for agent loops.
- The other four (`snapshot`, `wait`, `version`, `status`) already exist as CLI commands; this unit only confirms that their `CommandDescriptor` entries carry the full arg surface (refmap pipeline for snapshot, multi-mode flags for wait, structured output for status).

**Patterns to follow:**
- `crates/core/src/commands/click.rs` minimal shape.
- `crates/core/src/commands/snapshot.rs` for the refmap pipeline.

**Test scenarios:**
- **Happy path:** `ad_execute_by_ref` with valid ref + `{"type":"Click"}` action → calls `PlatformAdapter::execute_action(Click)` → returns ActionResult JSON.
- **Error path:** stale ref → `AdResult::ErrStaleRef` propagates cleanly.
- **Error path:** malformed action JSON → `AdResult::ErrInvalidArgs` with diagnostic.
- **Integration:** `ad_snapshot` exposes all flags from CLI (skeleton, root, surface, max-depth, include-bounds, interactive-only, compact).

**Verification:**
- Backfill test green; `crates/ffi/include/agent_desktop.h` exports all 5 `ad_*` functions.

---

### - [ ] Unit 3: Windows adapter foundation

**Goal:** `WindowsAdapter` implements every existing 53-command's trait method via UIA (`uiautomation 0.24`) + `windows 0.62.2`. Base capability parity with macOS. Skeleton traversal (`--skeleton`, `--root @ref`) works identically on Windows. P2-O1, P2-O2, P2-O3, P2-O5, P2-O6, P2-O7 (partial).

**Requirements:** R1, R2, R3, R5, R6, R7

**Dependencies:** Unit 1, Unit 2

### Windows Engineering Invariants (research-driven, MUST land in sub-PR 3.0 before any other U3 sub-PR)

All invariants validated against Microsoft docs (UIA Threading 2025-07-14, UIA Security 2026-02-18) and production UIA tooling. Source: `/tmp/agent-desktop-research/windows-headless.md`.

1. **DPI awareness at process startup:** `main.rs` Windows branch calls `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` before any UIA call. Without this, UIA returns coordinates in virtualized pixels and click/hover synthesis drifts on mixed-DPI setups.
2. **COM apartment on the main thread and UIA workers:** `CoInitializeEx(NULL, COINIT_MULTITHREADED)` is called once at startup and once per dedicated UIA worker thread. Research-backed: UIA explicitly prefers MTA for cross-thread event delivery; `IUIAutomationElement` is apartment-affine — caching elements across thread boundaries invalidates them.
3. **Never cache `IUIAutomationElement` across apartments.** NativeHandle stores an element obtained in the current apartment and is not safe to pass to a thread of different apartment affinity. Workers that need elements re-resolve from `RefEntry` on their own thread. Event handlers are created, registered, removed, and drained on the same dedicated MTA thread.
4. **UIA-first, SendInput-fallback.** Every action attempts the UIA pattern first (`InvokePattern`, `ValuePattern`, `TogglePattern`, `ExpandCollapsePattern`, `SelectionItemPattern`) — all focus-independent and headless-safe. Only when no pattern applies does the adapter fall back to `SendInput`, gated by an `AttachThreadInput` + `SetFocus` worker-thread dance. SendInput is documented as focus-dependent and silently blocked by UIPI against elevated targets.
5. **`PostMessage WM_KEYDOWN` is DEAD for modern apps.** Chromium (Electron, Edge, Chrome, VS Code, Slack, Cursor), UWP apps, and games all ignore posted keyboard messages. It's NOT a viable SendInput alternative; do not implement.
6. **UIPI elevation detection.** `GetTokenInformation(TokenIntegrityLevel)` compares the agent-desktop process's integrity level against the target window's (`GetWindowThreadProcessId` → open process → query integrity). If mismatch, return `PermDenied` with `platform_detail: "Target window runs at higher integrity level; SendInput and some UIA patterns are blocked by UIPI. Run agent-desktop with matching elevation."` Do NOT ship the `uiAccess=true` manifest flag by default — it requires code signing and a specific install location; document as an optional signed-release path.
7. **`RemoveAutomationEventHandler` teardown race.** Per Microsoft's 2025 UIA threading doc, the handler object must outlive the final dispatched callback. Implementation: store handlers in an `Arc<Handler>` clone map keyed by `(element_id, event_kind)`; `RemoveAutomationEventHandler` is called on the UIA client, but the `Arc<Handler>` is dropped only AFTER a post-remove barrier call (a no-op UIA method call that drains any in-flight callback dispatch).
8. **HRESULT encoding in `platform_detail`.** Standardized format: `COM HRESULT 0x80070005 (E_ACCESSDENIED: Access is denied)` — hex + FACILITY + symbolic name + `FormatMessageW` description. Parsed by a shared helper `crate::error::format_hresult(hr: HRESULT) -> String`.
9. **DWM composition caveat for legacy screenshot.** `PrintWindow(hwnd, hdc, 0)` returns black frames for composited windows on Windows 10+. `PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)` mitigates — use the flag unconditionally in legacy backend. `windows-capture` (modern) handles composition correctly.
10. **`ElementFromHandle(hwnd)` is headless-safe within the current desktop session** — works on same-user, same-session visible or minimized windows at an accessible integrity level regardless of focus. This is the foundation for observation headlessness. No focus side-effect.
11. **`Windows.Graphics.Capture` availability:** requires DWM and Windows 10 1903+ in an active interactive session. Fails in Session 0, Server Core, secure desktop, locked desktop, and some remote/virtualized sessions — document and return `PlatformNotSupported` in those environments. agent-desktop's own process does NOT need an HWND — `windows-capture` uses `GraphicsCaptureItem.CreateFromWindowHandle(target_hwnd)`.
12. **Session isolation:** agent-desktop cannot drive windows in other user sessions, Session 0, secure desktop, or locked desktops. Document; return `WindowNotFound`, `PermDenied`, or `PlatformNotSupported` with explanatory `platform_detail`.
13. **Foreground APIs are explicit only:** `SetForegroundWindow`, `SetFocus`, `AttachThreadInput`, and `SetWindowPos(HWND_TOP)` are allowed only in commands/policies that explicitly request focus/window/physical side effects. They are never fallback paths for semantic ref actions.
14. **Windows shell is part of the product surface:** Start menu/search, taskbar, system tray/overflow, Action Center, Quick Settings, UAC/integrity, virtual desktop detection, and mixed-DPI monitor geometry are first-class Phase 2 cases. Each must be handled by a command/surface, an integration test, or a documented structured unsupported response.

### Skeleton traversal on Windows

The skeleton primitive (`snapshot --skeleton`, `snapshot --root @ref`) is platform-agnostic at the core level (`crates/core/src/snapshot_ref.rs`). Windows adapter contribution is ~50 LOC:

- **`get_subtree(handle, opts)`** delegates to `build_subtree(element, opts)` where `element` is the `IUIAutomationElement` held by `NativeHandle`. Mirrors macOS's `adapter.rs:221-241` shape.
- **Walker:** `ControlViewWalker` (NOT `RawViewWalker` or `ContentViewWalker`). Research Topic 4 confirmed: `IsControlElement` auto-filters layout noise and complements Unit 4's Electron depth-skip.
- **`children_count`** for skeleton boundary annotation: single `FindAll(TreeScope_Children, TrueCondition)` round-trip. Cheap enough — one COM call, no per-child property fetch.
- **Depth clamping** at `opts.max_depth = 3` in skeleton mode is enforced in core's `build_subtree`; Windows adapter just respects the passed depth.
- **Scoped invalidation** is entirely core-level (`RefMap::remove_by_root_ref`). Windows inherits with zero adapter code.
- **Fresh `UICacheRequest` per drill-down call** — cached elements do not survive CLI process boundaries. Each `snapshot --root @ref` allocates a new cache request; no attempt at cross-invocation caching.
- **Token savings target:** 50-100× on VS Code / Slack Electron once Unit 4's `--force-electron-a11y` + empty-`UIA_Group` / `UIA_Custom` depth-skip are in place.

**Files:**
- Modify: `crates/windows/Cargo.toml` (target-gated `[target.'cfg(target_os = "windows")'.dependencies]` gains `uiautomation = "0.24"`, `windows = { version = "0.62.2", features = ["Win32_UI_Input", "Win32_UI_Input_KeyboardAndMouse", "Win32_System_Com", "Win32_System_DataExchange", "Win32_UI_WindowsAndMessaging", "Win32_Graphics_Gdi"] }`; `windows-capture` is deferred to U9; OLE/clipboard features for U12)
- Modify: `crates/windows/Cargo.toml` (mirror the deps; inherit `agent-desktop-core` from workspace)
- Rewrite: `crates/windows/src/lib.rs` (mod declarations + re-exports of `WindowsAdapter`)
- Create: `crates/windows/src/adapter.rs` (`WindowsAdapter` struct + `impl PlatformAdapter`)
- Create: `crates/windows/src/tree/{mod,element,builder,roles,resolve,surfaces}.rs` (UITreeWalker + UICacheRequest)
- Create: `crates/windows/src/actions/{mod,dispatch,activate,extras}.rs` (InvokePattern → TogglePattern → coord fallback)
- Create: `crates/windows/src/input/{mod,keyboard,mouse,clipboard}.rs` (SendInput, Win32 clipboard)
- Create: `crates/windows/src/system/{mod,app_ops,window_ops,key_dispatch,permissions,screenshot,wait}.rs` (`screenshot` here is the Phase 1 `PrintWindow` equivalent — legacy backend; modern is U9)
- Create: `crates/windows/src/helpers/pid.rs` (`narrow_pid: u32 → Result<i32, AdapterError>` returning `ErrorCode::ResourceExhausted` per KD3; document-review refinement — this aligns with KD3's semantic choice)
- Modify: `src/main.rs` (`build_adapter()` `#[cfg(target_os = "windows")]` branch flips from stub to real adapter)
- Test: `crates/windows/src/tree/roles.rs` (`#[cfg(test)]` — unit: every `UIA_*ControlTypeId` maps to a known role string)
- Test: `crates/windows/tests/it_tree.rs` (integration, Windows CI only: snapshot of `explorer.exe`, `notepad.exe`, `calc.exe` — non-empty tree with refs)
- Test: `crates/windows/tests/it_actions.rs` (click button in a test WinUI app; verify via UIA property read after the action)
- Test: `crates/windows/tests/it_input.rs` (`SendInput` types "hello" into Notepad; verify `Value` property == "hello")
- Test: `crates/windows/tests/it_clipboard.rs` (get / set / clear roundtrip with ASCII + CJK via `CF_UNICODETEXT`)
- Test: `crates/windows/tests/it_window_ops.rs` (resize, move, minimize, maximize, restore against a newly launched Notepad)
- Test: `crates/windows/tests/it_app_lifecycle.rs` (launch Notepad, type, close)

**Approach:**
- Tree walk: `IUIAutomation.ElementFromHandle(hwnd) → UITreeWalker.RawViewWalker.GetFirstChild/GetNextSibling` with `UICacheRequest` batching `Name, ControlType, AutomationId, LocalizedControlType, HelpText, BoundingRectangle, IsEnabled, HasKeyboardFocus, ExpandCollapseState, ToggleState, ValueValue` in one round-trip (origin §Windows API mapping).
- Role map: `UIA_*ControlTypeId` → unified role strings (matches macOS `tree/roles.rs` output). Table lives in `crates/windows/src/tree/roles.rs` and is exhaustive against the 50 UIA control types.
- Action dispatch: pattern-first — `InvokePattern.Invoke` → `TogglePattern.Toggle` → `ExpandCollapsePattern.Expand` → coordinate-click fallback via `SendInput`. Mirrors macOS smart-chain pattern; share dispatch config via `ActionDispatchConfig` in `crates/core` per learning 1.
- `blocked_combos()` returns `[alt+f4, ctrl+alt+del, win+l, ctrl+shift+esc]`. Key-down / key-up safety check matches the whole combo (KD11).
- PID narrowing: `fn narrow_pid(dword: u32) -> Result<i32, AdapterError>` errors with `ErrorCode::ResourceExhausted` (per KD3; document-review reconciliation — previously said `Internal`) + `platform_detail: "PID exceeds i32::MAX; Windows kernel should never produce this value"` for values above `i32::MAX`. Unlikely in practice; belt-and-braces.
- `NativeHandle` inner is `AutomationElement` (`uiautomation::UIElement`) held by pointer; `release_handle` calls `ComPtr::Release` equivalent.
- Clipboard: `OpenClipboard(null)` + `CF_UNICODETEXT` get/set with a UTF-16 ↔ String conversion.
- Screenshot: `PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)` — this is the **legacy** path in this unit; U9 adds the modern `windows-capture` backend as the default.
- App launch: `CreateProcessW` + `WaitForInputIdle` + resolve via `ElementFromHandle` on the main window.

**Execution note:** Keep each major surface (tree, actions, input, system) as its own sub-PR when possible to keep reviews manageable. The tree builder and UIA role map can land in one sub-PR (`tree/` complete); actions + input in a second; clipboard + app lifecycle + window ops in a third; permissions + screenshot (legacy) in a fourth. Windows CI from U13 must be in place before this unit's integration tests can run — land U13 **in parallel** against a draft branch early.

**Patterns to follow:**
- Platform subfolder layout in `crates/macos/src/` (tree/, actions/, input/, system/).
- `AXElement` safety pattern (`pub(crate)` inner pointer, Clone/Drop via Retain/Release) — mirror for `UIElement` with `ComPtr`.
- Smart activation chain in `crates/macos/src/actions/activate.rs` and `chain_steps.rs` — define a cross-platform `ActionDispatchConfig` (learning 1) so Windows doesn't copy the macOS structure wholesale.

**Test scenarios:**
- **Happy path:** `snapshot --app Explorer` returns a tree with ≥20 refs including the sidebar, breadcrumb, and file list (R1).
- **Happy path:** `snapshot --app Notepad` returns a tree with a `textfield` ref for the document surface.
- **Happy path:** `click @e<button_ref>` on a known Settings button fires `InvokePattern.Invoke` and the UIA state changes.
- **Happy path:** `type @e<textfield_ref> "hello"` sets the Value via `SendInput`; re-snapshot shows `value="hello"` (R3).
- **Edge case:** `press cmd+q` on Windows maps `Cmd → Win` and is **blocked** per `blocked_combos()`; returns `ActionNotSupported` with suggestion.
- **Edge case:** UIA element present but offscreen — bounds reported, `BoundingRectangle` reflects actual coordinates after scroll.
- **Edge case:** stale `@e5` (window closed) returns `ErrorCode::StaleRef` with snapshot refresh suggestion.
- **Error path:** calling `launch --app notexist.exe` returns `ErrorCode::AppNotFound` with the lookup failure surfaced in `platform_detail`.
- **Error path:** `OpenClipboard` contention (another process holds the clipboard) retries then returns `ActionFailed` with HRESULT in `platform_detail`.
- **Integration:** `clipboard-set "héllo ✓"` + `clipboard-get` roundtrips Unicode exactly (R5).
- **Integration:** cross-platform JSON identity — snapshot of Calculator on macOS and Windows, serialized, structurally identical on role set and ref ordering (R2). Fixture lives in `tests/fixtures/calculator.macos.json` and `tests/fixtures/calculator.windows.json`; a test asserts the union of refs and the identifier map.
- **Integration:** `window_op Maximize/Minimize/Restore` roundtrips via `ShowWindow`; state observable via `GetWindowPlacement`.
- **Integration:** PID narrowing test — mock `u32::MAX` → `narrow_pid` returns `ErrorCode::ResourceExhausted` (document-review refinement); any realistic PID (<2^31) roundtrips.

**Verification:**
- Windows CI (`test-windows` job from U13) runs all integration tests green.
- `cargo tree -p agent-desktop-core` still platform-free on Windows.
- Binary size under 15 MB for `agent-desktop.exe` (check in `.github/workflows/release.yml`).

---

### - [ ] Unit 3a: Windows notifications

**Goal:** Windows Toast / Action Center parity with macOS 4-command surface (`list-notifications`, `dismiss-notification`, `dismiss-all-notifications`, `notification-action`) when the OS exposes a supported notification path. Primary path is `UserNotificationListener` with explicit user permission and app identity/capability; Action Center UIA is a best-effort fallback. Subset of P2-O14 and P2-O18.

**Requirements:** R14 (Windows subset)

**Dependencies:** Unit 3

**Files:**
- Create: `crates/core/src/notifications/session.rs` (**deepening-pass addition** — `NotificationSession` trait; existing macOS `crates/macos/src/notifications/nc_session.rs` refactors to implement it; Windows `listener.rs` and `action_center.rs` implement it. Prevents "batch dismiss" future fixes from landing on only one platform.)
- Modify: `crates/macos/src/notifications/nc_session.rs` (refactor to `impl NotificationSession for NcSession`)
- Create: `crates/windows/src/notifications/{mod,list,dismiss,interact,listener.rs,action_center.rs}` (`listener.rs` wraps `UserNotificationListener`; `action_center.rs` implements UIA fallback)
- Modify: `crates/windows/src/adapter.rs` (implement `list_notifications`, `dismiss_notification`, `dismiss_all_notifications`, `notification_action`)
- Test: `crates/windows/tests/it_notifications.rs` (CI integration — launch a known toast, list, dismiss, verify)
- Test: `crates/core/src/notifications/session.rs` (unit — trait contract tests that both platforms satisfy)

**Approach:**
- Primary listener access: use `UserNotificationListener` when package identity/capability and explicit permission are present. Denied permission maps to `PERM_DENIED` with a targeted suggestion.
- Action Center fallback: open via `open-system-surface --surface action-center`; traverse exposed shell UIA only when the toast list and action buttons have stable names/descriptions.
- Each toast: listener metadata or UIA element with `Name` = title, `FullDescription` = body, app info, and child action buttons with `InvokePattern`.
- `dismiss_notification`: prefer listener dismissal where supported; otherwise locate the toast's close button by stable identity (e.g. `AutomationId == "DismissButton"` when exposed) and invoke it.
- `dismiss_all_notifications`: listener bulk clear if supported; otherwise Action Center's "Clear all" button. Mirror macOS `dismiss_all` semantics.
- `notification_action(index, identity, action_name)`: enforce identity-fingerprint match (learning 4) — if identity fields are `Some`, compare against the toast's title/app before invoking. Returns `NotificationNotFound` on mismatch.
- Focus Assist state: use supported shell APIs first; registry/WNF probes are best-effort diagnostics and must never be the sole correctness signal.

**Patterns to follow:**
- `crates/macos/src/notifications/` (list.rs, actions.rs, nc_session.rs) — same PlatformAdapter arm shape, same JSON contract.
- Learning 4: identity fingerprint is optional, tri-state decoded at FFI.

**Test scenarios:**
- **Happy path:** `list-notifications` after posting a test toast returns ≥1 entry with `title` and `body` populated through the listener path when permission/app identity is available.
- **Happy path:** `dismiss-notification --index 1` removes the toast; re-list shows count decremented.
- **Happy path:** `dismiss-all-notifications` clears Action Center; re-list returns empty.
- **Edge case:** identity mismatch — `notification-action --index 1 --title "Wrong"` returns `NotificationNotFound` with suggestion to re-list.
- **Edge case:** Action Center closed and listener unavailable → list returns empty or `PLATFORM_NOT_SUPPORTED` depending on whether the shell exposes a supported fallback.
- **Error path:** no notifications present → `dismiss-notification --index 1` returns `NotificationNotFound`.
- **Integration:** cross-platform contract — same JSON keys, same 1-based indexing (compare with `crates/macos/tests/it_notifications.rs`).

**Verification:**
- Windows listener tests green where package identity is available; Action Center fallback tests green on interactive desktop runners; hosted CI keeps unit/mocked contract tests green when shell UI is absent.

---

### - [ ] Unit 3b: Windows shell surfaces + system tray

**Goal:** New command `open-system-surface --surface <kind>` plus `list-tray-items`, `click-tray-item`, `open-tray-menu` on Windows. Surfaces cover Start menu/search, taskbar, system tray/overflow, Action Center, Quick Settings, and shell flyouts. macOS gains the portable parts in Unit 10. Subset of P2-O14 and P2-O18.

**Requirements:** R14 (tray subset)

**Dependencies:** Unit 3

**Files:**
- Create: `crates/windows/src/tray/{mod,list,interact}.rs`
- Create: `crates/windows/src/system/shell_surfaces.rs`
- Create: `crates/core/src/commands/open_system_surface.rs`
- Create: `crates/core/src/commands/list_tray_items.rs`, `click_tray_item.rs`, `open_tray_menu.rs` (the core dispatchers; delegate to adapter trait methods)
- Modify: `crates/core/src/adapter.rs` (trait methods `open_system_surface`, `list_tray_items`, `click_tray_item`, `open_tray_menu` with `not_supported()` defaults)
- Modify: `crates/windows/src/adapter.rs` (implement)
- Modify: `src/cli.rs`, `src/cli_args.rs`, `src/dispatch.rs` (wire new commands)
- Modify: `crates/core/src/commands/mod.rs` (module list)
- Test: `crates/windows/tests/it_shell_surfaces.rs` (Start/taskbar/Action Center/Quick Settings surface open + snapshot, interactive runner)
- Test: `crates/windows/tests/it_tray.rs` (CI integration or self-hosted interactive job)

**Approach:**
- Access `Shell_TrayWnd` window class via `FindWindow`; walk its UIA tree; tray items are `UIA_ButtonControlTypeId` leaves with `AutomationId` = ICON GUID when set.
- Overflow: `NotifyIconOverflowWindow` class; expand by clicking the chevron button.
- `open-system-surface --surface start-menu`: use the documented shell keyboard command through explicit shell-surface policy, then snapshot Start via UIA. This is not used for `launch`, which remains direct `CreateProcess` / `ShellExecuteEx`.
- `open-system-surface --surface quick-settings` and `action-center`: use supported shell invocation where available; return `PLATFORM_NOT_SUPPORTED` with `platform_detail` when the Windows build/session has no accessible shell surface.
- `snapshot --surface taskbar`: walk `Shell_TrayWnd` task list and expose pinned/running app buttons as refs. Invoking a taskbar button is a focus/window action and must go through explicit policy.
- Virtual desktop detection uses public `IVirtualDesktopManager` for current-desktop filtering/diagnostics. Moving windows across virtual desktops is deferred until a stable public write path is validated.
- Mixed-DPI geometry uses process-wide per-monitor awareness plus physical-pixel normalization before any coordinate fallback.
- `click_tray_item --name "Network"`: resolve by `Name` or by `identifier`; fallback to coordinate click via `SendInput` for items without UIA patterns.
- `open_tray_menu --name "Volume"`: click + wait for the resulting popup (UIA focus-changed event from U7, or poll fallback before U7 lands).

**Patterns to follow:**
- Commands in `crates/core/src/commands/` stay slim — delegate to adapter (click.rs, list_windows.rs shape).
- Learning 1 — define `SurfaceDetectionConfig` so tray, menu, and notification surface walkers share shape.

**Test scenarios:**
- **Happy path:** `open-system-surface --surface start-menu` opens Start; `snapshot --surface start-menu` returns search/results controls with refs.
- **Happy path:** `snapshot --surface taskbar` returns pinned/running app buttons with stable names.
- **Happy path:** `open-system-surface --surface quick-settings` opens Quick Settings on Windows builds that expose it; unsupported builds return `PLATFORM_NOT_SUPPORTED`.
- **Happy path:** `list-tray-items` returns volume, network, clock entries on a baseline interactive Windows VM.
- **Happy path:** `click-tray-item --name "clock"` opens the clock flyout; re-list-surfaces shows the popup surface.
- **Edge case:** unknown tray item name → `ElementNotFound` with suggestion to run `list-tray-items`.
- **Edge case:** overflow item — list includes items in `NotifyIconOverflowWindow` transparently.
- **Error path:** tray or shell surface not accessible (Explorer shell missing, locked desktop, Server Core, unsupported build) → `WindowNotFound` or `PlatformNotSupported` with platform detail.

**Verification:**
- Windows interactive integration green; hosted CI keeps unit/mocked shell-surface contract tests green when Explorer shell is unavailable; `list-surfaces` on Windows now includes `SystemTray`, `Taskbar`, `StartMenu`, `ActionCenter`, and `QuickSettings` when present.

---

### - [ ] Unit 4: Windows Electron / WebView2 compatibility

**Goal:** Depth-skip for non-semantic UIA wrappers, resolver depth 50, surface detection treats focused window as the target surface, `--force-electron-a11y` CLI flag. Matches macOS Electron compat. P2-O15.

**Requirements:** R15

**Dependencies:** Unit 3 (needs the tree walker to patch)

**Files:**
- Modify: `crates/windows/src/tree/builder.rs` (web-wrapper depth-skip: `UIA_GroupControlTypeId` / `UIA_CustomControlTypeId` with empty `Name` AND empty `Value` do not consume depth budget)
- Modify: `crates/windows/src/tree/resolve.rs` (`ABSOLUTE_MAX_DEPTH = 50` mirror of macOS)
- Modify: `crates/windows/src/tree/surfaces.rs` (surface detection checks focused window itself against target surface shape)
- Modify: `crates/core/src/adapter.rs` (`TreeOptions.force_electron_a11y: bool`)
- Modify: `src/cli_args.rs` (snapshot args gain `--force-electron-a11y`)
- Modify: `crates/windows/src/adapter.rs` (if flag set, apply `renderer-accessibility` UIA override — check `ClassName == "Chrome_WidgetWin_1"` and set `AutomationMode` via `Direct UI` handshake where available; otherwise warn via `platform_detail`)
- Modify: `crates/macos/src/tree/builder.rs` (honor the new `force_electron_a11y` flag; pair macOS change lands in the same PR per origin §Phase 2 "land atomically")
- **Note:** the `docs/solutions/best-practices/electron-compat-cross-platform-2026-04-18.md` port from private memory moves to Unit 14 (document-review scope reconciliation — that port is an orthogonal documentation task, not Electron-compat implementation).
- Test: `crates/windows/tests/it_electron.rs` (CI: snapshot VS Code with and without `--force-electron-a11y`; assert ≥100 refs with the flag)
- Test: `crates/macos/tests/it_electron.rs` (existing; extend to validate the new flag is respected)

**Approach:**
- Depth-skip: mirror macOS `is_web_wrapper` logic — `UIA_GroupControlTypeId` or `UIA_CustomControlTypeId` with `Name.is_empty() && Value.is_empty()` returns true; wrapper is traversed but does not increment depth.
- Chromium detection: `GetClassNameW` on the HWND; if `Chrome_WidgetWin_1` and tree is empty, emit warning in `platform_detail` with `--force-renderer-accessibility` hint.
- Surface detection for Electron modals: when `surface = Sheet | Alert`, check if the focused window IS the target surface (not only its children). This matches macOS's pattern — Electron wraps dialogs as independent windows.
- `--force-electron-a11y`: on macOS, sets `AXEnhancedUserInterface = YES` on app root (existing). On Windows, attempt `SetPropertyValue(AutomationElementInformation.AutomationId, true)` on the root element; if the app does not honor the flag, emit a warning rather than erroring.

**Patterns to follow:**
- `crates/macos/src/tree/builder.rs` `is_web_wrapper` (exists today — check for `AXGroup` / `AXGenericElement` with empty name/value).
- Learning 4 — `Some("")` vs `None` bug: Chromium returns `Some("")`; always use `is_none_or(str::is_empty)`.

**Test scenarios:**
- **Happy path:** VS Code snapshot **without** `--force-electron-a11y` returns ~3 refs (matches current behavior); **with** the flag returns ≥100 refs (R15).
- **Happy path:** Slack snapshot with depth-skip enabled returns ≥50 refs.
- **Edge case:** non-Chromium window with empty-name groups is NOT depth-skipped (ensures the heuristic doesn't over-trigger on, say, empty GroupBoxes in WinForms).
- **Edge case:** Chromium app in "accessibility disabled" mode → empty tree + warning in `platform_detail` suggesting `--force-renderer-accessibility`.
- **Integration:** file-picker dialog in VS Code → `--surface sheet` detects it as the sheet surface (the dialog is a separate Electron window).
- **Integration:** `--force-electron-a11y` JSON output is identical between macOS and Windows against the same VS Code version (structural).

**Verification:**
- CI integration tests green on both platforms.
- Ported solutions doc committed under `docs/solutions/best-practices/`.

---

### - [ ] Unit 5: Stable-selector field population

**Goal:** Both platforms populate `StableSelectors` wherever the OS/app exposes them. `resolve_element` prefers `identifier` when non-empty; falls back to the existing `(pid, role, name, bounds_hash)` fingerprint. `RefEntry.identifier` carries the selector across snapshots. Tests require known controls with explicit IDs to preserve them; real apps may omit identifiers. P2-O8.

**Requirements:** R8

**Dependencies:** Unit 1 (types), Unit 3 (Windows adapter present), Unit 2 (FFI regen)

**Files:**
- Modify: `crates/macos/src/tree/element.rs` → 2-way split into `element.rs` (core `AXElement` safety + `fetch_node_attrs` batch) + `element_selectors.rs` (new selector readers). File is at 404 L; splitting enforces the 400-LOC rule before adding reads. (Document-review refinement — Context & Research previously mentioned a 3-way split with `element_attrs.rs`; the authoritative split is 2-way.)
- Create: `crates/macos/src/tree/element_selectors.rs` (readers for `kAXIdentifierAttribute`, `kAXSubroleAttribute`, `kAXRoleDescriptionAttribute`, `kAXPlaceholderValueAttribute`, `kAXDOMIdentifierAttribute`, `kAXDOMClassListAttribute`)
- Modify: `crates/macos/src/tree/builder.rs` (populate `StableSelectors` during node construction)
- Modify: `crates/windows/src/tree/element.rs` (readers for `AutomationId`, `LocalizedControlType`, `HelpText`, plus WebView2 `HtmlId` / `HtmlClass` via `UIA_HtmlIdProperty` / `UIA_HtmlClassProperty`)
- Modify: `crates/windows/src/tree/builder.rs` (populate)
- Modify: `crates/core/src/ref_alloc.rs` (populate `RefEntry.identifier` when `StableSelectors.identifier` is non-empty)
- Modify: `crates/macos/src/tree/resolve.rs` (prefer-identifier: if `entry.identifier.is_some()`, search by `AXIdentifier` match first; fall back to `(pid, role, name, bounds_hash)`)
- Modify: `crates/windows/src/tree/resolve.rs` (same, via `AutomationId`)
- Test: `crates/macos/tests/it_selectors.rs` (target: a test harness `.app` that exports `accessibilityIdentifier` on known buttons; assert those controls carry `identifier` and ordinary controls without IDs omit it)
- Test: `crates/windows/tests/it_selectors.rs` (target: Calculator; assert the `=` button carries `AutomationId = "equalButton"`)
- Test: `crates/core/src/ref_alloc.rs` (unit: `RefEntry.identifier` preserved through allocator)
- Test: `tests/integration/stale_ref_rate.rs` (regression: against a fixture of 100 Electron/localized elements, resolver success rate with identifier preference ≥ baseline + 20 pp; captures the "measurably drops" metric in R8)

**Approach:**
- macOS: `AXUIElementCopyMultipleAttributeValues` request already batches reads; extend its attribute list with the 6 selector attributes. No extra round-trip.
- Windows: `UICacheRequest` already batches; add the 6 selector properties to the cache request. No extra round-trip.
- Resolver prefers: if `entry.identifier.is_some()`, walk the tree and match by `identifier` (O(n) per level but typically matches in first 3 levels). Only if identifier lookup fails, fall back to bounds-hash fingerprint. Matches learning 4 — identifier is an **optional** fingerprint, never mandatory.
- `dom_classes` on Windows requires `UIA_HtmlClassProperty` which is WebView2-only; returns empty Vec on non-Chromium apps. This is expected.

**Patterns to follow:**
- `crates/macos/src/tree/element.rs:fetch_node_attrs` — mirror batching shape for the new attributes.
- Learning 1 — define `SelectorReadConfig` in `crates/core` so macOS / Windows / (future) Linux readers share shape.
- Learning 4 — tri-state UTF-8 at FFI; invalid bytes → `INVALID_ARGS`.

**Test scenarios:**
- **Happy path (macOS):** A test-harness "Save" button has `subrole = "AXConfirmButton"` and `identifier = "save"` when the app exposes it.
- **Happy path (Windows):** Calculator's "=" button has `identifier = "equalButton"` and `role_description = "button"`.
- **Happy path:** `snapshot --app Calculator` JSON includes the `identifier` field on the `=` button; roundtrip through refmap preserves it.
- **Edge case:** element without an accessibility identifier emits no `identifier` field (skip_serializing_if).
- **Edge case:** WebView2 element in Edge has `dom_id` and `dom_classes` populated; non-WebView2 element in Notepad has neither.
- **Edge case:** localized app — same button carries the same `identifier` on English and German system locales (verifies identifier is locale-stable).
- **Integration:** `resolve_element` for a button whose `name` changed (e.g. Chrome tab title) but whose `identifier` is stable → resolves successfully; before this unit, returned `STALE_REF`.
- **Integration:** STALE_REF regression against a 100-element fixture: measured resolution success rate is at least +20 pp with identifier preference (the "measurably drops" metric in R8; baseline captured in a fixture generated once before this unit).

**Verification:**
- Both platform CI integration tests green.
- `tests/integration/stale_ref_rate.rs` asserts the +20 pp metric.

---

### - [ ] Unit 6: Action variant implementations — `LongPress`, `ShowMenu`, `WindowRaise`, `Cancel`

**Goal:** Implement the four cross-platform "simple" new Action variants on both platforms. `SelectRange` and `InsertAtCaret` wait for Unit 8 (require text infrastructure). `DeliverFiles` + `ForceClick` ship in Unit 12 (require heavier platform-specific scaffolding). Subset of P2-O9.

**Requirements:** R9 (subset)

**Dependencies:** Unit 1 (variants declared), Unit 3 (Windows adapter present)

**Pre-work (first sub-PR before any arm additions — deepening-pass addition):** `crates/macos/src/actions/chain_steps.rs` is **already at 407 LOC** (over the 400 cap today) and `crates/macos/src/actions/dispatch.rs` at 349 LOC will breach adding 4 arms. Before this unit's action-logic sub-PRs, land a refactor sub-PR that splits:
- `chain_steps.rs` → `chain_steps.rs` (step definitions) + `chain_steps_extra.rs` (the overflow) OR along responsibility seams; target each ≤350 LOC.
- `dispatch.rs` → `dispatch.rs` (primary dispatch) + `dispatch_variants.rs` (the 4 new Phase 2 arms + future U12 arms); target each ≤350 LOC.
This refactor is behavior-preserving (reorganizes internal module structure only, no public API change). U12 inherits the split, so its 2 additional arms (`DeliverFiles`, `ForceClick`) land in `dispatch_variants.rs` cleanly.

**Files:**
- Modify: `crates/macos/src/actions/chain_steps.rs` (split pre-work — ≤350 LOC each side)
- Create: `crates/macos/src/actions/chain_steps_extra.rs` (split pre-work)
- Modify: `crates/macos/src/actions/dispatch.rs` (split pre-work; then arms for `LongPress`, `ShowMenu`, `WindowRaise`, `Cancel` land in `dispatch_variants.rs`)
- Create: `crates/macos/src/actions/dispatch_variants.rs` (new arms here — uses `ActionDispatchConfig` from core)
- Modify: `crates/windows/src/actions/dispatch.rs` + optional `dispatch_variants.rs` split if needed
- Create: `crates/core/src/commands/long_press.rs`, `show_menu.rs`, `window_raise.rs`, `cancel.rs`
- Modify: `crates/core/src/commands/mod.rs`, `src/cli.rs`, `src/cli_args.rs`, `src/dispatch.rs`
- Test: `crates/macos/tests/it_actions_new.rs`
- Test: `crates/windows/tests/it_actions_new.rs`
- Test: `crates/macos/src/actions/` (smoke test after split — all existing behavior preserved)

**Approach:**
- `LongPress { duration_ms }`: macOS `CGEventCreateMouseEvent(MouseDown)` + sleep + `MouseUp` at element's `bounds.midpoint()`. Windows `SendInput` MOUSEDOWN + sleep + MOUSEUP. Duration clamped to `[50, 10_000]` ms with `InvalidArgs` outside.
- `ShowMenu`: macOS `AXPerformAction(kAXShowMenuAction)` if supported by the element; fallback to right-click. Windows `ExpandCollapsePattern.Expand` if available; fallback to `SendInput` right-click.
- `WindowRaise`: macOS `AXUIElementSetAttributeValue(kAXRaisedAttribute, true)` + `AXUIElementPerformAction(kAXRaiseAction)`. Windows `SetForegroundWindow(HWND)` + `SetWindowPos(HWND_TOP)`. This is an explicit focus/window command, not a fallback; its command policy must permit focus steal and tests assert the side effect is intentional.
- `Cancel`: macOS `AXPerformAction(kAXCancelAction)`. Windows `WindowPattern.Close` on dialog, or `InvokePattern.Invoke` on the cancel button if detectable; fallback synthesizes Escape.

**Patterns to follow:**
- Existing `crates/macos/src/actions/dispatch.rs` match arms for `Click`, `Toggle`, `Expand`.
- `crates/core/src/commands/click.rs` as command-file shape (14 L).

**Test scenarios:**
- **Happy path:** `long-press @e<ref> --duration 500` on a macOS button fires MouseDown then MouseUp 500 ms apart; element's `pressed` state observed transiently via probe.
- **Happy path:** `show-menu @e<ref>` on a Finder file opens the context menu; subsequent `list-surfaces` shows `menu` surface.
- **Happy path:** `window-raise` brings a background window to the front (verify `list-windows --focused-only` before/after).
- **Happy path:** `cancel` on a macOS Save dialog dismisses it; dialog disappears from `list-surfaces`.
- **Edge case:** `long-press` with `duration=0` returns `InvalidArgs`.
- **Edge case:** `show-menu` on an element that lacks context-menu support falls back to right-click; integration test verifies a menu appears or an `ActionNotSupported` is returned cleanly.
- **Error path:** `cancel` on a non-dialog element returns `ActionNotSupported`.
- **Integration:** cross-platform JSON shape identical for all four commands.

**Verification:**
- Both platforms' CI green for `it_actions_new.rs`.

---

### - [ ] Unit 7: `watch_element` — event subscription with push notifications

**Goal:** `watch --event <kind> --ref @e<id> --timeout <ms>` returns events within 500 ms of a programmatic change. P2-O11. Replaces the polling in `system/wait.rs` for element existence when event-driven path is available.

**Requirements:** R11

**Dependencies:** Unit 1 (types: `EventKind`, `ElementEvent`, trait method stub), Unit 3 (Windows adapter present).

**Ordering note (review-refined):** Unit 7's **opening spike PR** (validation-only, no shipped code) must land **before** Unit 1 sub-PR 1h so that `watch_element` trait signature + threading model are informed by validated behavior, not guessed. Sequence: U1 sub-PRs 1a→1g land → U7 spike lands → U1 sub-PR 1h lands (trait method signatures informed by spike) → U7 implementation proceeds.

**Files:**
- Modify: `crates/core/src/adapter.rs` (confirm `watch_element(handle, events, timeout) → Result<Vec<ElementEvent>, AdapterError>` signature; stub landed in Unit 1)
- Create: `crates/macos/src/events/{mod,observer,runloop_worker}.rs` (AXObserver + CFRunLoop on a dedicated worker thread)
- Create: `crates/windows/src/events/{mod,handler,mta_worker}.rs` (UIA event handler on a dedicated MTA thread)
- Create: `crates/core/src/commands/watch.rs`
- Modify: `src/cli.rs`, `src/cli_args.rs`, `src/dispatch.rs` (`wait --event <kind>` flag, with `--event` repeatable)
- Modify: `crates/core/src/commands/wait.rs` (when `--event` present, dispatch to new watch command rather than the polling path)
- Test: `crates/macos/tests/it_watch.rs` (value-changed, focus-changed, menu-opened scenarios against TextEdit / Finder)
- Test: `crates/windows/tests/it_watch.rs` (value-changed, focus-changed against Notepad / Settings)
- Test: `crates/core/src/commands/wait.rs` (unit: ref-format validation separate from event-kind validation)

**Approach:**
- **Opening spike (research-directed):** validate the **asymmetric** threading model on both platforms:
  - **macOS:** confirm that a **main-thread `CFRunLoop`** (the CLI's main thread runs `CFRunLoopRunInMode` during `watch_element`) supports `AXObserver` attach + event delivery on the same thread. Research Topic A: every production AX consumer (AXSwift, Hammerspoon, Phoenix) binds `AXObserverGetRunLoopSource` to `CFRunLoopGetMain()` — off-main attachment is a trap, not a supported configuration. Validate against Finder, TextEdit, VS Code on macOS 14/15.
  - **Windows:** confirm that a worker thread on `CoInitializeEx(MTA)` hosts `AddAutomationEventHandler` / `AddPropertyChangedEventHandler` with cross-thread event delivery via mpsc to the main thread. Per Microsoft's 2025 UIA threading doc this is the supported pattern. Validate against Notepad, Settings, VS Code.
  - Spike also validates **(a)** callback-panic → stop path (`catch_unwind` around the AX/UIA callback emits a synthetic `ElementEvent::CallbackPanic` and exits cleanly) and **(b)** hard-join timeout = `2 × user_timeout_ms` returning `ErrorCode::Internal`.
  - Spike PR lands **before** Unit 1 sub-PR 1h so trait signatures reflect validated behavior.
- **Trait signature:** `watch_element(&self, entry: &RefEntry, spec: &WatchSpec) -> Result<Vec<ElementEvent>, AdapterError>`. Takes `&RefEntry`, not `&NativeHandle`. Each platform's implementation resolves the element on whichever thread owns its accessibility state.
- **Asymmetric threading model (research-refined — Apple and Microsoft prescribe different patterns):**
  - **macOS — main-thread `AXObserver`.** Research Topic A confirmed all AX functions are main-thread-only. Implementation:
    1. Main thread: resolve `RefEntry` → `AXUIElementRef`, `AXObserverCreate(pid, callback)`, `AXObserverAddNotification` for each subscribed kind, `CFRunLoopAddSource(CFRunLoopGetMain(), AXObserverGetRunLoopSource(observer), kCFRunLoopDefaultMode)`.
    2. Main thread: spawn a tiny **signal thread** whose only job is `thread::sleep(timeout_duration)` + `CFRunLoopStop(main_loop)`.
    3. Main thread: `CFRunLoopRunInMode(kCFRunLoopDefaultMode, remaining_secs, false)` — blocks until timeout OR `CFRunLoopStop`.
    4. Callback (main thread): appends to a `RefCell<Vec<ElementEvent>>` local to `watch_element`; no mpsc needed — callback and consumer are the same thread.
    5. Teardown: `AXObserverRemoveNotification` + `CFRelease(observer)`; signal thread joins cleanly (either via its sleep expiring or a cancellation flag).
  - **Windows — worker-thread MTA handler.** UIA supports cross-thread event delivery:
    1. Main thread: resolve `RefEntry` → `IUIAutomationElement` on the main-thread MTA, extract `(pid, automation_id, runtime_id)` for the worker.
    2. Main thread: spawn worker thread → worker calls `CoInitializeEx(NULL, COINIT_MULTITHREADED)`, creates its own `IUIAutomation` client, re-resolves the element by fingerprint on its thread (apartment-affine handles do not cross thread boundaries per Research Topic 4).
    3. Worker: installs `AddAutomationEventHandler` (and friends) with handlers stored as `Arc<Handler>` so the handler object outlives the final dispatched callback (Research Topic 4 — post-remove barrier pattern).
    4. Worker: handler pushes `ElementEvent` into `mpsc::Sender` wrapped in `catch_unwind`.
    5. Main thread: `recv_timeout(Duration::from_millis(timeout_ms))` on the receiver.
    6. Teardown: main calls `PostThreadMessage(worker_tid, WM_QUIT)`; worker calls `RemoveAutomationEventHandler` + no-op UIA call (post-remove barrier drains in-flight callbacks) + `CoUninitialize`. Hard-join within `2 × timeout_ms`.
- **No shared `EventWorker` trait.** The two platforms are categorically different (main-thread CFRunLoop vs worker MTA). Inlining is simpler than abstracting over incompatible shapes. Revisit at Phase 3 when AT-SPI dbus joins; likely Linux is also asymmetric, and the three-way shape may or may not unify.
- Public API is **synchronous**: `watch_element` returns `Vec<ElementEvent>` or `TIMEOUT` error.
- **`WaitStrategy` enum in `commands/wait.rs`** (learning 1 refinement): introduce `enum WaitStrategy { EventDriven(WatchSpec), Polling(PollSpec) }` that `execute()` matches over. The existing `wait --element` / `--window` / `--text` / `--menu` / `--notification` routes construct `Polling(...)`; the new `wait --event` route constructs `EventDriven(...)`. This prevents future flags (e.g., `--until-visible`) from adding yet another branch.
- Error taxonomy (learning 2): ref syntax error → `INVALID_ARGS`; ref valid but element gone → `STALE_REF`; no events within timeout → `TIMEOUT`; observer couldn't attach → `ACTION_FAILED` with `platform_detail` naming the AX/COM error; > 32 subscriptions → `RESOURCE_EXHAUSTED`; worker hard-join timeout → `INTERNAL` with platform detail.
- **Secure-field redaction in `ElementEvent.attr_snapshot` (review-added — security):** before pushing an `ElementEvent` into the mpsc channel, the worker inspects the source element. If `AXSubrole == AXSecureTextField` (macOS) or `UIA_IsPasswordProperty == true` (Windows), the `value` field on the carried `AccessibilityNode` is replaced with `<redacted>`. This prevents `watch --event value-changed` from streaming the literal keystrokes as a user types into a password field.
- **Concurrency cap (review-added):** a process-wide `AtomicUsize` counts in-flight `watch_element` calls. Cap default `32`; beyond that, return `RESOURCE_EXHAUSTED` with suggestion "Close other watch sessions or raise AGENT_DESKTOP_MAX_WATCHES". Per-call MTA apartment creation on Windows costs ~5-20ms cold; unbounded fan-out from FFI consumers can hit COM rate limits. Cap configurable via env var.

**Execution note:** Open with the spike PR (no shipped code) before implementation units.

**Technical design (research-refined — asymmetric by platform):**

```
macOS — main-thread AXObserver (research Topic A):
============================================
main thread                              signal thread
-----------                              -------------
watch_element(entry, spec)
  │
  AXObserverCreate + AddNotification
  │
  CFRunLoopAddSource(main_loop, ...)
  │
  ├── spawn ────────────────────────►   sleep(timeout_ms)
  │                                     CFRunLoopStop(main_loop)
  │
  CFRunLoopRunInMode(default, t, false)
  │
  (callback on same thread appends to local Vec<ElementEvent>)
  │
  return Vec<ElementEvent>


Windows — worker-thread MTA handler (research Topic 4):
============================================
main thread                              worker thread
-----------                              -------------
watch_element(entry, spec)
  │
  ├── spawn ────────────────────────►   CoInitializeEx(MTA)
  │                                     IUIAutomation::new() on this thread
  │                                     re-resolve element by fingerprint
  │                                     AddAutomationEventHandler(Arc<Handler>)
  │                                     (handler:)
  │←────── mpsc::Sender ────────────    catch_unwind(|| tx.send(ElementEvent))
  │
  recv_timeout(Duration::from_ms(t))
  │
  PostThreadMessage(worker_tid, WM_QUIT) ─►  RemoveAutomationEventHandler
  │                                          post-remove barrier (no-op UIA call)
  │                                          CoUninitialize
  │                                          thread::exit
  │
  hard-join (2 * timeout_ms) or Internal
  │
  return Vec<ElementEvent>
```

**Patterns to follow:**
- `crates/macos/src/system/wait.rs` — current polling path that this unit replaces for element-existence scenarios.
- Learning 2 — separate error codes by failure mode; boundary-node pattern for capped subscription counts (max 32 active subscriptions per call; beyond that → `ResourceExhausted` with suggestion).

**Test scenarios:**
- **Happy path (macOS):** Start `watch --event value-changed --ref @e<textfield> --timeout 2000`; separately, set the field's value via `set-value`; the watch command returns a single `ValueChanged` event within 500 ms (R11 metric).
- **Happy path (Windows):** Same against Notepad.
- **Happy path:** Multi-event subscription — `--event value-changed --event focus-changed` receives both kinds.
- **Edge case:** `--event unknown-kind` → `InvalidArgs` with the list of accepted kinds.
- **Edge case:** ref has bad syntax (`not-a-ref`) → `InvalidArgs` (not `StaleRef`) per learning 2.
- **Edge case:** ref valid but element gone → `StaleRef` with snapshot-refresh suggestion.
- **Error path:** timeout reached with no events → `Timeout` (not `Ok(vec![])`). The contract: the command returns `Ok` only when events fired.
- **Edge case:** > 32 subscriptions → `ResourceExhausted` with "max 32 kinds per call" suggestion.
- **Integration:** worker thread joins cleanly; `std::thread::available_parallelism()` usage does not leak threads across 100 sequential `watch_element` calls (regression harness).
- **Integration:** the legacy polling path in `system/wait.rs` still works for non-event scenarios (`wait --element` without `--event`).

**Verification:**
- Spike PR lands first (separately reviewable — no shipped code); implementation PR cites it.
- Both platforms' CI green; 500 ms latency target met in the metric assertion.

---

### - [ ] Unit 8: Text range primitives

**Goal:** `text get-selection`, `text select-range`, `text insert-at-caret`, `text at-offset` on both platforms. Enables `Action::SelectRange` and `Action::InsertAtCaret` dispatch arms added in Unit 1. P2-O12.

**Requirements:** R12

**Dependencies:** Unit 1 (types), Unit 3 (Windows adapter present)

**Files:**
- Create: `crates/macos/src/text/{mod,selection,range,param_attrs}.rs` (parameterized attribute helpers: `AXStringForRangeParameterizedAttribute`, `AXBoundsForRangeParameterizedAttribute`, `AXRangeForLineParameterizedAttribute`; `AXValueCreate(kAXValueCFRangeType)`)
- Create: `crates/windows/src/text/{mod,text_pattern,range}.rs` (`TextPattern.GetSelection`, `TextRange.Select`, `TextRange.Move`, `TextRange.GetText`, `TextRange.GetBoundingRectangles`)
- Create: `crates/core/src/commands/text_get_selection.rs`, `text_select_range.rs`, `text_insert_at_caret.rs`, `text_at_offset.rs`
- Modify: `src/cli.rs`, `src/cli_args.rs`, `src/dispatch.rs` (top-level `text` subcommand with four sub-subcommands)
- Modify: `crates/macos/src/actions/dispatch.rs` + `crates/windows/src/actions/dispatch.rs` (`Action::SelectRange` → `set_text_selection`; `Action::InsertAtCaret` → `insert_text_at_caret`)
- Test: `crates/macos/tests/it_text.rs` (TextEdit: select-range + get-selection roundtrip; insert-at-caret advances caret)
- Test: `crates/windows/tests/it_text.rs` (Notepad: same)

**Approach:**
- Use **UTF-16 code units** for `TextRange { start, length }` consistently across both platforms (matches AX `CFRange` and UIA `TextRange` native conventions). Document this at the public-contract boundary with a one-line doc comment on `TextRange`.
- macOS `get_text_selection`: read `kAXSelectedTextRangeAttribute` → `CFRange`; decode to `TextRange`.
- macOS `set_text_selection`: `AXValueCreate(kAXValueCFRangeType, &CFRange)` → `AXUIElementSetAttributeValue(kAXSelectedTextRangeAttribute, value)`.
- macOS `insert_text_at_caret`: get selection → `AXUIElementSetAttributeValue(kAXSelectedTextAttribute, string)` replaces selection with string (and advances caret to end).
- **Password-field gate (review-added — security, both platforms):** before any text-read operation (`get_text_selection`, `get_text_at`), the adapter checks the target element for password-field status. macOS: `AXSubrole == AXSecureTextField` → return `ActionNotSupported` with suggestion "Text reads are blocked on password fields; use set-value to write without reading." Windows: `UIA_IsPasswordProperty == true` OR `ControlType == PasswordEdit` → same error. **Writes (`set_text_selection`, `insert_text_at_caret`) are allowed** on password fields because typing a credential is a legitimate agent operation. The `TextRangeConfig.check_password_field: bool` (default `true`) gates this; explicit `--allow-password-read` flag opts out with a stern CLI warning, reserved for security research scenarios.
- Windows `get_text_selection`: `TextPattern.GetSelection` → first `TextRange.GetBoundingRectangles` → start/length relative to `TextPattern.DocumentRange.GetText(-1)` UTF-16 length.
- Windows `set_text_selection`: `TextPattern.DocumentRange.Clone().Move(TextUnit_Character, start).MoveEndpointByRange(End, start+length)` → `Select`.
- Windows `insert_text_at_caret`: `set_text_selection` to caret-only range → `SendInput` typing the text (UIA has no direct "insert at caret" primitive); alternatively `TextPattern.SupportedTextSelection` fallback.

**Patterns to follow:**
- Learning 2 — `INVALID_ARGS` for bad range bounds (negative, overflow); `STALE_REF` for gone elements; `ACTION_FAILED` for elements that don't support text patterns.

**Test scenarios:**
- **Happy path:** TextEdit with "hello world" → `text select-range @e<field> 0 5` + `text get-selection @e<field>` returns `{ start: 0, length: 5, text: "hello" }`.
- **Happy path:** `text insert-at-caret @e<field> "bye "` inserts at current caret; field value reflects it; caret advanced by 4.
- **Happy path:** `text at-offset @e<field> 0 5` returns `"hello"` without mutating selection.
- **Edge case:** `start` > document length → `InvalidArgs` with the document length in the suggestion.
- **Edge case:** `length = 0` selects nothing (caret-only); valid.
- **Edge case:** element without `TextPattern` on Windows or `AXValue` string type on macOS → `ActionNotSupported`.
- **Edge case:** Unicode surrogate pairs ("👋") — UTF-16 `length = 2` for one emoji; test verifies boundary alignment.
- **Error path:** element gone between snapshot and call → `StaleRef`.
- **Integration:** cross-platform roundtrip — same test against a multi-line field on both platforms produces identical JSON with identical offsets.

**Verification:**
- Both platform CI integration tests green; roundtrip assertions equal on macOS and Windows.

---

### - [ ] Unit 9: Modern screenshot backends

**Goal:** Modern default on both platforms where the OS/session supports it. macOS via ScreenCaptureKit (`screencapturekit 1.5`); Windows via `windows-capture 1.5.4`. Legacy fallback (`--screenshot-backend legacy`) keeps the Phase 1 subprocess on macOS and `PrintWindow` on Windows. Cold latency <50 ms on supported modern paths. P2-O13.

**Requirements:** R13

**Dependencies:** Unit 1 (`ScreenshotBackend` type, trait method `get_screenshot_with_backend`), Unit 3 (Windows adapter; legacy screenshot already landed there)

**Files:**
- Modify: `Cargo.toml` (target-gated: macOS gains `screencapturekit = "1.5"` (research-refined — **published crates.io** canonical crate as of Q1 2026, doom-fish fork is the maintained successor and is published there; no git-SHA pin needed) + `objc2 = "0.6"` with `Foundation`/`AppKit` features; Windows gains `windows-capture = "1.5.4"` (latest stable, pins `windows = "0.62.2"` matching our workspace))
- Create: `docs/security/screenshot-deps-audit-2026-04-18.md` (**review-added** — documents (a) why `screencapturekit 1.5` (crates.io) is trusted, (b) the weekly CI audit that compares `Cargo.lock` hash against last known good, (c) the screen-capture code paths reviewed for exfiltration risk. Committed before Unit 9 merge.)
- Modify: `.github/workflows/ci.yml` (**review-added** — weekly cron job: `cargo update --dry-run -p windows-capture -p screencapturekit` reports transitive dep shifts; alerts on unexpected movement.)
- Modify: `crates/macos/Cargo.toml`, `crates/windows/Cargo.toml` (mirror deps)
- Modify: `crates/macos/src/system/screenshot.rs` (split into `modern.rs` + `legacy.rs` if LOC pressure; wire `ScreenshotBackend::Modern` to ScreenCaptureKit `SCShareableContent.windows` + `SCScreenshotManager.captureImage`)
- Modify: `crates/windows/src/system/screenshot.rs` (split; modern via `windows-capture` + `Direct3D11CaptureFramePool`)
- Modify: `crates/core/src/commands/screenshot.rs` (wire `--screenshot-backend <modern|legacy>` flag; default Modern)
- Modify: `src/cli_args.rs` (add flag)
- Modify: `crates/macos/src/adapter.rs` + `crates/windows/src/adapter.rs` (implement `get_screenshot_with_backend`; `screenshot` legacy trait method routes to `get_screenshot_with_backend(ScreenshotBackend::Legacy)`)
- Create: `.github/workflows/ci.yml` grep guard step — fails if `objc2::` import appears outside `crates/macos/src/system/screenshot.rs` / `crates/macos/src/system/permissions.rs` (R6 mitigation per origin risks)
- Test: `crates/macos/tests/it_screenshot_modern.rs` (SSIM compare modern vs legacy against Finder window; latency assertion <50 ms)
- Test: `crates/windows/tests/it_screenshot_modern.rs` (same against Notepad)

**Approach:**
- **Spike first** per origin risk R3: verify `windows-capture 1.5.4` API + `Capture::start` + frame-callback signature compiles with `windows 0.62.2` before merging. Verify `screencapturekit 1.5` `SCScreenshotManager.captureImage(contentFilter:config:)` signature on macOS 14/15.
- macOS modern: `SCShareableContent.current(excludingDesktopWindows: false, onScreenWindowsOnly: true)` → filter by `CGWindowID` → `SCContentFilter(desktopIndependentWindow: window)` → `SCScreenshotManager.captureImage(contentFilter:config:)`. Config: `SCStreamConfiguration` with `width`/`height` = window bounds; `pixelFormat = BGRA`. Result: `CGImage` → `CGImageDestinationCreateWithData` → PNG buffer.
- Windows modern: first check WGC/session support (`GraphicsCaptureSession::IsSupported` where available plus a real `CreateFromWindowHandle` probe). Then `GraphicsCaptureItem.CreateFromWindowHandle(HWND)` → `Direct3D11CaptureFramePool::Create(device, BGRA8, 2, item.Size)` → attach `FrameArrived` callback; start session; receive one frame; copy to CPU memory via D3D11 `Map`; encode PNG via `image` crate or `windows::Win32::Graphics::Imaging::WIC`.
- Legacy fallback: macOS continues `/usr/sbin/screencapture -R <rect> -t png`; Windows continues `PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)`.
- `--screenshot-backend legacy` exists for WDA-protected windows (e.g. password managers) where modern APIs return a black frame.

**Execution note:** Open with a two-PR spike — one per platform — validating API compilation + single capture against a test app. Commit spike outputs (sample PNGs) in `tests/fixtures/screenshot-*.png` as ground truth for the SSIM regression.

**Patterns to follow:**
- `crates/macos/src/system/screenshot.rs` current structure — keep its file shape, split into modern/legacy submodules.
- Learning 4 — screenshot permission handling (Screen Recording TCC); failure → `PermDenied` with Screen-Recording-specific suggestion (distinct from AX). Pairs with Unit 11.

**Test scenarios:**
- **Happy path (macOS):** `screenshot --app Finder` (Modern) produces a valid PNG; dimensions match Finder window bounds; file opens in Preview.
- **Happy path (Windows):** `screenshot --app Notepad` (Modern) produces a valid PNG via `windows-capture`.
- **Happy path:** `screenshot --app Notepad --screenshot-backend legacy` produces a valid PNG via `PrintWindow`.
- **Integration:** cold latency — `screenshot --app Finder` Modern completes in <50 ms on macOS; <50 ms on Windows when WGC is supported by the runner/session (R13 metric). Measured across 10 runs; median reported.
- **Integration:** SSIM — Modern vs Legacy outputs against the same window match with SSIM ≥ 0.95 (not pixel-identical because color spaces differ, but structurally identical).
- **Edge case:** Modern backend unavailable (pre-macOS 12.3, pre-Win10 1903, Session 0, Server Core, locked/secure desktop, or runner without WGC support) → automatic fall-through to Legacy when possible with a warning in `platform_detail`; otherwise `PLATFORM_NOT_SUPPORTED`.
- **Edge case:** WDA-protected window → Modern returns black frame; user re-runs with `--screenshot-backend legacy`; legacy succeeds.
- **Error path:** denied Screen Recording on macOS → Modern returns `PermDenied` with Screen-Recording-specific suggestion (Unit 11's tri-state makes the distinction from AX possible).
- **Integration:** grep guard CI step — adding `use objc2::…` outside the two allowed files fails CI with a clear message.

**Verification:**
- Both platforms' CI green; latency regression asserted; SSIM assertion green; grep guard active.

---

### - [ ] Unit 10: New surfaces — Toolbar, Spotlight, Dock, MenuBarExtras, Windows shell surfaces

**Goal:** macOS: add `Toolbar`, `Spotlight`, `Dock`, `MenuBarExtras` surfaces; ship tray commands (mirror of U3b). Windows: add structured shell surfaces for `Toolbar`, `Taskbar`, `SystemTray`, `SystemTrayOverflow`, `StartMenu`, `ActionCenter`, and `QuickSettings` where the current Windows build/session exposes them. P2-O14 and P2-O18.

**Requirements:** R14

**Dependencies:** Unit 3 (Windows adapter), Unit 3b (Windows tray commands already merged; this unit backfills macOS)

**Files:**
- Modify: `crates/core/src/adapter.rs` (`SnapshotSurface` enum gains `Toolbar`, `Spotlight`, `Dock`, `MenuBarExtras`, `Taskbar`, `SystemTray`, `SystemTrayOverflow`, `StartMenu`, `ActionCenter`, `QuickSettings`)
- Modify: `crates/macos/src/tree/surfaces.rs` (detect Toolbar via `AXRole == AXToolbar` or `AXUnifiedTitleAndToolbar`; Spotlight via `Spotlight.app` PID; Dock via `Dock.app` PID; MenuBarExtras via `SystemUIServer` + `ControlCenter` + per-app `AXExtrasMenuBar`)
- Modify: `crates/windows/src/tree/surfaces.rs` (Taskbar/SystemTray via `Shell_TrayWnd`, overflow via `NotifyIconOverflowWindow`, Start/ActionCenter/QuickSettings via shell-surface roots in `list_surfaces`; the open-surface and tray command dispatchers already landed in U3b)
- Modify: `crates/macos/src/adapter.rs` (implement `list_tray_items`, `click_tray_item`, `open_tray_menu` against MenuBarExtras items)
- Modify: `crates/core/src/commands/list_surfaces.rs` (no change — the new `SnapshotSurface` variants flow through)
- Modify: `src/cli_args.rs` (snapshot `--surface` accepts the new kinds)
- Test: `crates/macos/tests/it_surfaces.rs` (`snapshot --surface toolbar` on Safari; `list-surfaces` includes Spotlight / Dock / MenuBarExtras pids)
- Test: `crates/windows/tests/it_surfaces.rs` (`snapshot --surface toolbar` on Edge; `list-surfaces` includes present Windows shell surfaces)

**Approach:**
- Spotlight surface: launch Spotlight via `Cmd+Space` equivalent if needed (Spotlight process is ephemeral); `NSRunningApplication.applications(withBundleIdentifier: "com.apple.Spotlight")` → pid → AX subtree.
- Dock surface: `NSRunningApplication.applications(withBundleIdentifier: "com.apple.dock")` → pid → AX subtree; children are `AXDockItem` entries.
- MenuBarExtras surface: union of `SystemUIServer`, `ControlCenter`, and each running app's `AXExtrasMenuBar`.
- Windows shell surfaces: walk `Shell_TrayWnd` UIA children + `NotifyIconOverflowWindow`; surface roots opened by `open-system-surface` are snapshotted immediately through their UIA roots.

**Patterns to follow:**
- `crates/macos/src/tree/surfaces.rs` Menu / Sheet / Popover detection.
- Learning 1 — `SurfaceDetectionConfig` shared shape so macOS and Windows surface detectors don't diverge by accident.

**Test scenarios:**
- **Happy path:** `snapshot --surface toolbar --app Safari` returns toolbar children (back, forward, URL field, bookmarks) with refs.
- **Happy path:** `list-surfaces` on macOS includes `{type: "spotlight"}`, `{type: "dock", item_count: N}`, `{type: "menubar_extras", item_count: M}`.
- **Happy path:** `list-surfaces` on Windows includes present shell surfaces such as `{type: "system_tray", item_count: K}`, `{type: "taskbar", item_count: M}`, `{type: "start_menu"}`, `{type: "action_center"}`, and `{type: "quick_settings"}`.
- **Happy path:** `snapshot --surface toolbar --app Edge` on Windows returns Edge's toolbar children.
- **Edge case:** Spotlight not running → Spotlight surface missing from `list-surfaces` (not an error).
- **Edge case:** Dock hidden (auto-hide enabled) → Dock surface present but `item_count` reflects visible items.

**Verification:**
- Both platforms' CI green for surfaces.

---

### - [ ] Unit 11: Permission tri-state on macOS

**Goal:** `check_permissions` on macOS returns the tri-state struct introduced in Unit 1, fully populated for Accessibility, Screen Recording, and Automation. `permissions` command output shows all three. Errors distinguish `PermDenied` (AX), `AutomationPermissionDenied`, and Screen-Recording-denied screenshot failures. P2-O17.

**Requirements:** R17

**Dependencies:** Unit 1 (tri-state types, new error codes), Unit 9 (Modern screenshot uses Screen Recording)

**Files:**
- Modify: `crates/macos/src/system/permissions.rs` (extend from the current ~55 L; add `CGPreflightScreenCaptureAccess` + `CGRequestScreenCaptureAccess` reads; add `AEDeterminePermissionToAutomateTarget` probe)
- Modify: `crates/macos/src/adapter.rs` (`check_permissions` returns tri-state)
- Modify: `crates/macos/src/system/screenshot.rs` (Modern backend returns `PermDenied` with Screen-Recording-specific suggestion when `CGPreflightScreenCaptureAccess` is false)
- Modify: `crates/macos/src/system/app_ops.rs` (`close_app` that requires AppleEvents returns `AutomationPermissionDenied` when `AEDeterminePermissionToAutomateTarget` says denied)
- Modify: `crates/core/src/commands/permissions.rs` (output shape matches tri-state)
- Test: `crates/macos/tests/it_permissions.rs` (on a macOS runner without Screen Recording granted: `screenshot` returns `PermDenied` with the right suggestion)
- Test: `crates/macos/tests/it_permissions_automation.rs` (verify `AutomationPermissionDenied` fires on `close-app` against an app lacking Automation grant)

**Approach:**
- `CGPreflightScreenCaptureAccess` returns bool; `CGRequestScreenCaptureAccess` triggers the TCC prompt; expose the prompt behind an explicit `--request-permission` flag — do not auto-prompt during `check_permissions`.
- **TTY-gate on `--request-permission` (review-added — security):** the flag is permitted **only** when `atty::is(atty::Stream::Stdin)` returns true. Automated agent pipelines (stdin piped / not a TTY) reject with `ErrorCode::InvalidArgs` and suggestion "Permission requests require interactive stdin; run the tool manually once to grant." Prevents agent-consent-bypass where a malicious task payload instructs an automated agent to grant itself OS permissions.
- `AEDeterminePermissionToAutomateTarget(const AEAddressDesc *target, AEEventClass theAEEventClass, AEEventID theAEEventID, Boolean askUserIfNeeded)` — call with `askUserIfNeeded = false` during probing; the returned `OSStatus` maps to the tri-state (`noErr` → `Granted`, `errAEEventNotPermitted` → `Denied`, `procNotFound` → `Unknown`).
- Tri-state output JSON:
  ```json
  {
    "accessibility": "Granted",
    "screen_recording": { "state": "denied", "suggestion": "Open System Settings > Privacy & Security > Screen & System Audio Recording" },
    "automation": { "state": "not_determined" }
  }
  ```
- Error mapping: screenshot failure with Screen Recording denied → `ErrorCode::PermDenied` + suggestion naming **Screen Recording** specifically (not the generic AX suggestion). `close_app` failure with Automation denied → `ErrorCode::AutomationPermissionDenied`.

**Patterns to follow:**
- Existing `crates/macos/src/system/permissions.rs:check_permissions` shape.
- Learning 4 — tri-state matches the optional-fingerprint pattern.

**Test scenarios:**
- **Happy path:** `permissions` on a fully-granted macOS runner returns all three = `Granted`.
- **Happy path:** on a Screen-Recording-denied runner, `permissions` returns `screen_recording.state = "denied"` with the Screen Recording suggestion; `accessibility.state` remains `Granted`.
- **Happy path:** `screenshot --app Finder` with Screen Recording denied returns `PermDenied` + Screen-Recording suggestion (not the generic AX one).
- **Happy path:** `close-app --app TargetApp` with Automation denied returns `AutomationPermissionDenied`.
- **Edge case:** `Unknown` state → the `permissions` JSON still has the field (no `suggestion` key in that branch).
- **Edge case:** `--request-permission` flag triggers `CGRequestScreenCaptureAccess`; subsequent `permissions` reflects the user's grant.
- **Integration:** cross-platform parity — the `permissions` command on Windows returns `accessibility = "Granted"` (always granted on Windows after UAC elevation) + `Unknown` for the other two.

**Verification:**
- macOS CI green; Windows CI exercises the JSON shape (tri-state across both platforms).

---

### - [ ] Unit 12: `DeliverFiles` + `ForceClick` per-platform (formerly "FileDrop")

**Goal:** `DeliverFiles(Vec<PathBuf>)` (renamed from `FileDrop` per research) and `ForceClick` on macOS and Windows. Linux returns `ActionNotSupported` on `ForceClick` (Phase 3 non-goal). Balance of P2-O9.

**Rename rationale (research-driven):** `NSDraggingSession` is NOT headless-compatible on macOS — it requires an `NSApplication` event loop, a window-server mouse-event stream, and foreground activation. Research Topic 1 confirmed this is not a gap to work around but a categorical incompatibility. The headless-first invariant (§Headless-First Invariant) forbids `NSDraggingSession` as a primary path. The action is renamed `DeliverFiles` to reflect the contract: "files appear at the destination" — the mechanism is platform-appropriate, not always a literal drag event.

**Requirements:** R9 (remaining)

**Dependencies:** Unit 1 (variants declared — `FileDrop` enum variant renamed to `DeliverFiles` in 1d), Unit 6 (simple variants landed)

**macOS delivery strategy (research Topic 1 — four-tier fallback, headless-first):**
1. **Tier 1 — app-native URL scheme (per-app registry).** A small table in `crates/macos/src/actions/deliver_files.rs` maps known bundle IDs to their URL scheme / CLI protocols (e.g., `com.microsoft.VSCode` → `vscode://file/…`, `com.tinyspeck.slackmacgap` → slack slash-command). Only used when the target app advertises an official protocol; never guessed.
2. **Tier 2 — `NSWorkspace.open(urls:withApplicationAt:configuration:completionHandler:)` with `activates: false` (PRIMARY path for most apps).** Headless, non-focus-stealing, Apple-supported since macOS 10.15. Files open in the target app. Works for Finder, Preview, TextEdit, Safari, most native apps, and Electron apps that implement standard file-open protocol.
3. **Tier 3 — `NSPasteboard.general` write of `public.file-url` + `CGEventPostToPid(cmd+v)` to target PID.** Simulates paste-into-focused-view. Requires the target app to have a focused paste-capable view. Used for Electron text editors, note apps. Preserves the system clipboard by saving/restoring.
4. **Tier 4 — `osascript -e 'tell application "X" to open {POSIX file "/path"}'`.** AppleScript Apple Events. Requires Automation TCC grant (Unit 11 tri-state permission covers this). Fallback for apps with no URL scheme and no pasteboard response.

Selection logic: `deliver_files.rs` tries tiers in order, returns on first success. Tier choice logged via `tracing::debug!` for the user to understand which path fired. `NSDraggingSession` is **NEVER** invoked.

**Windows delivery strategy:**
- **Tier 1 — app-native URI / command handoff.** Use documented app URI handlers or shell verbs when the target app advertises them. Never guess private protocols.
- **Tier 2 — filesystem destination.** For Explorer windows and known filesystem destinations, use `IFileOperation::CopyItems` / `MoveItems` as the semantic path. This is the default for folders because it avoids fake drag state entirely.
- **Tier 3 — `CF_HDROP` clipboard paste.** Populate clipboard file-drop format, target the destination with an explicit paste action, then restore the prior clipboard. This is used only when the destination accepts paste semantics.
- **Tier 4 — `IDataObject + DoDragDrop` spike/fallback.** Implement only after a spike proves it can be driven without violating focus/cursor policy for the target class. It is never the default headless path.

**ForceClick strategy unchanged from origin plan** — macOS via `kCGMouseEventPressure` on `CGEventCreateMouseEvent`; Windows via `SendInput` with pen-input flags. Linux → `ActionNotSupported`.

**Opening spikes (first sub-PRs of Unit 12):** validate macOS Tier 2 (`NSWorkspace.open` with `activates: false`) against 3 targets — Finder folder, VS Code workspace, Safari tab — confirming (a) files land correctly, (b) no focus steal, (c) agent-desktop process remains headless. Validate Windows Tiers 2/3 against Explorer, Notepad/wordpad-style file-open targets, and one Electron app. Only after that spike can Tier 4 OLE drag be attempted for targets that require drag semantics.

**Path-validation rules (review-added — security, both platforms):**
- All input paths pass through `std::fs::canonicalize` before any pasteboard / `IDataObject` write.
- Default: reject paths outside `$HOME` and `/tmp` (macOS), outside `%USERPROFILE%` and `%TEMP%` (Windows).
- Reject paths containing `..` segments **after** canonicalization (defense in depth).
- `--allow-system-paths` flag overrides the scope check with a CLI warning; never enabled by default, never settable via env var (explicit intent required).
- Empty path list → `InvalidArgs`.
- Non-existent source path → `InvalidArgs` with the missing path in the message.
- Threat model documented in `docs/solutions/logic-errors/deliver-files-path-safety-2026-04-18.md` (created in U14 docs pass).

**Files:**
- Create: `crates/macos/src/actions/deliver_files.rs` (4-tier headless fallback from §Unit 12 strategy; `NSWorkspace.open` is Tier 2 primary path — no `NSDraggingSession`)
- Create: `crates/macos/src/actions/deliver_files_registry.rs` (per-app URL scheme table for Tier 1)
- Create: `crates/macos/src/actions/force_click.rs` (`kCGMouseEventPressure = 1.0` + `kCGEventMouseSubtypeTabletPoint` on `CGEventCreateMouseEvent`)
- Create: `crates/windows/src/actions/deliver_files.rs` (URI/shell delivery, `IFileOperation`, `CF_HDROP` clipboard paste, and policy-gated OLE fallback after spike)
- Create: `crates/windows/src/actions/force_click.rs` (`SendInput` with `PEN_FLAGS_BARREL` pen input flags)
- Modify: `crates/macos/src/actions/dispatch.rs` + `crates/windows/src/actions/dispatch.rs` (add arms)
- Modify: `crates/linux/src/adapter.rs` (ensure `execute_action` returns `ActionNotSupported` for `ForceClick` with documented-platform-divergence message — this is the stub; real implementation is Phase 3)
- Modify: `crates/core/src/commands/` (commands for `deliver-files`, `force-click`)
- Modify: `src/cli.rs`, `src/cli_args.rs`, `src/dispatch.rs`
- Test: `crates/macos/tests/it_deliver_files.rs` (2 files delivered to a Finder folder via Tier 2; verify files appear; verify NO focus steal on agent-desktop process)
- Test: `crates/macos/tests/it_force_click.rs` (force-click a word in TextEdit; verify the definition popover appears via `list-surfaces`)
- Test: `crates/windows/tests/it_deliver_files.rs` (files delivered to an Explorer folder via `IFileOperation`; verify files appear; verify NO focus steal; clipboard paste path restores clipboard; OLE fallback tests are gated behind explicit policy/spike fixtures)
- Test: `crates/windows/tests/it_force_click.rs` (pen input into a test app; verify pressure was delivered)

**Approach:**
- macOS `ForceClick`: `CGEventCreateMouseEvent(..., kCGEventMouseDown, kCGMouseButtonLeft, ...)` → `CGEventSetIntegerValueField(event, kCGMouseEventPressure, 1)` → `CGEventSetIntegerValueField(event, kCGMouseEventSubtype, kCGEventMouseSubtypeTabletPoint)` → post; then `MouseUp`.
- macOS `DeliverFiles`: see the **4-tier headless fallback strategy** above (URL scheme → `NSWorkspace.open` with `activates: false` → pasteboard + `Cmd-V` → AppleScript). `NSDraggingSession` / `NSFilePromiseProvider` are explicitly rejected — they require foreground activation + `NSApp.run` event loop, which violate the Headless-First Invariant.
- Windows `ForceClick`: `SendInput` with `INPUT_PEN` structures; set `PEN_FLAGS_BARREL` and `pressure = 1024` (max).
- Windows `DeliverFiles`: implement the semantic tiers in order: URI/shell handoff, `IFileOperation` for Explorer/filesystem destinations, `CF_HDROP` clipboard paste with save/restore, and only then policy-gated `IDataObject + DoDragDrop` for target classes whose integration spike proves no unintended focus/cursor side effects.

**Execution note:** `DeliverFiles` macOS Tier-2 and Windows Tier-2/Tier-3 spikes are mandatory. OLE drag is treated as a fallback requiring proof, not assumed safe by architecture.

**Patterns to follow:**
- `crates/macos/src/actions/extras.rs` as the template for action extras that bypass the standard AX dispatch.
- Learning 1 — `ActionDispatchConfig` so platform-specific quirks don't leak into the core trait.

**Test scenarios:**
- **Happy path (macOS):** `deliver-files @e<folder_ref> /tmp/a.txt /tmp/b.txt` produces a.txt and b.txt inside the folder (verified via `ls`); no focus change observed on active frontmost app.
- **Happy path (Windows):** same against Explorer.
- **Happy path (macOS):** `force-click @e<word_ref>` in TextEdit → definition popover appears in `list-surfaces`.
- **Edge case:** `deliver-files` with non-existent source path → `InvalidArgs` with the missing path in the message.
- **Edge case:** `force-click` on an element that doesn't respond to pressure (e.g. a regular button) → action succeeds but no side-effect; document this as expected behavior.
- **Edge case:** Linux `force-click` → `ActionNotSupported` with platform-divergence note in `platform_detail`.
- **Error path:** `deliver-files` with 0 paths → `InvalidArgs`.
- **Error path:** destination element not a drop target → `ActionFailed` with suggestion.

**Verification:**
- Both platform CI integration tests green. Linux stub returns `ActionNotSupported` cleanly.

---

### - [ ] Unit 13: Windows CI + release pipeline (x86_64 + aarch64)

**Goal:** `windows-latest` runs build, clippy, unit, contract, and non-interactive tests on every PR; interactive UIA/shell tests run on a labeled Windows desktop runner; Windows CLI binaries (x86_64, aarch64) attached to Phase 2 release; npm `postinstall` picks up Windows branches. P2-O6, P2-O7.

**Requirements:** R6, R7

**Dependencies:** None hard (runs parallel to U3…U12); must land **before** Unit 3's integration tests can validate on a Windows runner. In practice: a draft PR for this unit opens alongside Unit 3.

**Files:**
- Modify: `.github/workflows/ci.yml` (add `test-windows` job: `runs-on: windows-latest`; mirrors the `test` job — clippy, `cargo test --lib --workspace`, `cargo build --profile ci`, `cargo build --profile release-ffi -p agent-desktop-ffi`, `cargo tree` isolation check)
- Modify: `.github/workflows/ci.yml` (add optional `test-windows-interactive` job guarded by label/runner availability: `runs-on: [self-hosted, windows, desktop, interactive]`; runs UIA/shell integration tests for Explorer, Start, taskbar, Action Center, Quick Settings, Notepad, and screenshot WGC. Hosted `windows-latest` must not be required to expose an unlocked Explorer desktop.)
- Modify: `.github/workflows/ci.yml` (the existing `test` job stays on `macos-latest`; add a matrix if sharing logic becomes tidy)
- Modify: `.github/workflows/release.yml` (CLI `build` matrix gains `x86_64-pc-windows-msvc` + `aarch64-pc-windows-msvc` on `windows-latest`. **aarch64 archive suffixed `-experimental`** (review-refined — shipping a build-only artifact with no test coverage deserves explicit labeling): `agent-desktop-v0.2.0-aarch64-pc-windows-msvc-experimental.zip`. FFI matrix gains `aarch64-pc-windows-msvc` row (same suffix). Archive format `zip` for Windows matches existing FFI Windows row. **MSVC ARM64 toolchain setup step added** (review-refined — cross-compile from windows-latest x64 host requires the MSVC ARM64 build tools component, not present in the default Desktop C++ workload): step invokes `Install-Module VSSetup` / `vs_installer modify` to add `Microsoft.VisualStudio.Component.VC.Tools.ARM64` before `cargo build --target aarch64-pc-windows-msvc`, then sources `vcvarsall.bat arm64` via `VsDevCmd.bat`.)
- Modify: `rust-toolchain.toml` (`targets` list gains `x86_64-pc-windows-msvc`; `aarch64-pc-windows-msvc` stays opt-in via `rustup target add` in the workflow step)
- Modify: `npm/scripts/postinstall.js` (add `win32-x64` branch selecting `agent-desktop-v<version>-x86_64-pc-windows-msvc.zip`. **aarch64 Windows falls back to x86_64 auto-selection** (review-refined — arm64 archive is flagged experimental; npm does not auto-select it until a runner validates). Explicit opt-in via `AGENT_DESKTOP_PREFER_ARM64=1` env var for users who want to test the experimental build. Decompress via native node `yauzl` (smaller footprint than `adm-zip`, no optionalDeps).)
- Modify: `npm/package.json` (optional: declare `os`/`cpu` matrix includes win32)
- Modify: `npm/bin/agent-desktop` (wrapper picks `.exe` on Windows)
- Test: `.github/workflows/ci.yml` — Windows CI green on the reference branch before Unit 3 lands.
- Test: `npm/scripts/postinstall.test.js` (mock win32 + arm64 environments; assert correct archive name selected)

**Approach:**
- Clone the existing `test` job into a new `test-windows` job targeting `windows-latest` for compile/unit/contract coverage. Add a separate interactive desktop job for UIA/shell tests; it is required before marking Windows adapter GA but is not assumed to be available on GitHub-hosted runners. Skip binary-size check on Windows initially (MSVC toolchain + dynamic CRT produces slightly larger binaries; either raise the cap to 20 MB on Windows or skip — recommend skip with a TODO for Phase 5 binary-size work).
- The ARM64 Windows CLI binary is **build-only** for now. When GitHub Actions promotes the ARM runner to GA, a follow-up PR wires testing; the release job publishes the binary regardless.
- `npm/scripts/postinstall.js` mirrors existing macOS logic; adds `win32` branches.

**Patterns to follow:**
- Existing `release.yml` FFI matrix already includes `x86_64-pc-windows-msvc` — copy its structure for the CLI matrix row.
- Existing `ci.yml` `test` job shape.

**Test scenarios:**
- **Happy path:** PR with a trivial `crates/core/src/` change triggers `test-windows` and it passes.
- **Happy path:** PR with Windows shell changes triggers `test-windows-interactive` when the label/runner is available; otherwise the PR must include mocked/unit coverage and a manual evidence artifact before merge.
- **Happy path:** release-please creating a v0.2.0 tag publishes x86_64 + aarch64 Windows CLI `.zip` archives alongside existing macOS and Linux artifacts; checksum file includes them.
- **Happy path:** `npm install @lahfir/agent-desktop` on a Windows x64 machine downloads the right archive and the binary runs.
- **Integration:** `cargo tree -p agent-desktop-core` isolation check runs on Windows and passes.
- **Edge case:** Windows-specific test failure surfaces clearly in the PR check list.
- **Error path:** missing `windows-latest` runner minutes → workflow fails fast with a non-retriable error (document runbook in Unit 14 docs).

**Verification:**
- Windows CI job green on a reference PR.
- Release workflow dry-run against a test tag produces the expected artifact set.

---

### - [ ] Unit 14: Documentation + skills + `phases.md` sync

**Goal:** Ship Phase 2 docs. `skills/agent-desktop-windows/SKILL.md` created; core skill updated to three-platform; `skills/agent-desktop-ffi/` reflects `ad_abi_version` + `ad_set_log_callback`; README platform table + Windows install + permissions; `phases.md` stale-reference cleanup. Skill Maintenance Addendum compliance. Final unit; runs after U1..U13 are merged.

**Requirements:** Skill Maintenance Addendum; release-readiness

**Dependencies:** U1..U13 (all implementation complete)

**Files:**
- Create: `skills/agent-desktop-windows/SKILL.md` (sibling to macOS content)
- Create: `skills/agent-desktop-windows/references/uia.md` (UIA control types, patterns, pattern-first dispatch order)
- Create: `skills/agent-desktop-windows/references/windows-permissions.md` (UAC, UIA access, integrity-level mismatches, WGC/session support, Focus Assist / notification listener permission, and unsupported locked/secure desktop behavior)
- Create: `skills/agent-desktop-windows/references/windows-shell-surfaces.md` (Start menu/search, taskbar, system tray/overflow, Action Center, Quick Settings, virtual desktop detection, mixed-DPI coordinate caveats, and when to use `open-system-surface`)
- Create: `skills/agent-desktop-windows/references/chromium.md` (Windows-flavored version of `electron-compat.md` + `--force-electron-a11y`)
- Modify: `skills/agent-desktop/SKILL.md` (three-platform command surface; link to macOS + Windows sibling skills; note tray + notifications on Windows)
- Modify: `skills/agent-desktop/references/commands-*.md` (add `watch`, `text *`, `open-system-surface`, `list-tray-items`, `click-tray-item`, `open-tray-menu`, `long-press`, `show-menu`, `window-raise`, `cancel`, `force-click`, `deliver-files`; note new flags `--force-electron-a11y`, `--screenshot-backend`, `--event`, `--request-permission`)
- Modify: `skills/agent-desktop-ffi/SKILL.md` (document `ad_abi_version()`, `ad_set_log_callback`, registry-driven wrapper surface, `AD_ABI_VERSION_MAJOR`)
- Modify: `skills/agent-desktop-ffi/references/ownership.md` + `references/threading.md` (note worker-thread use in `watch_element`; `ad_set_log_callback` lifetime)
- Create: `crates/ffi/README.md` (pre-1.0 FFI policy statement per KD17)
- Modify: `README.md` (platform support matrix; Windows install; permission pre-flight; link to Windows skill)
- Modify: `docs/phases.md` (strike "Gap Analysis — 2026-04-17 Research at the bottom" references; fix stale Windows dependency pins to `0.62.2`; remove `Watch(WatchSpec)` from P2-O9 enumeration; update shipped command count; add Phase 2 "Shipped" status block once v0.2.0 tags)
- Modify: `CHANGELOG.md` (release-please will generate, but add a human-curated summary of v0.2.0 breaking changes: `ErrorCode` now `#[non_exhaustive]`, `PermissionReport` tri-state, `ad_abi_version` exported + policy)
- Create: `docs/migrations/0.1-to-0.2.md` (consumer migration guide: JSON schema changes, FFI ABI changes, new error codes)
- Create: `docs/solutions/best-practices/electron-compat-cross-platform-2026-04-18.md` (document-review refinement — moved here from Unit 4; ports private-memory `electron-compat.md` so Windows + future Linux contributors see the depth-skip rules)
- Keep backward-compat shim: `scripts/update-ffi-header.sh` (thin wrapper) alongside the new `scripts/update-ffi.sh` so existing references, skills, and contributor muscle memory don't break (document-review refinement)
- Modify: `skills-lock.json` (register `agent-desktop-windows`)

**Approach:**
- Use `/skill-creator` for every SKILL.md touched (per MEMORY.md Skill Maintenance rule).
- `phases.md` surgical edits — do not rewrite narrative, just fix the stale claims and add a "Shipped 2026-MM-DD" line to Phase 2 §Status.
- Consumer migration guide covers: `ErrorCode` additions (consumers parsing SCREAMING_SNAKE_CASE must accept unknown codes since `#[non_exhaustive]`), `PermissionReport` JSON shape change, any renames.

**Patterns to follow:**
- Existing `skills/agent-desktop/SKILL.md` structure and references layout.
- Existing release-please-generated CHANGELOG conventions.

**Test scenarios:**
- **Test expectation: none** — documentation-only unit; validated via manual review and by running `skills/` publish dry-run (`clawhub sync --root skills/ --dry-run`).
- **Verification gates (not tests):**
  - `scripts/update-ffi-header.sh` shows no drift.
  - `skills-lock.json` references the new Windows skill.
  - `docs/phases.md` grep for `"Gap Analysis — 2026-04-17 Research at the bottom"` returns zero matches.
  - `docs/phases.md` grep for `windows = "0.58"` returns zero matches.
  - `README.md` platform table includes Windows x64 + arm64.

**Verification:**
- Manual review of all skill files and README on the release PR.
- CI green; skills publish step dry-run succeeds.

---

## System-Wide Impact

- **Interaction graph:** `watch_element` introduces worker threads inside the adapter — breaks the single-threaded Phase 1 assumption. All shared state between callbacks and the main thread goes through `mpsc` channels (no shared mutable state). **Deepening-pass decision:** the `watch_element` trait signature takes `&RefEntry` (not `&NativeHandle`), so the worker re-resolves the element on its own thread and the `NativeHandle` `PhantomData<*const ()>` `!Send`/`!Sync` invariant is preserved intact. The existing `unsafe impl Send for NativeHandle` / `unsafe impl Sync for NativeHandle` at `crates/core/src/adapter.rs:93-94` (Phase 1 single-threaded justification) does NOT need reassessment in Phase 2.
- **Error propagation:** `ErrorCode` gains 4 new variants (`#[non_exhaustive]` makes this forward-compatible for consumers that match exhaustively and accept a default arm). Consumers of the FFI `AdResult` enum see 4 new discriminants; the `const _: () = assert!(…)` parity gate keeps them in lockstep. New errors (`AutomationPermissionDenied`, Screen-Recording-specific `PermDenied`) flow through existing `AdapterError::with_suggestion` / `with_platform_detail` helpers.
- **State lifecycle risks:** `DeliverFiles` macOS uses `NSWorkspace.open(activates: false)` (no delegate lifetime to manage — async completion via closure, `Retained<T>` wrapper for any callback handles). On Windows, `IFileOperation`, clipboard save/restore, and optional `IDataObject` fallback each need explicit lifetime ownership; `IDataObject` must span `DoDragDrop` only inside the policy-gated fallback. `watch_element` on macOS uses main-thread `CFRunLoopRunInMode` (no worker thread lifecycle); on Windows the worker thread is hard-joined inside `watch_element` (no leak). `ad_set_log_callback` installs a global `OnceCell`; removing the callback is not supported (second call errors).
- **API surface parity:** Every new trait method ships with a `not_supported()` default implementation so Linux (Phase 3) compiles without changes. The Linux adapter explicitly keeps `ForceClick` returning `ActionNotSupported` as permanent divergence.
- **Integration coverage:** Cross-platform JSON identity fixture (`tests/fixtures/calculator.*.json`, `tests/fixtures/vscode.*.json`) ensures structural parity is checked, not just individual unit behavior. `watch_element` + `ad_set_log_callback` + `ad_abi_version` each require their own FFI integration harness.
- **Unchanged invariants:**
  - Ref format `@e{n}` and refmap file location at `~/.agent-desktop/last_refmap.json` (0o600).
  - JSON envelope shape (`{version, ok, command, data, error}`).
  - Binary size limit 15 MB for the CLI (Windows CI may skip pending Phase 5 size work; documented explicitly).
  - Core isolation — `cargo tree -p agent-desktop-core` still free of platform crates (CI checks on macOS AND Windows).
  - Single-command-per-file + 400-LOC rule (Unit 5 splits `element.rs` because it's at the cap).
  - No `unwrap()` in non-test code; zero-warning clippy.

## Risks & Dependencies

| ID | Risk | Mitigation | Owner Unit |
|----|------|------------|------------|
| R1 | Calendar slip — 15 units is a large PR count | Parallelize where dependency graph allows; each unit is independently reviewable; honest calendar-months sizing up front | — |
| R2 | `watch_element` architecture unproven (AXObserver off main thread, UIA handler lifetime) | Unit 7 opens with a named spike against three apps per platform; spike outcome gates the rest of U7 | U7 |
| R3 | `windows-capture` newer majors may move the API surface | Pin `windows-capture = "=1.5.4"`; verify API compiles against 1.5.4 before merging U9 spike PR; re-evaluate newer majors post-Phase 2 | U9 |
| R4 | Registry migration (U2) affects every existing command — high blast radius | Migrate one command category at a time (observation → interaction → system → clipboard → notifications → batch); each sub-PR keeps `cargo test --workspace` green | U2 |
| R5 | MSRV bump to 1.82 may break downstream consumers on pinned toolchains | Release notes flag MSRV; pre-1.0 explicitly unstable; acceptable | U1 |
| R6 | `objc2` introduction spreads beyond screenshot/permissions | CI grep guard: `rg "objc2::" crates/macos/src/ \| grep -v "system/(screenshot\|permissions).rs" && exit 1` (landed in U9) | U9 |
| R7 | FFI ABI churn across 17 objectives needs one coordinated `ad_abi_version()` bump | Unit 1 exports `ad_abi_version()` as **sub-PR 1a** with anchor `MAJOR = 1`; the first struct-layout-breaking sub-PR (1g — `PermissionReport` tri-state) atomically bumps to `MAJOR = 2`; every subsequent FFI-affecting change in U1–U12 asserts via CI grep that the version has incremented when its own changes touch `crates/ffi/src/error.rs` or `crates/ffi/src/generated/` | U1 |
| R8 | Windows runner flakiness against real apps | Split Windows CI: `test-windows-unit` (blocks merge) + `test-windows-ui` (nightly + pre-release); Unit 13 implements the split when GitHub Actions runner minutes warrant | U13 |
| R9 | `ForceClick` on Linux has no native path (Phase 3 gap) | Documented; Linux adapter returns `ActionNotSupported` — legitimate per-platform divergence, not a bug | U12 |
| R10 | Cross-compile-from-macOS iteration cost for Windows-heavy phase | Windows CI runs on every PR touching `crates/windows/` or cross-platform trait methods; if iteration cost exceeds 2000 CI minutes/PR-cycle, spin up a local Windows VM | U3 / U13 |
| R11 | `AccessibilityNode` field explosion — nested `StableSelectors` preserves wire shape | Planning picked `#[serde(flatten)]` on an inline `StableSelectors` with per-field `skip_serializing_if` (see §Open Questions §Resolved); JSON wire shape preserved | U1 |
| R12 | `phases.md` references a "Gap Analysis — 2026-04-17 Research" section that doesn't exist | Fixed in U14 alongside other phases.md syncs | U14 |
| R13 | Codegen mechanism for FFI wrappers | **Resolved by research** — Unit 2 uses `build.rs` filesystem enumeration of `crates/core/src/commands/*.rs` (deterministic, cdylib-safe, zero linker magic). `inventory`/`linkme`/`xtask` all rejected per Research Topic B. Spike PR still opens Unit 2 to validate codegen on `click` alone before migrating 52 more. | U2 |
| R14 | Electron rules currently live in private memory (`electron-compat.md`) | Unit 4 ports those rules to `docs/solutions/best-practices/electron-compat-cross-platform-2026-04-18.md` as the first task | U4 |
| R15 | `SendInput` on Windows doesn't honor `blocked_combos` by itself — the safety check is ours | Unit 3's input/keyboard.rs calls `is_blocked(combo, adapter.blocked_combos())` before dispatching; test covers this | U3 |
| R16 | `UIA_HtmlIdProperty` / `UIA_HtmlClassProperty` only work on WebView2 elements | Unit 5 returns empty Vec for non-WebView2 nodes; documented in selectors reference | U5 |
| R17 | **FFI wrapper rename gap** — renaming a command without updating its filename or the `descriptor()` function could slip past wrapper drift check | `crates/core/tests/cli_registry_parity.rs` cross-checks that every clap subcommand name has a matching `CommandDescriptor` and vice versa; `build.rs` filesystem enumeration makes drift impossible because the file listing IS the source of truth | U2 |
| R18 | **400-LOC breaches compound in Phase 2** — `actions/chain_steps.rs` already at 407 L today; `actions/dispatch.rs` at 349 L will breach adding 4 arms in U6 | Unit 6 opens with a behavior-preserving refactor sub-PR that pre-splits both files; documented in U6 pre-work | U6 |
| R19 | **`NativeHandle` `!Send`/`!Sync` invariant vs Unit 7 worker thread** | Deepening pass resolves by passing `&RefEntry` (not `&NativeHandle`) to `watch_element`; worker re-resolves on its own thread, preserving the invariant | U1 / U7 |
| R20 | **Unit 7 callback panic → hung worker thread leak** (critical for long-running FFI consumers) | U7 spike validates `catch_unwind` wrapper around AX/UIA callback; hard-join timeout in `watch_element` itself (`2 × timeout_ms`) with `Internal` error on refuse-to-exit | U7 |
| R21 | **`AD_ABI_VERSION_MAJOR` bump ordering race** — sub-PR 1g is the only actual ABI-breaking sub-PR in Unit 1 (struct-layout change on `PermissionReport`); 1b adds `#[non_exhaustive]` without variant changes (non-breaking) and 1c adds variants under `#[non_exhaustive]` (breaking for C consumers per adversarial review — see P0 findings) | Document-review refinement: `ad_abi_version()` ships at sub-PR 1a as anchor `MAJOR = 1`; sub-PR 1g atomically bumps to `MAJOR = 2` with the layout change; CI grep asserts the bump happens on every sub-PR that touches layout files | U1 |

## Alternative Approaches Considered

1. **Sub-phase split (2a/2b).** Rejected in brainstorm §D1 before this plan opened. Accumulates unstated deferrals; the 15-unit PR decomposition managed via this plan is lower-risk than a sub-release cut.
2. **`Option<StableSelectors>` with `#[serde(flatten)]` wrapper.** Rejected during planning (§Open Questions §Resolved). The inline struct with per-field `skip_serializing_if` preserves JSON wire shape with no behavioral change; the Option wrapper adds a layer of indirection that doesn't buy anything.
3. **Persistent modifier-state file for key-down/key-up.** Rejected. Adds state to a stateless CLI; the whole-combo safety check in `blocked_combos()` is sufficient because every blocked combo includes at least one non-modifier key.
4. **Shared bounded worker-thread pool for U7 and U9.** Rejected. Different lifetimes (observer loops vs single-shot captures); different threading models (MTA vs no specific apartment). Revisit with Phase 4 daemon.
5. **`windows 0.58` as originally named in the early draft.** Rejected. Patched to `windows 0.62.2` in origin §D15 because `windows-capture 1.5.x` requires 0.62+ and the workspace pins the current compatible release.
6. **`inventory` OR `linkme` for the command registry.** BOTH rejected (research-driven). Research Topic B found neither survives link-GC reliably across ld64, ld-prime, GNU ld, lld, MSVC link.exe for cdylib consumers. Replaced with `build.rs` filesystem enumeration — deterministic, no linker magic.
7. **Auto-commit regenerated FFI wrappers from `build.rs` back into `crates/ffi/src/generated/`.** Rejected per learning 3 — auto-heal masks CI drift. The committed copy is the ABI contract; `cargo xtask gen-ffi` refreshes on demand.
7a. **Codegen via `build.rs` macro expansion inside `crates/ffi/src/lib.rs`.** Rejected (deepening pass) — has no committed artifact to drift-check against, which violates learning 3 and means rename-detection / shape-drift caught only at runtime. `xtask` path (chosen) keeps the committed file as the ABI contract.
7b. **Codegen via a proc-macro crate that expands `deterministic registry iteration` at the `ffi` crate's top level.** Rejected for the same reason as 7a — no committed artifact.
8. **`screencapturekit 0.3` from the early draft.** Rejected for `1.5` per origin §D15 — the 0.3 series is older and lacks `SCScreenshotManager` stable shape.
9. **Widen `WindowInfo.pid` to `i64` for Windows DWORD safety.** Rejected per KD3 — breaks JSON and FFI ABI for zero practical benefit; narrow-at-boundary is adequate.

## Phased Delivery

Calendar-month ceilings (hard, not commitments — if breached, invoke the Deferral Order below).

### Phase 0 — v0.1.14 prep release (review-added, ~1 week)

Before Phase 2 proper opens, ship a minimal non-breaking release that gives downstream consumers FFI-stability primitives without a major bump. Scope:
- `#[non_exhaustive]` on `ErrorCode` (no variant additions yet)
- `ad_abi_version()` exported returning `AD_ABI_VERSION_MAJOR = 1` (anchor Phase 1 value)
- `ad_init(expected_major: u32) -> AdResult` enforced version-negotiation handshake
- `AD_RESULT_UNKNOWN = -99` sentinel exported
- `crates/ffi/README.md` documenting the pre-1.0 FFI policy (KD17)

Consumers adopt these and update their integration tests before v0.2.0 breaks the PermissionReport layout. Splits the 4-way break (MSRV + non_exhaustive + tri-state + ABI bump) into 2+2, giving adopters breathing room.

**Gate:** v0.1.14 tag pushed; FFI consumers on the main integration list (if any) have adopted `ad_init()`.

### Phase A — Foundation (~4 weeks) — Unit 1 + Unit 2 + Unit 2.5 + Unit 2.6
U1 lands across 10 sub-PRs (1a–1j). U2 (registry migration, single-concern) across 6 category sub-PRs. U2.5 (`ad_set_log_callback` with redaction) and U2.6 (Phase 1.5 FFI backfill) land as small follow-ups. U7's opening spike PR lands between sub-PRs 1g and 1h (so trait signatures are informed by spike outcome). **Gate:** `cargo test --workspace` green on macOS; `ad_init` handshake working; parity const assertion intact; registry count matches clap command enumeration under all CI profiles.

### Phase B — Windows foundation (~5 weeks) — Unit 3 + Unit 3a + Unit 3b; Unit 13 in parallel
U3 implements the Windows adapter across 4 sub-PRs (tree / actions+input / clipboard+app+window / permissions+screenshot-legacy). U3a and U3b follow. U13 opens in parallel to keep Windows CI running against U3 draft branches. **Gate:** Windows CI green; cross-platform JSON fixture test (R2) meets the review-refined metric (jaccard ≥ 0.85, identifier equality, ±15% ref count).

### Phase C — Feature parity (~6 weeks) — Units 4–12, parallelizable
Units 4, 5, 6, 7, 8, 9, 10, 11, 12 all depend only on U1, U2, U3. They open in parallel worktrees.

**Merge order (review-refined — value-dense first, not risk-cheapest first):**
1. **U5** (stable-selector fields) — R8 directly benefits agent loops
2. **U7** (watch_element) — R11 replaces 100ms polling with sub-500ms push
3. **U4** (Windows Electron compat) — R15 unlocks VS Code / Slack / Cursor
4. **U8** (text range primitives) — R12 enables in-place editing flows
5. **U6** (action variants LongPress/ShowMenu/WindowRaise/Cancel) — completes the Action surface
6. **U11** (permission tri-state on macOS) — R17 unblocks Modern screenshot
7. **U9** (modern screenshot) — R13, depends on U11 for Screen Recording probe
8. **U10** (new surfaces) — R14, polish
9. **U12** (DeliverFiles + ForceClick) — remaining Action variants, highest-risk spike

**Gate:** every P2-O* metric green in CI integration tests.

### Phase D — Release (~1 week) — Unit 14
U14 updates docs, strikes stale phases.md references, creates the Windows skill, publishes the FFI pre-1.0 policy, ports electron-compat to docs/solutions. release-please cuts v0.2.0. **Gate:** tag pushed; GitHub Release artifacts include all Windows + macOS CLI and FFI archives (aarch64 Windows marked `-experimental`); npm publish succeeds.

### Total ceiling: **16 weeks**

### Deferral Order (if ceiling breached)

Defer last-to-first in this order (lowest-impact first):
1. **U10** (new surfaces: Spotlight / Dock / MenuBarExtras / Windows shell surfaces) → v0.2.1
2. **U3b** (Windows tray commands) → v0.2.1
3. **U12** (DeliverFiles + ForceClick) → v0.2.1 or v0.3.0 (DeliverFiles spikes decide)
4. **U4** (Electron compat on Windows) → v0.2.1 — but prefer keeping, since this blocks VS Code / Slack adoption on Windows

R8, R11, R12, R13, R16, R17 are **never deferred** — they are the core agent-value wins of Phase 2.

## Success Metrics

- **R8 metric (review-refined):** Primary — real-app integration: 10-step Slack sidebar navigation on macOS and Windows completes without `STALE_REF`-induced re-snapshot in ≥80% of runs (10 runs minimum). Secondary — `identifier` field populated on ≥70% of interactive nodes across VS Code + Slack + Calculator fixture set. The earlier "+20pp on curated fixture" is retained as a diagnostic, not a gate.
- **R11 metric:** `watch --event value-changed` returns within 500 ms on both platforms.
- **R13 metric:** `screenshot --app Finder` (Modern) cold latency <50 ms median over 10 runs on both platforms.
- **R15 metric:** VS Code snapshot with `--force-electron-a11y` exposes ≥100 refs at default depth on both platforms.
- **R16 metric:** Adding a new command (`hello-world` test fixture) requires only creating one file under `crates/core/src/commands/`; CLI, FFI wrappers, and schemas auto-register; asserted by an integration test in Unit 2.
- **FFI ABI stability:** `ad_abi_version()` returns `AD_ABI_VERSION_MAJOR = 2` from Phase 2 onward; `crates/ffi/README.md` documents the bump policy.
- **Binary size:** macOS CLI release binaries remain under 15 MB; Windows CLI initially unbounded (size check skipped on Windows in Unit 13 with TODO).

## Documentation Plan

- `skills/agent-desktop-windows/SKILL.md` — created in Unit 14 with `/skill-creator`.
- `skills/agent-desktop/SKILL.md` — updated for three-platform support in Unit 14.
- `skills/agent-desktop-ffi/SKILL.md` — `ad_abi_version` + `ad_set_log_callback` noted in Unit 14.
- `crates/ffi/README.md` — new file in Unit 2 / Unit 14 with the pre-1.0 FFI ABI policy.
- `docs/migrations/0.1-to-0.2.md` — created in Unit 14; consumer migration guide.
- `docs/phases.md` — stale-reference cleanup in Unit 14.
- `README.md` — three-platform install + permission pre-flight in Unit 14.
- `docs/solutions/best-practices/electron-compat-cross-platform-2026-04-18.md` — created in Unit 4 (ports private memory).
- `CHANGELOG.md` — human-curated v0.2.0 breaking-change summary in Unit 14 (release-please generates the base).
- `MEMORY.md` — update the user's project memory after v0.2.0 ships (Phase 2 status, Windows adapter, registry migration).

## Operational / Rollout Notes

- **Breaking release:** v0.2.0. Downstream FFI consumers MUST call `ad_abi_version()` at load time and compare against their expected major. Example in `skills/agent-desktop-ffi/references/build-and-link.md`.
- **Toolchain:** MSRV bumps to 1.82. Pre-commit hook and CI cache keys refresh automatically on first PR after the bump.
- **Permissions pre-flight:** a new README section explains Screen Recording TCC grant on macOS before running `screenshot`; Windows documents UIA integrity boundaries, WGC support checks, UAC/elevation mismatch, and shell-surface unsupported cases.
- **Rollback:** v0.1.x remains installable via `npm install @lahfir/agent-desktop@0.1`. ABI breakage means mixing 0.1 CLI with 0.2 FFI is undefined — migration guide says so.
- **Monitoring:** post-release, watch `STALE_REF` rate in field (user-reported) and compare against the Unit 5 baseline fixture. Phase 3 plan deepens with Linux AT-SPI coverage.
- **Follow-up PRs after v0.2.0 ships:**
  - `windows-capture 2.0` upgrade evaluation (pending 2.x stability).
  - GitHub Actions `windows-11-arm` GA → wire aarch64 Windows testing matrix (today: build-only).
  - Binary size budget for Windows CLI (Phase 5 production-readiness work).

## Swarm & Parallelization Strategy

Phase 2 is 16 weeks single-threaded. Most of Phase C and large parts of Phase A can run concurrently via Claude Code agent teams (spawn via Agent tool with `isolation: "worktree"`) or parallel worktrees managed by the user. This section lists what can safely run in parallel and what cannot.

### Hard serial dependencies (DO NOT parallelize)

These must land in strict order — parallelizing them creates merge conflicts on shared types or breaks intermediate builds:

| Dependency | Reason |
|---|---|
| v0.1.14 prep → Phase 2 start | Consumers need `ad_init()` available before ABI breaks |
| U1 sub-PRs 1a → 1b → 1c → 1d → 1e → 1f → 1g | Each sub-PR mutates `crates/core/src/error.rs` or `node.rs` in sequence; parallel edits conflict on the parity `const` assertion and serde derives |
| U7 spike → U1 sub-PR 1h → U7 implementation | Trait signature depends on spike outcome |
| U1 sub-PR 1g → U1 sub-PR 1i | `PermissionReport` layout change must precede the `AD_ABI_VERSION_MAJOR` bump test that reads it |
| U2 → U2.5, U2.6 | `ad_set_log_callback` and backfill are `CommandDescriptor` entries; registry must exist first |
| U2 → U3 | Windows adapter's new commands register via `deterministic registry metadata`; registry must exist |
| U3 → U3a, U3b, U4 | Sub-units extend the Windows adapter surface |
| U11 → U9 | Modern screenshot needs tri-state permission probe to distinguish Screen-Recording-denied from AX-denied |
| All units → U14 | Docs update reflects shipped behavior |

### Parallelizable swarm fan-out points

#### Swarm Point 1 — Phase A (sub-PR 1i onward)

After Unit 1 sub-PR 1g has landed, these can fan out concurrently:

| Worker | Work | Isolation |
|---|---|---|
| A | U1 sub-PR 1h (trait method stubs) | main branch, serial |
| B | U1 sub-PR 1i (`event.rs`, `text_range.rs`, `screenshot_backend.rs`, 4 config structs) | worktree |
| C | U1 sub-PR 1j (CLI arm reservations with `#[clap(hide = true)]`) | worktree |
| D | U7 spike PR (validation-only, `crates/macos/src/events/spike.rs` throwaway) | worktree, **blocks sub-PR 1h** |

Sub-PRs 1h, 1i, 1j can land in any order once 1g is in main, because they touch disjoint files.

#### Swarm Point 2 — Phase A (after U2 merges)

U2.5 and U2.6 are small and independent. Fan out 2 workers:

| Worker | Work | Estimated PR size |
|---|---|---|
| A | U2.5 (`ad_set_log_callback` + redaction layer) | ~400 LOC |
| B | U2.6 (Phase 1.5 FFI backfill: `ad_execute_by_ref` + descriptor confirms) | ~200 LOC |

#### Swarm Point 3 — Phase B (Windows foundation)

U3's 4 sub-PRs serialize internally (tree → actions → clipboard → permissions), but U13 (Windows CI + release pipeline) runs entirely in parallel against U3's draft branches:

| Worker | Work |
|---|---|
| A | U3 tree sub-PR (UITreeWalker, roles map, element wrapper) |
| B | U3 actions sub-PR — **starts after A merges** |
| C | U13 sub-PRs (CI job, release.yml matrix, npm postinstall) — **parallel to A/B/C/D** |
| D | `docs/security/screencapturekit-fork-audit-2026-04-18.md` audit (can start anytime) |

#### Swarm Point 4 — Phase C fan-out (highest parallelism)

After U3 merges, 9 workers can land concurrently. This is the densest parallelization point. **Recommended team composition: 4–6 workers maximum** (not 9 — reviewer bandwidth is the bottleneck).

| Worker | Recommended units | Isolation | Total LOC estimate |
|---|---|---|---|
| A | U5 (stable-selector population) | worktree | ~600 |
| B | U7 (watch_element — spike validated) | worktree | ~1200 |
| C | U4 (Windows Electron compat) | worktree | ~500 |
| D | U8 (text range primitives) | worktree | ~900 |
| E | U6 + U11 (action variants + macOS tri-state permissions) | worktree | ~700 |
| F | U9 (modern screenshot — after E's U11) + U10 (new surfaces) | worktree | ~1000 |

Worker E bundles U6 + U11 because both are small and independent. Worker F starts U9 only after U11 merges (serial dependency per Phase C merge order).

U12 (DeliverFiles + ForceClick) lands **after** F's U9 merges, with its own opening spikes. Treat U12 as single-worker.

#### Swarm Point 5 — Phase D (Documentation)

U14 fans out across skill references and docs:

| Worker | Work |
|---|---|
| A | `skills/agent-desktop-windows/SKILL.md` + references |
| B | `docs/migrations/0.1-to-0.2.md` + CHANGELOG curation |
| C | `docs/phases.md` cleanup + `crates/ffi/README.md` policy doc |
| D | `docs/solutions/best-practices/electron-compat-cross-platform-2026-04-18.md` port |

All four are independent; merge in any order.

### Spawning workers with Claude Code `Agent` tool

```
Agent({
  description: "U5 stable-selector implementation",
  subagent_type: "general-purpose",
  isolation: "worktree",
  prompt: "Implement Unit 5 of docs/plans/2026-04-18-001-feat-phase2-windows-crossplatform-plan.md — stable-selector field population on macOS and Windows. Read the full unit definition including Files, Approach, Patterns to follow, Test scenarios, Verification. Land each of the test scenarios as a test case. Keep crates/macos/src/tree/element.rs under 400 LOC (split into element.rs + element_selectors.rs per the Files list). Return when all tests pass and cargo clippy is clean."
})
```

Spawn 4–6 in one message for true concurrent execution. Each worker operates in an isolated worktree so merge conflicts surface at PR review, not mid-write.

### Swarm anti-patterns (avoid)

- **Do not parallelize sub-PRs within U1.** They all touch `crates/core/src/{error,action,node,adapter,refs}.rs` in overlapping regions; parallel edits conflict on the parity assertion.
- **Do not fan out U3's 4 sub-PRs.** `crates/windows/src/adapter.rs` is the contended file; serialize.
- **Do not start U9 before U11.** Screen Recording permission probe is a dependency.
- **Do not start U12 before U9.** DeliverFiles depends on Modern screenshot's TCC story for shared `objc2` surface area; also U12's spikes need U9's dependency tree.
- **Do not assign two workers to the same unit.** Units are atomic scope; splitting within a unit requires manual coordination that defeats the swarm.

---

## Progressive Commit Checklist

Every box below maps to **one PR-sized commit**. Dependency order is top-to-bottom within each phase. Where `[parallel with: …]` appears, spawn a swarm worker.

### Phase 0 — v0.1.14 prep

- [ ] C0.1: add `#[non_exhaustive]` to `ErrorCode` (no variant additions) — `feat(ffi): prepare error enum for non-exhaustive evolution`
- [ ] C0.2: export `ad_abi_version()` returning 1, add `AD_RESULT_UNKNOWN = -99`, add `ad_init(expected_major)` handshake — `feat(ffi): add abi version + init handshake`
- [ ] C0.3: publish `crates/ffi/README.md` FFI policy — `docs(ffi): document pre-1.0 abi policy`
- [ ] C0.4: release v0.1.14 — `chore: release 0.1.14`

### Phase A — Foundation (Unit 1 + U2 + U2.5 + U2.6)

- [ ] C1.1 (sub-PR 1a): abi_version.rs + ad_init + AD_RESULT_UNKNOWN sentinel (if not in 0.1.14) — serial
- [ ] C1.2 (sub-PR 1b): `#[non_exhaustive]` on `ErrorCode` (if not in 0.1.14) — serial
- [ ] C1.3 (sub-PR 1c): 4 new `ErrorCode` variants + matching `AdResult::Err*` — serial
- [ ] C1.4 (sub-PR 1d): 8 new `Action` variants — serial
- [ ] C1.5 (sub-PR 1e): `StableSelectors` + `#[serde(flatten)]` on `AccessibilityNode` — serial
- [ ] C1.6 (sub-PR 1f): `RefEntry.identifier: Option<String>` — serial
- [ ] C1.7 (sub-PR 1g): `PermissionReport` tri-state + `AD_ABI_VERSION_MAJOR` bumps to 2 atomically — serial, **gates downstream**
- [ ] C1.8 (U7 spike): validate `AXObserver` non-main-thread behavior — `feat(spike): validate ax observer threading` [parallel with: C1.9, C1.10]
- [ ] C1.9 (sub-PR 1h): trait method stubs (signature informed by C1.8) — parallel with C1.10
- [ ] C1.10 (sub-PR 1i): supporting types + 4 shared configs — parallel with C1.9
- [ ] C1.11 (sub-PR 1j): reserve CLI arms with `#[clap(hide = true)]` — parallel with C1.9, C1.10
- [ ] C2.1: U2 registry migration spike on `click` command only — `feat(ffi): registry migration spike (click)` [gates C2.2–C2.7]
- [ ] C2.2: U2 observation category migration — `refactor(ffi): migrate observation commands to registry`
- [ ] C2.3: U2 interaction category migration — `refactor(ffi): migrate interaction commands`
- [ ] C2.4: U2 system category migration — `refactor(ffi): migrate system commands`
- [ ] C2.5: U2 clipboard category migration — `refactor(ffi): migrate clipboard commands`
- [ ] C2.6: U2 notifications category migration — `refactor(ffi): migrate notification commands`
- [ ] C2.7: U2 batch migration + xtask + drift check + link-GC mitigation — `refactor(ffi): complete registry migration with xtask`
- [ ] C2.5.1: U2.5 log_callback + redaction layer — `feat(ffi): ad_set_log_callback with redaction` [parallel with C2.6.1]
- [ ] C2.6.1: U2.6 Phase 1.5 backfill (execute_by_ref + descriptor confirms) — `feat(ffi): backfill phase 1.5 wrappers` [parallel with C2.5.1]

### Phase B — Windows foundation (U3 + U3a + U3b; U13 parallel)

- [ ] C3.1: U3 tree — `feat(windows): uia tree walker + role map`
- [ ] C3.2: U3 actions — `feat(windows): uia action dispatch with smart chain` [after C3.1]
- [ ] C3.3: U3 input — `feat(windows): sendinput keyboard/mouse + win32 clipboard` [after C3.1]
- [ ] C3.4: U3 system — `feat(windows): app lifecycle + window ops + legacy screenshot` [after C3.1]
- [ ] C3.5: U3a Windows notifications — `feat(windows): toast/action-center notifications` [after C3.2]
- [ ] C3.6: U3b Windows shell surfaces + tray — `feat(windows): shell surfaces and system tray commands` [after C3.2, parallel with C3.5]
- [ ] C13.1: U13 Windows CI job — `ci: add windows-latest test job` [parallel with C3.*]
- [ ] C13.2: U13 release matrix + postinstall — `ci: release pipeline for x86_64 + aarch64 windows` [parallel with C3.*]
- [ ] C13.3: U13 MSVC arm64 toolchain setup — `ci: install arm64 msvc build tools` [after C13.2]

### Phase C — Feature parity (U4–U12, heavily parallel)

**Swarm fan-out at this point — spawn 4–6 workers.**

- [ ] C5: U5 stable-selector population — `feat(tree): populate stable selector fields` [parallel with C7, C4, C8, C6, C11]
- [ ] C7: U7 watch_element — `feat(events): watch_element with push notifications` [parallel with C5, C4, C8, C6, C11]
- [ ] C4: U4 Windows Electron compat — `feat(windows): electron/webview2 depth-skip` [parallel with C5, C7, C8, C6, C11]
- [ ] C8: U8 text range primitives — `feat(text): text range primitives with password-field gate` [parallel with C5, C7, C4, C6, C11]
- [ ] C6: U6 action variants (LongPress/ShowMenu/WindowRaise/Cancel) + pre-work chain_steps.rs + dispatch.rs split — `refactor(macos): split actions files; feat(actions): 4 new variants` [parallel with C5, C7, C4, C8, C11]
- [ ] C11: U11 macOS permission tri-state — `feat(macos): screen recording + automation tri-state` [parallel with C5, C7, C4, C8, C6; gates C9]
- [ ] C9.1: U9 screencapturekit fork audit — `docs(security): audit screencapturekit doom-fish fork` [parallel with all Phase C, gates C9.2]
- [ ] C9.2: U9 modern screenshot — `feat(screenshot): modern backend via ScreenCaptureKit + windows-capture` [after C11 + C9.1]
- [ ] C10: U10 new surfaces (Toolbar/Spotlight/Dock/MenuBarExtras + tray commands on macOS) — `feat(surfaces): add toolbar/spotlight/dock/menubar-extras/tray` [parallel with C9.2]
- [ ] C12.1: U12 DeliverFiles macOS/Windows spikes — `feat(spike): validate deliver-files semantic paths` [after C9.2, gates C12.2]
- [ ] C12.2: U12 DeliverFiles + ForceClick implementations — `feat(actions): deliver-files (headless) + force-click with path validation` [after C12.1]

### Phase D — Documentation + Release (U14)

- [ ] C14.1: `skills/agent-desktop-windows/SKILL.md` + references — `docs(skill): add windows skill` [parallel with C14.2–C14.5]
- [ ] C14.2: `docs/migrations/0.1-to-0.2.md` + CHANGELOG curation — `docs: v0.2.0 migration guide` [parallel]
- [ ] C14.3: `docs/phases.md` cleanup + `crates/ffi/README.md` policy updates — `docs: phase 2 hygiene sweep` [parallel]
- [ ] C14.4: port `electron-compat.md` private memory → `docs/solutions/best-practices/` — `docs(solutions): cross-platform electron compat` [parallel]
- [ ] C14.5: `docs/solutions/logic-errors/deliver-files-path-safety-2026-04-18.md` + `docs/solutions/logic-errors/watch-element-thread-safety-2026-04-18.md` — `docs(solutions): phase 2 threat models` [parallel]
- [ ] C14.6: release v0.2.0 — `chore: release 0.2.0`

### Progressive commit discipline

- Each box = one `feat:` / `fix:` / `refactor:` / `docs:` / `ci:` conventional commit, single concern.
- After every box merges to `main`, the preceding deferred tests / CI workflow re-runs and must stay green.
- If a swarm worker's commit would conflict with another worker's in-flight commit, the later-to-open PR rebases — never force-push an earlier PR.
- Sub-PR 1g (the `AD_ABI_VERSION_MAJOR = 1 → 2` bump) is the one-way door; everything after it is v0.2.0.

---

## Sources & References

- **Origin document:** [docs/brainstorms/2026-04-18-phase2-windows-crossplatform-brainstorm.md](../brainstorms/2026-04-18-phase2-windows-crossplatform-brainstorm.md)
- **Prior plan (superseded):** [docs/plans/2026-02-25-feat-windows-adapter-phase2-plan.md](./2026-02-25-feat-windows-adapter-phase2-plan.md) — covers a narrower P2 scope that this plan supersedes.
- **Related plan:** [docs/plans/2026-04-16-001-fix-ffi-safety-abi-correctness-plan.md](./2026-04-16-001-fix-ffi-safety-abi-correctness-plan.md) — FFI safety baseline this plan builds on.
- **Phases reference:** [docs/phases.md §Phase 2](../phases.md) (cleaned up in Unit 14).
- **Institutional learnings referenced:**
  - [docs/solutions/best-practices/deduplicate-ref-allocator-via-config-struct-2026-04-14.md](../solutions/best-practices/deduplicate-ref-allocator-via-config-struct-2026-04-14.md)
  - [docs/solutions/logic-errors/progressive-snapshot-review-contract-2026-04-16.md](../solutions/logic-errors/progressive-snapshot-review-contract-2026-04-16.md)
  - [docs/solutions/best-practices/deterministic-build-artifact-marker-2026-04-16.md](../solutions/best-practices/deterministic-build-artifact-marker-2026-04-16.md)
  - [docs/solutions/best-practices/identity-fingerprint-against-os-reorder-2026-04-16.md](../solutions/best-practices/identity-fingerprint-against-os-reorder-2026-04-16.md)
- **External references:**
  - UIA patterns: https://learn.microsoft.com/en-us/dotnet/framework/ui-automation/ui-automation-control-patterns-overview
  - `uiautomation` crate: https://crates.io/crates/uiautomation
  - `windows` crate: https://crates.io/crates/windows
  - `windows-capture` crate: https://crates.io/crates/windows-capture
  - ScreenCaptureKit: https://developer.apple.com/documentation/screencapturekit
  - `screencapturekit` crate (doom-fish fork): https://crates.io/crates/screencapturekit
  - `objc2` crate: https://crates.io/crates/objc2
  - `inventory` crate: https://crates.io/crates/inventory
  - `schemars` crate: https://crates.io/crates/schemars
  - MS Agent Framework MCP transport: https://learn.microsoft.com/en-us/ai/agent-framework (stdio sufficient through 1.0)
