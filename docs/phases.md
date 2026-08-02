# agent-desktop — Phase Roadmap

> Public source of truth for shipped and planned platform work.

---

## Release Tracker

Most recent shipments against this roadmap:

| Version | Date | What shipped |
|---------|------|---------------|
| **Unreleased** | — | Foundation reliability contract (Playwright-grade, U0–U19) completed on `feat/foundation-playwright-grade-contract` (PR #93, `feat!`) — capability-supertrait `PlatformAdapter` split, canonical role/state vocabulary, live `is --property visible`, `list-displays` + honest `--screen` + scale factor, truthful Automation permission, `native_id` identity spine, window-id-first resolution, `LocatorQuery` + live `find`, default-on auto-wait, three-way occlusion gate, `scroll_into_view`, core accname computation, `supported_surfaces` introspection, typed `ActionStep` delivery tier, `ProcessState` + `APP_UNRESPONSIVE`, `LaunchOptions`, `SignalBaseline` + `wait --event`, typed clipboard, mouse modifiers/wheel, FFI ABI major 3, envelope 2.1. Will cut the next minor per pre-1.0 policy. See [Phase 1.6](#phase-16--playwright-grade-foundation-contract-completed). |
| v0.4.7 | 2026-07-02 | Trace viewer and replay artifacts |
| v0.4.6 | 2026-07-02 | Sessions promoted to the first-class trace container: `session start/end/list/gc`, manifest-gated automatic JSONL segments under `sessions/<id>/trace/` |
| v0.4.5 | 2026-06-30 | `--wait-for` selector polling flags |
| v0.4.4 | 2026-06-29 | macOS + core adapter hardening with caller-controllable guardrails |
| v0.4.3 | 2026-06-28 | macOS `retained_handle` null-guard hardening against release-only `CFRetain(null)` |
| v0.4.2 | 2026-06-27 | FFI Phase B and C: Python smoke harness, parity gates, command-wrapper groundwork |
| v0.4.1 | 2026-06-26 | FFI C-ABI surface complete (Phase A): load-time ABI handshake, session-scoped adapter, JSON-envelope command entrypoints (version, status, snapshot, wait, execute-by-ref), tracing log callback |
| v0.4.0 | 2026-06-24 | **Breaking:** `version` command always emits the JSON envelope (drops `--json`); over-engineering cleanup |
| v0.3.1 | 2026-06-21 | macOS stale-ref resolution hardening |
| v0.3.0 | 2026-06-20 | Playwright-grade reliability hardening on the Phase 1 contracts: session-scoped latest snapshot pointers, qualified snapshot refs within explicit namespaces, actionability checks, headed/headless policy, JSONL `--trace`, stale-ref diagnostics, and refstore symlink hardening |
| v0.2.3 | 2026-06-06 | macOS AX window fallback hardening and fullscreen AX tree retrieval fixes |
| v0.2.2 | 2026-06-02 | macOS CFArray type-safety fix for Mail.app snapshot stability |
| v0.2.1 | 2026-05-23 | Empty accessibility-identity ref stability fix |
| v0.2.0 | 2026-05-20 | Unified command execution contracts; chain deadlines now return structured `TIMEOUT` instead of `ACTION_FAILED` |
| v0.1.14 | 2026-05 | Phase 1 unified core: typed batch/CLI path, `CommandPolicy`, `PermissionReport`, snapshot-scoped `RefStore`, headless `ActionRequest`, macOS screenshot backend boundary |
| v0.1.13 | 2026-04-17 | FFI cdylib on 5 platforms (aarch64/x86_64 macOS + Linux, x86_64 Windows MSVC), Sigstore build-provenance attestations, FFI review fixes (#26 — 66 commits) |
| v0.1.12 | 2026-03–04 | Progressive skeleton traversal + ref-rooted drill-down (#20) |
| v0.1.11 | 2026-02–03 | Skill-install prompt fix on all success paths |
| v0.1.9  | 2026-01–02 | Scalable skill architecture + ClawHub auto-publish (#14) |
| v0.1.8  | 2026-01    | `--compact` flag to collapse single-child unnamed nodes |
| v0.1.7  | 2025-12    | Electron / web app accessibility-tree compatibility |

- Phase 1 completion: incremental across v0.1.0 – v0.1.14 (macOS MVP, 58 shipped command names — 54 operational, 4 fail-closed pending daemon-owned held input — unified core engine).
- v0.2.0 – v0.3.1 unify command execution contracts and harden Playwright-grade ref reliability on top of the Phase 1 contracts.
- v0.4.0 – v0.4.7 complete the FFI C-ABI surface (load-time handshake, session-scoped adapter, JSON-envelope entrypoints), then land session-first tracing and a trace viewer.
- Foundation reliability contract (Playwright-grade, U0–U19) completed on `feat/foundation-playwright-grade-contract` — see [Phase 1.6](#phase-16--playwright-grade-foundation-contract-completed).
- Phase 1.5 completion: v0.1.13 (FFI cdylib on 5 platforms); the FFI C-ABI surface itself (`ad_snapshot` / `ad_execute_by_ref` / `ad_wait` / `ad_version` / `ad_status` / `ad_init` / `ad_abi_version`) completed in v0.4.1.
- Phase 2: planned. Delivered as dependency-ordered sub-phases 2.0–2.15 into the `feat/windows-adapter` integration branch — see [Platform Delivery Model](#platform-delivery-model--sub-phases-and-integration-branches) and the Phase 2 section below.
- Phase 3+: planned. See each phase section below for the additive platform work and trait defaults that later phases backfill.

---

## Phase Overview

| Phase | Name | Status | Platforms |
|-------|------|--------|-----------|
| 1 | Foundation + macOS MVP | **Completed** (v0.1.0 – v0.1.14) | macOS |
| 1.5 | FFI Distribution (C-ABI cdylib) | **Completed** (v0.1.13; C-ABI surface completed v0.4.1) | macOS, Windows, Linux |
| 1.6 | Playwright-grade Foundation Contract | **Completed** (PR #93) | macOS (contract in core) |
| 2 | Windows Adapter | Planned — sub-phases 2.0–2.15 | macOS, Windows |
| 3 | Linux Adapter | Planned — sub-phases 3.0–3.15 | macOS, Windows, Linux |
| 4 | MCP Server Mode | Planned | All |
| 5 | Production Readiness | Planned | All |

Future platform phases are additive against the Phase 1 + 1.5 + 1.6 contracts: typed command args, `CommandPolicy`, `PermissionReport`, snapshot-scoped refs, session-scoped latest snapshot pointers, `ActionRequest`, headed/headless interaction policy, JSONL reliability tracing, the capability-supertrait `PlatformAdapter` boundary (`ActionOps` / `InputOps` / `ObservationOps` / `SystemOps`), default-on auto-wait, and the occlusion gate. Core can still gain explicitly planned additive trait methods, but Windows/Linux implement — never fork — command semantics, and never duplicate transport dispatch. Phase 2 and Phase 3 ship as dependency-ordered sub-phases against a per-platform integration branch; see [Platform Delivery Model](#platform-delivery-model--sub-phases-and-integration-branches).

---

## Command Surface Architecture (DRY invariant)

Every command in agent-desktop has one shared semantic path. CLI and batch both parse into the same typed `Commands` enum, run the same `CommandPolicy` preflight, and enter the same `dispatch()` match. Platform crates implement primitives through `PlatformAdapter`; they do not own command semantics.

Current shipped code uses explicit match arms, not a runtime command registry. Later sections that discuss descriptor/codegen work are planned future transport-generation work; they do not describe the current CLI/batch dispatch path.

### Current Layering

| Layer | Scope | Invariant |
|-------|-------|-----------|
| `crates/core/src/commands/<name>.rs` | Platform-agnostic command behavior and args passed to `&dyn PlatformAdapter` | One command implementation |
| `src/cli/` / `src/cli_args/` | Clap command enum and transport args | CLI shape only, no platform behavior |
| `src/command_policy/` | Permissions, ref usage, side-effect classification | One policy source of truth for CLI, batch, and tests |
| `src/batch/` | JSON batch parser and executor | Deserializes into `Commands`; no separate command interpretation |
| `src/dispatch/` | Direct command match plus parse helpers | Shared CLI/batch execution path |
| `crates/{macos,windows,linux}/` | Adapter method implementations across four capability traits (`ActionOps`, `InputOps`, `ObservationOps`, `SystemOps`) | Same trait signatures across platforms |
| `crates/ffi/` | C ABI wrappers around adapter/core types | ABI marshaling only |

### Add a Command

1. Add `crates/core/src/commands/{name}.rs`.
2. Register it in `crates/core/src/commands/mod.rs`.
3. Add the CLI args/variant in `src/cli_args/` and `src/cli/mod.rs`.
4. Add a single arm in `src/dispatch/mod.rs`.
5. Add a `CommandPolicy` arm.
6. If needed, add one method to the relevant `PlatformAdapter` capability trait (`ActionOps` / `InputOps` / `ObservationOps` / `SystemOps`) with a `not_supported()` default, then implement it per adapter.

Batch receives the command automatically once `src/batch/mod.rs` maps the JSON command name to that same CLI enum variant. There is no separate batch-only behavior.

### Headless Contract

Ref actions use `ActionRequest { action, policy }`. The default `InteractionPolicy` forbids focus stealing and cursor movement. macOS is the reference adapter:

- Semantic AX steps run first.
- Physical fallbacks are explicit and policy-gated.
- Raw cursor commands (`hover`, `drag`, `mouse-*`) require `--headed`; other commands must not silently focus apps or move the cursor.
- Expected OS denials return specific error codes such as `PERM_DENIED`, `SNAPSHOT_NOT_FOUND`, or `POLICY_DENIED`, not generic `INTERNAL`.
- **Auto-wait is on by default (Phase 1.6).** Every ref-consuming action waits, bounded, for its target to become actionable — visible, enabled, stable, unoccluded, and receiving events — before acting, the same default-on model as Playwright's actionability checks. The bound is 5000ms; `--timeout-ms 0` restores pre-1.6 single-shot behavior (act immediately, fail fast, no retry loop). A three-way occlusion gate (`hit_test` → `ReachesTarget` / `InterceptedBy { role, name, bounds }` / `Unknown`) blocks delivery when another element visibly intercepts the target point, but an inconclusive probe reports `Unknown` and never false-fails the action. Every action step reports a typed `ActionStep { label, outcome, mechanism: Option<StepMechanism>, verified: Option<bool> }`, so callers can see whether delivery was `SemanticApi` or `PhysicalSynthetic` and whether the effect was independently verified — no command claims success it did not observe.

Windows and Linux should implement the same signatures rather than copying macOS-specific fallback decisions.

---

## Phase 1 — Foundation + macOS MVP

**Status: Completed** — shipped incrementally across v0.1.0 – v0.1.14, with the contract surface further hardened through v0.4.7 and the Phase 1.6 foundation contract (below).

Phase 1 is the load-bearing phase. It establishes the shared command path, trait boundaries, output contract, error types, permission model, ref lifecycle, and full workspace structure. All subsequent platform phases build on top of this foundation without duplicating command semantics.

### Objectives

| ID | Objective | Success Metric |
|----|-----------|----------------|
| P1-O1 | Working macOS snapshot CLI | `snapshot --app Finder` returns valid JSON with refs for all interactive elements |
| P1-O2 | Platform adapter trait | Trait compiles with mock adapter; macOS adapter satisfies all trait methods |
| P1-O3 | Ref-based interaction | `click @s8f3k2p9:e3` successfully invokes AXPress on the resolved element |
| P1-O4 | Context efficiency | Typical Finder snapshot < 500 tokens (measured via tiktoken) |
| P1-O5 | Typed JSON contract | Output envelope carries `version: "2.2"`. **Partial**: dedicated standalone JSON-Schema files were never delivered — deferred to later quality gates. |
| P1-O6 | Permission detection | Permission report covers Accessibility, Screen Recording, and Automation with recovery suggestions |
| P1-O7 | Command extensibility | Adding a new command follows the current shared path: `commands/{name}.rs` + `commands/mod.rs` + `src/cli_args/` + `src/cli/mod.rs` + `src/dispatch/mod.rs` + `src/command_policy/mod.rs` |
| P1-O8 | 58 shipped command names (54 operational) | All commands pass integration tests; `key-down` / `key-up` / `mouse-down` / `mouse-up` fail closed pending daemon-owned held input |
| P1-O9 | CI pipeline | GitHub Actions macOS runner executes full test suite on every PR |
| P1-O10 | Progressive skeleton traversal | Skeleton + drill-down workflow achieves 78%+ token savings on Electron apps |

P1-O3's `@s8f3k2p9:e3` example is the snapshot-qualified ref form current commands accept and emit. Other historical Phase 1 prose in this document may use the shorter legacy bare `@e3` form for brevity — see the Ref System note under [Phase 1.6](#phase-16--playwright-grade-foundation-contract-completed).

### Workspace Structure

```
agent-desktop/
├── Cargo.toml              # workspace: members, shared deps
├── CONCEPTS.md             # shared domain vocabulary for refs, snapshots, sessions, actionability
├── rust-toolchain.toml     # pinned Rust version
├── clippy.toml             # project-wide lint config
├── LICENSE                 # Apache-2.0 (shipped in every release tarball)
├── crates/
│   ├── core/               # agent-desktop-core (platform-agnostic)
│   │   └── src/
│   │       ├── lib.rs           # public re-exports only
│   │       ├── node.rs          # AccessibilityNode, Rect, WindowInfo
│   │       ├── adapter/         # capability traits + composed PlatformAdapter (actions.rs, input.rs, observation.rs, system.rs)
│   │       ├── adapter_session.rs # AdapterSession trait (open_session's Send+Sync return type)
│   │       ├── action.rs        # Action enum
│   │       ├── action_request.rs / action_result.rs / action_step.rs
│   │       ├── actionability/   # Live actionability checks, occlusion gate, stability sampling
│   │       ├── live_locator/    # LocatorQuery evaluation, hydration, evidence-based resolution
│   │       ├── locator.rs       # LocatorQuery, IdentityPredicate, StatePredicate
│   │       ├── hit_test.rs      # HitTestResult (ReachesTarget / InterceptedBy / Unknown)
│   │       ├── process_state.rs # ProcessState (Running / Exited / Crashed / Unresponsive)
│   │       ├── launch_options.rs # LaunchOptions (args, env, cwd, timeout_ms, attach_if_running)
│   │       ├── clipboard_content.rs # ClipboardContent (Text / Image / FileUrls)
│   │       ├── display_info.rs  # DisplayInfo (id, bounds, is_primary, scale)
│   │       ├── accname.rs       # Core accessible-name computation over NameEvidence
│   │       ├── name_evidence.rs # NameEvidence supplied by adapters
│   │       ├── role.rs / state.rs / state_predicate.rs # canonical role/state vocabulary
│   │       ├── signal_baseline.rs / signals.rs # SignalBaseline capture + diff_signals for wait --event
│   │       ├── deadline.rs      # Deadline + thread_local INHERITED_DEADLINE propagation
│   │       ├── refs.rs          # RefMap and RefEntry
│   │       ├── refs_store.rs    # Snapshot/session-scoped ref persistence
│   │       ├── refs_lock.rs     # RefStore write lock
│   │       ├── ref_action.rs    # Ref-consuming action pipeline (poll/wait/exactly-once family)
│   │       ├── ref_alloc.rs     # INTERACTIVE_ROLES, allocate_refs, is_collapsible, transform_tree
│   │       ├── snapshot_ref.rs  # Ref-rooted drill-down (run_from_ref)
│   │       ├── snapshot_surface.rs # SnapshotSurface (predeclared cross-platform surface vocabulary)
│   │       ├── snapshot.rs      # SnapshotEngine (filter, allocate, serialize)
│   │       ├── session/         # session start/end/list/gc, manifest, liveness
│   │       ├── private_file*.rs # 0600-equivalent private artifact writing (portable std::fs on every target; Windows ACL hardening not yet built, see Phase 2.1)
│   │       ├── trace.rs / trace_read/ # JSONL reliability trace, segment merge, HTML viewer
│   │       ├── error_code.rs    # ErrorCode enum
│   │       ├── adapter_error.rs / app_error.rs # AdapterError, AppError
│   │       ├── output.rs        # ENVELOPE_VERSION ("2.2") + envelope builders
│   │       ├── notification.rs  # NotificationInfo, NotificationFilter, NotificationIdentity
│   │       └── commands/        # one file per command (direct match, no Command trait)
│   ├── macos/              # agent-desktop-macos (Phase 1, shipped)
│   ├── windows/            # agent-desktop-windows (stub → Phase 2)
│   ├── linux/              # agent-desktop-linux (stub → Phase 3)
│   └── ffi/                # agent-desktop-ffi (cdylib, shipped v0.1.13; C-ABI surface completed v0.4.1; see Phase 1.5)
├── src/                    # agent-desktop binary (entry point)
│   ├── main.rs
│   ├── batch/               # JSON batch -> typed Commands
│   ├── cli/                 # Clap enum, help text, contract tests
│   ├── cli_args/            # Command argument structs by domain
│   ├── command_policy/      # Permission/ref/side-effect policy
│   ├── dispatch/            # Command dispatcher and parse helpers
│   └── tests/               # Binary-level conformance tests
├── docs/
│   └── solutions/           # documented solutions to past problems, tagged by module/problem_type
└── tests/
    ├── fixtures/
    ├── fixture-app/         # SwiftUI/AppKit fixture app for live e2e (AgentDeskFixture.swift + build.sh)
    └── integration/
```

This tree is representative, not exhaustive — `crates/core/src/` alone holds roughly 200 files (one struct/enum/fn-group per file) plus the subdirectories named above.

### PlatformAdapter Trait

The single most important abstraction. Every platform-specific operation goes through this trait. Core never imports platform crates. Since Phase 1.6, `PlatformAdapter` is a **composed capability supertrait**, not one flat trait:

```rust
// crates/core/src/adapter/mod.rs
pub trait PlatformAdapter: ObservationOps + ActionOps + InputOps + SystemOps {}
impl<T: ObservationOps + ActionOps + InputOps + SystemOps> PlatformAdapter for T {}
```

Each capability lives in its own file under `crates/core/src/adapter/`, and every method defaults to `Err(AdapterError::not_supported(..))` unless noted — this is what lets a platform crate implement only what it supports while the workspace stays green. `crates/core/src/lib.rs` re-exports the four traits and `PlatformAdapter`. The method lists below are representative; the four files are the source of truth.

**`ActionOps`** (`adapter/actions.rs`) — element interaction:
```rust
fn execute_action(&self, handle: &NativeHandle, request: ActionRequest, lease: &InteractionLease) -> Result<ActionResult, AdapterError>;
fn scroll_into_view(&self, handle: &NativeHandle, lease: &InteractionLease) -> Result<(), AdapterError>;
```

**`InputOps`** (`adapter/input.rs`) — raw OS input and clipboard:
```rust
fn mouse_event(&self, event: MouseEvent, lease: &InteractionLease) -> Result<(), AdapterError>;
fn key_event(&self, combo: &KeyCombo, down: bool, lease: &InteractionLease) -> Result<(), AdapterError>;
fn drag(&self, params: DragParams, lease: &InteractionLease) -> Result<(), AdapterError>;
fn get_clipboard_content(&self, format: ClipboardFormat, deadline: Deadline) -> Result<Option<ClipboardContent>, AdapterError>;
fn set_clipboard_content(&self, content: &ClipboardContent, lease: &InteractionLease) -> Result<(), AdapterError>;
fn clear_clipboard(&self, lease: &InteractionLease) -> Result<(), AdapterError>;
```
`get_clipboard` / `set_clipboard` (untyped `String`) were removed pre-1.0 in favor of the typed `ClipboardContent` methods above; the C ABI (`ad_get_clipboard` / `ad_set_clipboard`) marshals the typed content and is unaffected by the Rust-side rename.

**`ObservationOps`** (`adapter/observation.rs`) — reading the tree and live element state:
```rust
fn observe_tree(&self, root: ObservationRoot<'_>, request: &ObservationRequest) -> Result<ObservedTree, AdapterError>;
fn list_windows(&self, filter: &WindowFilter, deadline: Deadline) -> Result<Vec<WindowInfo>, AdapterError>;
fn list_apps(&self, deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError>;
fn get_tree(&self, win: &WindowInfo, opts: &TreeOptions, deadline: Deadline) -> Result<AccessibilityNode, AdapterError>;
fn get_subtree(&self, handle: &NativeHandle, opts: &TreeOptions, deadline: Deadline) -> Result<AccessibilityNode, AdapterError>;
fn resolve_element_strict(&self, entry: &RefEntry, deadline: Deadline) -> Result<NativeHandle, AdapterError>;
fn resolve_locator_anchor(&self, entry: &RefEntry, deadline: Deadline) -> Result<NativeHandle, AdapterError>;
fn list_surfaces(&self, process: ProcessIdentity, deadline: Deadline) -> Result<Vec<SurfaceInfo>, AdapterError>;
fn get_live_value(&self, handle: &NativeHandle, deadline: Deadline) -> Result<Option<String>, AdapterError>;
fn get_live_state(&self, handle: &NativeHandle, deadline: Deadline) -> Result<Option<ElementState>, AdapterError>;
fn get_live_actions(&self, handle: &NativeHandle, deadline: Deadline) -> Result<Option<Vec<String>>, AdapterError>;
fn get_live_element(&self, handle: &NativeHandle, deadline: Deadline) -> Result<LiveElement, AdapterError>;
fn get_element_bounds(&self, handle: &NativeHandle, deadline: Deadline) -> Result<Option<Rect>, AdapterError>;
fn hit_test(&self, handle: &NativeHandle, point: Point, deadline: Deadline) -> Result<HitTestResult, AdapterError>;
```
`list_apps_scoped` is the one method in this trait with a real default (filters `list_apps()`), rather than `not_supported()`.

**`SystemOps`** (`adapter/system.rs`) — lifecycle, windows, permissions, capture, notifications:
```rust
fn acquire_interaction_lease(&self, deadline: Deadline) -> Result<InteractionLease, AdapterError>;
fn permission_report(&self, deadline: Deadline) -> Result<PermissionReport, AdapterError>;
fn focus_window(&self, win: &WindowInfo, lease: &InteractionLease) -> Result<(), AdapterError>;
fn launch_app(&self, id: &str, options: &LaunchOptions, lease: &InteractionLease) -> Result<WindowInfo, AdapterError>;
fn close_app(&self, app: &AppInfo, force: bool, lease: &InteractionLease) -> Result<(), AdapterError>;
fn process_state(&self, process: ProcessIdentity, deadline: Deadline) -> Result<ProcessState, AdapterError>;
fn is_protected_process(&self, identifier: &str) -> bool;
fn window_op(&self, win: &WindowInfo, op: WindowOp, lease: &InteractionLease) -> Result<(), AdapterError>;
fn screenshot(&self, target: ScreenshotTarget, deadline: Deadline) -> Result<ImageBuffer, AdapterError>;
fn list_displays(&self, deadline: Deadline) -> Result<Vec<DisplayInfo>, AdapterError>;
fn focused_window(&self, deadline: Deadline) -> Result<Option<WindowInfo>, AdapterError>;
fn press_key_for_app(&self, process: ProcessIdentity, combo: &KeyCombo, policy: InteractionPolicy, lease: &InteractionLease) -> Result<ActionResult, AdapterError>;
fn supported_surfaces(&self) -> Vec<SnapshotSurface>;
fn open_session(&self, affinity: &SessionAffinity, deadline: Deadline) -> Result<Box<dyn AdapterSession>, AdapterError>;
fn capture_signal_baseline(&self, filter: &SignalFilter, deadline: Deadline) -> Result<SignalBaseline, AdapterError>;
fn list_notifications(&self, filter: &NotificationFilter, policy: InteractionPolicy, deadline: Deadline, lease: Option<&InteractionLease>) -> Result<Vec<NotificationInfo>, AdapterError>;
fn dismiss_notification(&self, request: DismissNotificationRequest<'_>, lease: &InteractionLease) -> Result<NotificationInfo, AdapterError>;
fn dismiss_all_notifications(&self, request: DismissAllNotificationsRequest<'_>, lease: &InteractionLease) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError>;
fn notification_action(&self, request: NotificationActionRequest<'_>, lease: &InteractionLease) -> Result<ActionResult, AdapterError>;
```
`permission_report`, `request_permissions`, `unknown_accessibility_means_unsupported`, `supported_surfaces`, `is_protected_process`, and `is_blocked_combo` carry real (non-`not_supported`) defaults, so a bare-minimum adapter still reports something sane.

`open_session` is the Send+Sync landing zone later phases use for platform session affinity — the Windows COM-MTA worker thread and the Linux D-Bus connection both attach through `AdapterSession::close(self: Box<Self>)`, not through `PlatformAdapter` itself.

### Key Supporting Types

- `Action` — closed core enum whose platform dispatch arms must stay exhaustive. Current 21 variants (`crates/core/src/action.rs`): `Click`, `DoubleClick`, `RightClick`, `TripleClick`, `SetValue(String)`, `SetFocus`, `Expand`, `Collapse`, `Select(String)`, `Toggle`, `Check`, `Uncheck`, `Scroll(Direction, u32)`, `ScrollTo`, `PressKey(KeyCombo)`, `KeyDown(KeyCombo)`, `KeyUp(KeyCombo)`, `TypeText(String)`, `Clear`, `Hover`, `Drag(DragParams)`. Also carries `headed_requirement()`, `name()`, `requires_cursor_policy()`, `requires_hit_test()`, `requires_scroll_into_view()`, `may_use_focus_fallback()`, `base_interaction_policy()`.
- `ActionRequest` — `{ action, policy }`; default policy forbids focus stealing and cursor movement
- `ActionStep` — `{ label: String, outcome: ActionStepOutcome, mechanism: Option<StepMechanism>, verified: Option<bool> }`; `StepMechanism` distinguishes `SemanticApi` delivery from `PhysicalSynthetic` delivery so every reported step is honest about how it reached the target
- `LocatorQuery` — `{ identity: IdentityPredicate, has_text: Option<String>, exact: bool, states: Vec<StatePredicate>, containment: ContainmentPredicate }`; evaluated by `resolve_query` and the live `find` command
- `HitTestResult` — enum `ReachesTarget | InterceptedBy { role, name, bounds } | Unknown`; `Unknown` on probe failure, never a false negative
- `ProcessState` — enum `Running | Exited { code: Option<i32> } | Crashed { signal_or_code: i32 } | Unresponsive`
- `LaunchOptions` — `{ args: Vec<String>, env: BTreeMap<String,String>, cwd: Option<PathBuf>, timeout_ms: u64, attach_if_running: bool }` (default `timeout_ms` 5000, `attach_if_running` true)
- `ClipboardContent` — enum `Text(String) | Image(ImageBuffer) | FileUrls(Vec<String>)`
- `DisplayInfo` — `{ id: String, bounds: Rect, is_primary: bool, scale: f64 }`
- `NameEvidence` — `{ explicit_label, labelled_by_text, native_title, static_value, child_label, placeholder, description: all Option<String> }`; adapters supply evidence, core computes accessible-name precedence
- `SignalBaseline` — `{ windows: Vec<WindowInfo>, apps: Vec<AppInfo>, surfaces: Vec<SurfaceSignal>, completeness: SignalCompleteness }`; `diff_signals(baseline, current) -> Vec<UiEvent>` is a pure function that never touches the adapter
- `Deadline` — `{ started_at, expires_at, timeout_ms, capped }`; thread-local `INHERITED_DEADLINE` propagation via `enter_scope` / `min_expiry` so nested reads share one budget
- `AdapterSession` — `trait AdapterSession: Send + Sync { fn close(self: Box<Self>) -> Result<(), AdapterError>; }`, returned by `open_session`
- `SurfaceWait` — command-local enum (`crates/core/src/commands/wait_surface.rs`) used by `wait`: `Menu | MenuClosed | Notification`. Not a shared core type — scoped to the `wait` command's own predicate handling.
- `ProcessId` — transparent `u32` process identifier shared by core, every adapter, JSON, and the C ABI. Platform adapters perform checked conversion only at native boundaries such as macOS `pid_t`; core never narrows a Windows `DWORD` process ID to a signed integer
- `PermissionReport` — `{ accessibility, screen_recording, automation }`, each `{ "state": "granted" }`, `{ "state": "denied", "suggestion": "..." }`, `{ "state": "not_required" }`, or `{ "state": "unknown" }`
- `MouseEvent`, `DragParams`, `KeyCombo` — dedicated types (not unified under an `InputEvent` enum). The CLI-facing `DragArgs` pairs `DragEndpoint { ref_id: Option<String>, xy: Option<(f64,f64)> }` values for `from`/`to`.
- `WindowOp` — Resize{w,h}, Move{x,y}, Minimize, Maximize, Restore, Close
- `ScreenshotTarget` — Screen(usize), Display { index, expected }, ExactWindow(WindowInfo), FullScreen
- `NotificationInfo` — index, app_name, title, body, actions: Vec<String>
- `NotificationIdentity` — expected_app, expected_title (used for NC-reorder-safe `notification_action`)
- `SurfaceInfo` — kind, label, bounds (for `list-surfaces` command)
- `SnapshotSurface` — predeclared cross-platform surface vocabulary: `Window`, `Focused`, `Menu`, `Menubar`, `Sheet`, `Popover`, `Alert`, `Desktop`, `Toolbar`, `NotificationCenter`, plus shell-specific variants already predeclared for later phases (`Dock`, `Spotlight`, `MenuBarExtras` for macOS; `Taskbar`, `SystemTray`, `SystemTrayOverflow`, `QuickSettings`, `StartMenu`, `ActionCenter` for Windows). Declaring a variant does not imply any adapter implements it — see `supported_surfaces()` introspection.
- `TreeOptions` — max_depth, include_bounds, interactive_only, compact, surface, skeleton (root is CLI-only via `SnapshotArgs.root_ref`, not plumbed into `TreeOptions`)

### macOS Adapter Implementation

Located in `crates/macos/src/` following the platform crate folder structure (49 files under `tree/`, 31 under `actions/`, 30 under `input/`, 47 under `system/`, 12 under `notifications/` — the listing below is a representative subset):

```
crates/macos/src/
├── lib.rs / adapter.rs / cf_type.rs   # mod glue + MacOSAdapter: PlatformAdapter impl + CFType helpers
├── tree/
│   ├── mod.rs, element.rs, ax_element.rs, ax_value.rs, attributes.rs, capabilities.rs
│   ├── build_context.rs, element_bounds.rs, element_dedupe.rs, element_name.rs, child_labels.rs
│   ├── hit_test.rs, native_id.rs, roles.rs, state_reader.rs, surfaces.rs, surface_inventory.rs
│   ├── resolve.rs, resolve_classify.rs, resolve_search.rs, resolve_bounds.rs, resolve_roots.rs, locator_deadline.rs
│   ├── query/           # live-locator query evaluation glue
│   └── node_attribute_*.rs, node_attr_states.rs, node_control_states.rs, node_semantic_states.rs, node_identifiers.rs (batched attribute read/decode family)
├── actions/
│   ├── mod.rs, dispatch.rs, adapter.rs, ax_helpers.rs, ax_mutation.rs
│   ├── chain.rs, chain_context.rs, chain_def.rs, chain_defs.rs, chain_delivery.rs, chain_disclosure_steps.rs, chain_menu_steps.rs, chain_step.rs, chain_step_exec.rs, chain_value_write.rs, chain_verify.rs
│   ├── delivery_tracker.rs, post_state.rs, extras.rs, select_menu.rs, toggle_state.rs
│   ├── physical_click.rs, physical_keyboard.rs, physical_target.rs, type_text.rs
│   └── scroll.rs, scroll_into_view.rs, scroll_read.rs
├── input/
│   ├── mod.rs, adapter.rs, keyboard.rs, keyboard_event.rs, keyboard_map.rs
│   ├── mouse.rs, mouse_drag.rs, mouse_drag_state.rs, mouse_move.rs, mouse_scroll.rs
│   ├── blocked_combo.rs, owned_object.rs
│   └── clipboard.rs, clipboard_file_urls.rs, clipboard_image_io.rs, clipboard_rich.rs, clipboard_runtime.rs, clipboard_transaction.rs, clipboard_helper_{client,dl,entry,identity,process,protocol}.rs  # typed clipboard + isolated-helper protocol family
├── notifications/
│   ├── mod.rs, list.rs, actions.rs, dismiss_verify.rs, read.rs, scan.rs
│   └── nc_session.rs   # NcSession / NcSessionOps RAII lifecycle (open/wait-ready/close/reactivate)
└── system/
    ├── mod.rs, adapter.rs, app_inventory.rs, app_ops.rs, process.rs, process_apps.rs, process_identity.rs, process_state.rs
    ├── window_inventory.rs, window_inventory_global.rs, window_ops.rs, window_ax_state.rs, window_bridge.rs, window_postcondition.rs, window_resolve.rs
    ├── display.rs, display_work_area.rs, cg_window.rs, cg_window_exact.rs
    ├── focus.rs, key_dispatch.rs, renderer_activation.rs, launch.rs, launch_bridge.m, launch_workspace.rs, launch_completion.rs, launch_callback_result.rs
    ├── permission_helper.rs, permission_operation.rs, permissions.rs
    ├── screenshot.rs, screen_bridge.m, screen_bridge_contract_tests.rs
    ├── signals.rs, wait.rs
    └── appkit_bridge.m / appkit_bridge.rs, cocoa_runtime.rs, workspace_apps.rs
```

**Tree traversal:**
- Entry: `AXUIElementCreateApplication(pid)` for app root
- Children: `kAXChildrenAttribute` recursively with ancestor-path set (not global visited set — macOS reuses AXUIElementRef pointers across sibling branches)
- Batch fetch: `AXUIElementCopyMultipleAttributeValues` for 3-5x faster attribute reads
- Role mapping: AXRole strings → unified role enum in `tree/roles.rs`
- Max depth default: 10, configurable via `--max-depth`
- Name: core `accname.rs` computes accessible-name precedence over the `NameEvidence` the adapter supplies (`kAXTitleAttribute` / `kAXDescriptionAttribute` / static value / child label / placeholder). Value: `kAXValueAttribute`
- Bounds: `kAXPositionAttribute` + `kAXSizeAttribute` combined to Rect
- Identity: `native_id` reads macOS `AXIdentifier`, falling back to `AXDOMIdentifier` for web content

**Action execution:**
- Ref actions take `ActionRequest`, not bare `Action`, and auto-wait for actionability before dispatch (Phase 1.6)
- Default policy forbids focus stealing and cursor movement
- Click/right-click/scroll use semantic AX delivery headlessly and prefer physical delivery only under explicit `--headed`
- Type uses the focus-fallback policy floor; SetValue/Clear are the pure headless AX value-mutation paths
- SetValue/Clear: `AXUIElementSetAttributeValue(kAXValueAttribute, value)`
- SetFocus/Press/Hover/Drag/Mouse: explicit focus/cursor/physical commands
- Keyboard/Mouse: `CGEventCreateKeyboardEvent` / `CGEventCreateMouseEvent` via CoreGraphics; mouse events carry modifier chords and wheel deltas
- Clipboard: `NSPasteboard.generalPasteboard` read/write via Cocoa FFI, marshaled through typed `ClipboardContent` (`Text` / `Image` / `FileUrls`) via the isolated clipboard-helper protocol
- Screenshot: `ScreenshotBackend` boundary with secure temporary files; Screen Recording denial maps to `PERM_DENIED`
- Every step reports a typed `ActionStep` (`SemanticApi` vs `PhysicalSynthetic` mechanism, `verified: Option<bool>`) — see Phase 1.6

**Permission detection:**
- Probe once per CLI process into `PermissionReport`
- Accessibility: `AXIsProcessTrusted()` / `AXIsProcessTrustedWithOptions(prompt: true)`
- Screen Recording: platform screen-capture preflight/request path
- Automation: probed against System Events without prompting for `permissions` / `status`; `permissions --request` may prompt through the bounded isolated permission helper. The probe is truthful (Phase 1.6 U4) rather than optimistically assumed.
- `status`, `permissions`, preflight, and `batch` share the same report; `permissions --request` invokes the request path

**Notification management:**
- Open Notification Center through the guarded System Events path to Control Center's Clock item; authorization is probed without prompting before the Apple Event is sent
- List notifications: traverse the Notification Center AX tree — each notification is an `AXGroup` with title, subtitle, and action buttons
- Dismiss: perform `AXPress` on the notification's close button, or `AXRemoveFromParent` if supported
- Interact: resolve action buttons within a notification group and perform `AXPress`
- Dismiss all: `AXPress` the "Clear All" button at the group level
- Do Not Disturb detection: read Focus/DND state via `NSDoNotDisturbEnabled` user defaults or `CoreFoundation` preferences

**System tray / Menu bar extras:**
- Menu bar extras (status items) live under the `SystemUIServer` process AX tree
- Current support is through surface discovery/snapshotting (`menubar` / `menu`) where the AX tree exposes those items
- Dedicated `list-tray-items`, `click-tray-item`, and `open-tray-menu` commands are not shipped
- Control Center items: accessible via the `ControlCenter` process (bundleId: `com.apple.controlcenter`)

**AXElement safety:**
- Inner field: `pub(crate)` not `pub` (prevents double-free via raw pointer extraction)
- `Clone` impl must call `CFRetain`
- `Drop` impl must call `CFRelease`

### Snapshot Engine and Ref Allocator

Platform-agnostic, lives in `agent-desktop-core`:

1. Raw tree: Call `adapter.get_tree(window, opts)`
2. Filter: Remove invisible/offscreen. Remove empty groups with no interactive descendants. Prune beyond max_depth
3. Allocate refs: Depth-first. Interactive roles get sequence numbers `e1`, `e2`, etc., emitted snapshot-qualified as `@<snapshot_id>:e1`, `@<snapshot_id>:e2` (e.g. `@s8f3k2p9:e1`). Legacy bare `@e1` remains valid input only with an explicit `--snapshot <id>`. Store in RefMap
4. Serialize: Omit null fields. Omit empty arrays. Omit bounds in compact mode
5. Estimate tokens: Optionally warn if exceeding threshold

Snapshot refs persist through `RefStore`. The default namespace stores snapshots under `~/.agent-desktop/snapshots/{snapshot_id}/refmap.json`; `--session <id>` stores the same shape under `~/.agent-desktop/sessions/{id}/snapshots/{snapshot_id}/refmap.json`. Each namespace owns one `latest_snapshot_id` pointer for commands that omit `--snapshot`. A qualified ref or explicit `--snapshot <id>` identifies the snapshot within the selected namespace; session-owned snapshots still require the matching `--session` or `AGENT_DESKTOP_SESSION` scope, and lookup never searches another namespace. `~/.agent-desktop/last_refmap.json` remains only as a latest-snapshot inspection artifact. Action commands resolve through `RefStore` using strict re-identification from platform-neutral `RefEntry` evidence — pid, role, path/source surface, role-conditional stable text identity, and bounds hash; mutable control values are volatile and are never treated as stable text identity. Return `STALE_REF` on 0 live candidates and `AMBIGUOUS_TARGET` on 2+ plausible live candidates.

**Progressive Skeleton Traversal:**
- `--skeleton` flag clamps depth to `min(max_depth, 3)`, annotates truncated containers with `children_count` for agent discovery
- `--root <REF>` flag starts traversal from a previously-discovered ref instead of window root; `--snapshot <snapshot_id>` selects the ref namespace
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

#### Wait Command Update (Notification — Completed, Menu — Completed, Event/Selector — Completed in 1.6)

The `wait` command has been extended with notification, menu, event-diff, and selector-polling support:
- `wait --notification` — Wait for any new notification to appear (index-diff based detection)
- `wait --notification --app Safari` — Wait for a notification from a specific app
- `wait --notification --text "Download complete"` — Wait for a notification containing specific text
- `wait --menu` / `wait --menu-closed` — Wait for context menu open/close
- `wait --event <kind> [--app ...] [--window-id ...]` — Wait for a `SignalBaseline` diff event (window-opened/closed, app-launched/terminated, focus-changed, surface-appeared); see [Phase 1.6](#phase-16--playwright-grade-foundation-contract-completed)
- `--wait-for <selector>` — Poll a `LocatorQuery`-shaped selector until it matches (v0.4.5)

### Commands Shipped (58 names — 54 operational, 4 fail closed)

| Category | Commands | Count |
|----------|----------|-------|
| App / Window | `launch`, `close-app`, `list-windows`, `list-displays`, `list-apps`, `focus-window`, `resize-window`, `move-window`, `minimize`, `maximize`, `restore` | 11 |
| Observation | `snapshot`, `screenshot`, `find` (live `LocatorQuery` lookup), `get` (text, value, title, bounds, role, states, tree-stats), `is` (live property reads: visible, enabled, checked, focused, expanded, ...), `list-surfaces` | 6 |
| Interaction | `click`, `double-click`, `triple-click`, `right-click`, `type`, `set-value`, `clear`, `focus`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse` | 14 |
| Scroll | `scroll`, `scroll-to` | 2 |
| Keyboard | `press`, `key-down`\*, `key-up`\* | 3 |
| Mouse | `hover`, `drag`, `mouse-move`, `mouse-click`, `mouse-down`\*, `mouse-up`\*, `mouse-wheel` | 7 |
| Clipboard | `clipboard-get`, `clipboard-set`, `clipboard-clear` | 3 |
| Notification (macOS) | `list-notifications`, `dismiss-notification`, `dismiss-all-notifications`, `notification-action` | 4 |
| Wait | `wait` (`--element`, `--window`, `--text`, `--menu`, `--notification`, `--event <kind>`, `--app`, `--window-id` flags) | 1 |
| System | `status`, `permissions`, `version`, `skills`, `session` (start/end/list/gc), `trace` (export/show) | 6 |
| Batch | `batch` | 1 |

11+6+14+2+3+7+3+4+1+6+1 = 58 (54 operational + 4 fail-closed).

\* **Fail closed:** `key-down`, `key-up`, `mouse-down`, `mouse-up` parse and validate their arguments, then unconditionally return an error through `input_hold_policy::reject(...)` before ever calling the adapter (`crates/core/src/commands/key_down.rs`, `key_up.rs`, `mouse_down.rs`, `mouse_up.rs`). Held input (press without an immediate matching release) needs an owner that outlives a single CLI invocation to guarantee release — that owner is the Phase 5 daemon. Until then, these four names exist in the CLI/FFI/MCP surface — so scripts and generated schemas can reference them by a stable name — but they always fail with a suggestion to use `press`, `mouse-click`, or `drag` instead.

> System Tray / Menu Bar Extras commands are listed under "Not Yet Implemented" above — they never shipped in Phase 1.

### JSON Output Contract

All commands produce a response envelope with `version: "2.2"`. Standalone schema files are still deferred; the current contract is enforced by Rust serde types, CLI conformance tests, and documented examples.

Success:
```json
{
  "version": "2.2",
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
  "version": "2.2",
  "ok": false,
  "command": "click",
  "error": {
    "code": "STALE_REF",
    "message": "Element could not be resolved from the requested snapshot",
    "suggestion": "Run 'snapshot' to refresh, then retry with updated ref",
    "recovery": {
      "strategy": "refresh_snapshot_then_retry_original",
      "retryable": true,
      "requires_fresh_snapshot": true
    },
    "disposition": {
      "delivery": "not_delivered",
      "retry": "safe"
    }
  }
}
```

The `error` object may also carry an optional `details` object. `recovery` and `disposition` are both present whenever the error type has a well-defined recovery path; consumers use `recovery.strategy` only when `disposition.retry` is `"safe"`. Envelope 2.1 removed the 2.0 `retry_command` string outright — there is no compatibility alias, so 2.0 consumers reading `retry_command` must migrate to `recovery.strategy`.

Serialization rules: omit null/None fields (`skip_serializing_if`), omit empty arrays, omit bounds in compact mode, `ref_count` and `tree` inside `data`.

### Error Taxonomy

The `ErrorCode` enum in `crates/core/src/error_code.rs` (`AdapterError` lives in `adapter_error.rs`, `AppError` in `app_error.rs` — there is no `error.rs`) exposes these machine-readable variants:

| Code | Category | Example | Recovery Suggestion |
|------|----------|---------|---------------------|
| `PERM_DENIED` | Permission | Accessibility not granted | Open System Settings > Privacy > Accessibility and add the app that launches agent-desktop |
| `ELEMENT_NOT_FOUND` | Ref | @s8f3k2p9:e12 could not be resolved | Run 'snapshot' to refresh, then retry with updated ref |
| `APP_NOT_FOUND` | Application | --app 'Photoshop' not running | Launch the application first |
| `ACTION_FAILED` | Execution | AXPress returned error on disabled button | Element may be disabled. Check states before acting |
| `ACTION_NOT_SUPPORTED` | Execution | Expand on a button | This element does not support the requested action |
| `STALE_REF` | Ref | Element could not be re-identified from the requested snapshot | Run 'snapshot' (or `snapshot --skeleton`) to refresh |
| `AMBIGUOUS_TARGET` | Ref | Ref identity maps to more than one live candidate | Run 'snapshot' to refresh, then retry with a more specific ref |
| `WINDOW_NOT_FOUND` | Window | --window w-999 does not exist | Run 'list-windows' to see available windows |
| `PLATFORM_NOT_SUPPORTED` | Platform | Windows/Linux adapter not yet shipped | This platform ships in Phase 2/3 |
| `TIMEOUT` | Wait / Traversal / Auto-wait | wait --element exceeded timeout; auto-wait exceeded 5000ms without the target becoming actionable | Increase --timeout or --timeout-ms, or check app state |
| `INVALID_ARGS` | Input | Bad CLI argument or unknown ref format | Fix the argument per CLI help |
| `NOTIFICATION_NOT_FOUND` | Notification | Notification ID not found / NC reordered | Run 'list-notifications' to see current notifications |
| `SNAPSHOT_NOT_FOUND` | Ref | Requested snapshot ID is missing | Run 'snapshot' again and use the returned snapshot_id |
| `POLICY_DENIED` | Action policy | Physical input blocked by headless policy | Retry with `--headed` for explicit cursor movement, or use a semantic AX action when available |
| `APP_UNRESPONSIVE` | Liveness | Target process failed a terminal read-only liveness probe (hung app window, not-responding) | Wait for the app to recover, or close/relaunch it — this is a terminal enrichment only; transient AX messaging blips degrade gracefully and never by themselves fail a whole command |
| `INTERNAL` | Internal | Unexpected error or caught panic | Re-run with verbose logging |

Exit codes: `0` success, `1` structured error (JSON on stdout), `2` argument/parse error.

> Codes the earlier draft listed but that **do not exist** in the codebase: `TREE_TIMEOUT` (use `TIMEOUT`), `CLIPBOARD_EMPTY` (no special code; empty clipboard returns an empty string / `None` content), `NOTIFICATION_UNSUPPORTED` (use `PLATFORM_NOT_SUPPORTED`), `TRAY_NOT_FOUND` / `TRAY_UNSUPPORTED` (tray commands never shipped). The liveness enrichment shipped in Phase 1.6: `APP_UNRESPONSIVE` is now a real code (a failed read-only liveness probe upgrades a hang via `ProcessState::Unresponsive`), while ordinary AX messaging exhaustion still reports plain `TIMEOUT` — the once-mooted `AX_MESSAGING_TIMEOUT` was never added as a variant, and `AUTOMATION_PERMISSION_DENIED` folded into `PERM_DENIED` with platform detail. Remaining future-candidate codes: `PERMISSION_REVOKED` (TCC yanked mid-session, distinct from `PERM_DENIED`), `RESOURCE_EXHAUSTED` (refmap >1MB, tree node-count cap).

### Testing

**Unit tests (core):**
- AccessibilityNode ser/de roundtrips
- Ref allocator only assigns interactive roles
- SnapshotEngine filtering
- Error serialization
- JSON contract / output conformance coverage
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
- Menu/menu-bar surface snapshot and wait behavior where the host exposes AX menu surfaces

**Golden fixtures (`tests/fixtures/`):**
- Real snapshots from Finder, TextEdit, etc. checked into repo
- Regression-test serialization format changes

**Live e2e (`tests/e2e/run.sh`, Phase 1.6):**
- Requires macOS + `AGENT_DESKTOP_E2E_EXCLUSIVE=1`; builds the release binary, macOS helper, and `release-ffi` cdylib
- Re-execs itself through `interaction_lock.py run` for exclusive-desktop serialization
- Drives the release binary against the SwiftUI/AppKit fixture app (`tests/fixture-app/AgentDeskFixture.swift`) and asserts every effect by independent observation — never the command's own `ok:true`
- Covers every ref action in both headless and `--headed` mode

### CI

Current `.github/workflows/ci.yml` runs on push to main/master, pull_request, and workflow_dispatch:

| Job | Runner | What it checks |
|-----|--------|-----------------|
| `fmt` | ubuntu-latest | `cargo fmt --all -- --check`; shellcheck + bash3-compat on e2e scripts; `py_compile` + unittest on `tests/e2e/*.py`; actionlint on workflow files |
| `msrv` | ubuntu-latest | `cargo +1.89.0 check` (pinned MSRV) on core, linux, and the binary crate |
| `platform-check` | matrix: Linux / Windows / macOS | `cargo check --all-targets` per platform crate + binary — proves the stub crates compile on their target before any adapter code lands |
| `test` | macos-latest | Dependency isolation check (`cargo tree -p agent-desktop-core` has zero platform crate names), release-consistency check, file-size rule check, `cargo clippy --all-targets -- -D warnings`, core+macos unit tests, `locator_benchmark` example, `permission-contract.sh`, binary command tests, FFI integration tests, release binary build + version-flag check + 15MB size check, FFI cdylib build (`release-ffi` profile), FFI helper-discovery smoke, npm package tests + wrapper smoke |
| `ffi-python-smoke` | macos-latest | Builds the FFI dylib with the `stub-adapter` feature, runs `tests/ffi-python/smoke.py` against it |
| `ffi-header-drift` | macos-latest | `cbindgen 0.29.4 --verify` against the committed `crates/ffi/include/agent_desktop.h` |
| `ffi-panic-guard` | macos-latest | Asserts `profile.release-ffi` keeps `panic = "unwind"`; runs `crates/ffi/tests/run_cdylib_panic_probe.sh` |
| `ffi-passthrough` | ubuntu-latest | `cargo test -p agent-desktop-ffi --features stub-adapter --test c_abi_passthrough` — Family-B entrypoints (`ad_snapshot` / `ad_status` / `ad_wait` / `ad_execute_by_ref` / `ad_version` + `ad_init` / `ad_destroy` / `ad_check_permissions`) against the stub adapter |

Three more workflows run outside `ci.yml`: `native-e2e.yml` (workflow_dispatch only, self-hosted `[self-hosted, macOS, agent-desktop-e2e]` — builds and runs the exclusive native E2E suite via `scripts/run-native-e2e-ci.sh`), `codeql.yml` (pull_request + push to main/master + weekly cron + workflow_dispatch — CodeQL analysis for GitHub Actions/JS-TS and Rust), and `supply-chain.yml` (pull_request + push to main/master + weekly cron + workflow_dispatch — release-metadata consistency, npm package/publish policy, `cargo-deny`, `zizmor` workflow security audit). `release.yml` (push to main + workflow_dispatch) runs `release-please`, the per-target `build` and `build-ffi` matrices, `ffi-release-gates`, and the `publish-github` / `publish-npm` / `publish-skills` jobs.

Every substantive change also runs the performance baseline gate — `bash scripts/perf-baseline-compare.sh` against the merge-base, with the resulting `report.html` reviewed for unintentional latency deltas before merge. This is part of `CLAUDE.md`'s Definition of Done as of the Phase 1.6 branch tip, and is treated as a per-sub-phase gate in Phase 2/3 (see [Platform Delivery Model](#platform-delivery-model--sub-phases-and-integration-branches)).

### Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `clap` | 4.6 | CLI parsing with derive macros |
| `serde` + `serde_json` | 1.x | JSON serialization |
| `thiserror` | 2.0 | Error derive macros |
| `tracing` | 0.1+ | Structured logging |
| `tracing-subscriber` | 0.3 | env-filter log formatter |
| `rustc-hash` | 2.1 | Faster hashing for ref maps and visited sets |
| `smallvec` | 1.13 | Small fixed-size vectors in hot paths |
| `base64` | 0.22+ | Screenshot encoding |
| `accessibility-sys` | 0.2.0 | macOS AXUIElement FFI |
| `core-foundation` | 0.10.1 | macOS CF types |
| `core-foundation-sys` | 0.8.7 | macOS CF FFI |
| `core-graphics` | 0.25.0 | macOS CG types |

### Documentation Delivered

- [x] README with installation (npm + source), core workflow, command reference, JSON output, ref system, platform support table
- [x] Architecture diagram
- [x] Agent skills: `skills/agent-desktop/` (core + macOS references) and `skills/agent-desktop-ffi/`

---

## Phase 1.5 — FFI Distribution (C-ABI cdylib)

**Status: Completed — v0.1.13 (2026-04-17).** The C-ABI surface itself (load-time handshake, session-scoped adapter, JSON-envelope command entrypoints) completed later, in v0.4.1.

Phase 1.5 ships `crates/ffi/` as a first-class distribution target. The CLI stays the primary surface; the cdylib lets Python (ctypes), Swift, Node (ffi-napi), Go (cgo), Ruby (fiddle), and C consumers call `PlatformAdapter` directly without spawning `agent-desktop` per call.

### Objectives

| ID | Objective | Metric |
|----|-----------|--------|
| P1.5-O1 | Stable C-ABI surface | `crates/ffi/include/agent_desktop.h` compiled in CI as the committed ABI contract |
| P1.5-O2 | 5-platform release | Tarballs for aarch64/x86_64 apple-darwin, aarch64/x86_64 unknown-linux-gnu, and x86_64 pc-windows-msvc on every tagged release |
| P1.5-O3 | Panic safety | Dedicated `release-ffi` profile overrides `panic = "abort"` → `"unwind"`; `catch_unwind` wraps every `extern "C"` boundary via `trap_panic` / `trap_panic_ptr` / `trap_panic_const_ptr` / `trap_panic_void` |
| P1.5-O4 | Main-thread safety (macOS) | `require_main_thread()` guard in every build profile; worker-thread call returns `AD_RESULT_ERR_INTERNAL` with a static `'static CStr` message |
| P1.5-O5 | Enum UB immunity | Public ABI struct fields store raw `i32`; every entry validates discriminants at the boundary via `try_from_c_enum!` |
| P1.5-O6 | Out-param zeroing before any guard | Every fallible entry zeroes `*out` before pointer / UTF-8 / main-thread checks, so a worker-thread early return never leaves a stale caller buffer |
| P1.5-O7 | Sigstore build-provenance | `actions/attest-build-provenance@v4.1.0` signs every release artifact; consumers verify with `gh attestation verify <file> --repo <owner>/agent-desktop` |
| P1.5-O8 | Skill documentation | `skills/agent-desktop-ffi/SKILL.md` + references: `build-and-link.md`, `ownership.md`, `threading.md`, `error-handling.md` |
| P1.5-O9 | README surface | "Language bindings (FFI)" section on the project README with platform→artifact table, Python dlopen snippet, and Sigstore verify one-liner |

### Crate Layout

```
crates/ffi/
├── Cargo.toml           # crate-type = ["cdylib", "rlib"]
├── cbindgen.toml        # maintainer-only header regeneration config
├── build.rs             # 5 lines: bakes install_name = @rpath/libagent_desktop_ffi.dylib on macOS only — no codegen step
├── codegen_templates/   # empty, untracked — reserved for the P2-O16 build.rs codegen migration, not wired up today
├── include/
│   └── agent_desktop.h  # committed, drift-checked against the OUT_DIR output; AD_ABI_VERSION_MAJOR = 3
├── src/                 # ad_* extern "C" entrypoints, organized by domain
│   ├── types/           # 34 one-type-per-file modules (AdAction, AdRect, AdWindowList, ...)
│   ├── convert/         # string / rect / window / app / surface / notification helpers
│   ├── tree/            # BFS flat-tree layout (flatten.rs, get.rs, free.rs)
│   ├── actions/         # conversion, resolve, execute, result, native_handle
│   ├── commands/        # 8 hand-written ad_* command-backed wrappers (see below); `generated/` subdir is empty/untracked and not compiled — no `mod generated;` in `lib.rs`
│   ├── apps/ windows/ input/ screenshot/ surfaces/ notifications/ observation/
│   ├── error.rs         # AdResult, errno-style TLS last-error (message/suggestion/platform_detail)
│   ├── ffi_try.rs       # panic boundary helpers (trap_panic_*)
│   ├── enum_validation.rs # try_from_c_enum! macro, fuzz tests
│   └── main_thread.rs   # require_main_thread() guard
├── tests/
│   ├── c_abi_harness.rs    # raw extern "C" decls, enum fuzzing, out-param zeroing, null tolerance
│   ├── c_header_compile.rs # shells out to `cc` to verify every AD_* constant is usable from C
│   ├── c_abi_passthrough.rs # Family-B command entrypoints vs stub adapter (ffi-passthrough CI job)
│   └── error_lifetime.rs   # last-error pointer stability across successful follow-up calls
└── examples/
    └── panic_spike.rs   # demonstrates panic boundary on the release-ffi profile
```

**Command-backed entrypoints** (`crates/ffi/src/commands/`, all 8 hand-written today):

| File | Entrypoint |
|------|------------|
| `execute_by_ref.rs` | `ad_execute_by_ref` |
| `execute_by_ref_timeout.rs` | `ad_execute_by_ref_timeout` |
| `snapshot.rs` | `ad_snapshot` |
| `status.rs` | `ad_status` |
| `trace_export.rs` | `ad_trace_export` |
| `trace_show.rs` | `ad_trace_show` |
| `version.rs` | `ad_version` |
| `wait.rs` | `ad_wait` |

`envelope_out.rs` and `timeout.rs` in the same directory are shared helpers, not entrypoints; `mod.rs` is module glue. The `ffi-passthrough` CI job exercises the original five Family-B entrypoints (`ad_snapshot`, `ad_status`, `ad_wait`, `ad_execute_by_ref`, `ad_version`) plus `ad_init` / `ad_destroy` / `ad_check_permissions` against the stub adapter; `ad_execute_by_ref_timeout`, `ad_trace_export`, and `ad_trace_show` joined the entrypoint set afterward.

`wait`'s core event-wait mode (`--event` / `--window-id`) is intentionally **not** exposed over FFI in this release — `wait_args_from_ffi` (`crates/ffi/src/types/wait_args.rs`) always forwards `event: None` and `window_id: None` to core, documented inline.

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

Current gates in `.github/workflows/ci.yml`:

- `cargo build --profile release-ffi -p agent-desktop-ffi` + FFI integration suites (`c_abi_harness`, `c_header_compile`, `error_lifetime`) inside the main `test` job (macos-latest)
- `ffi-python-smoke` (macos-latest) — Python ctypes smoke harness (`tests/ffi-python/smoke.py`) against a dylib built with the `stub-adapter` feature
- `ffi-header-drift` (macos-latest) — `cbindgen --verify` (pinned 0.29.4) against the committed header; exits non-zero on any diff
- `ffi-panic-guard` (macos-latest) — asserts `panic = "unwind"` in `Cargo.toml` and runs the `panic_spike` example to prove `catch_unwind` survives the `release-ffi` profile
- `ffi-passthrough` (ubuntu-latest) — `--test c_abi_passthrough --features stub-adapter` confirms the command entrypoints round-trip against the stub adapter

There is no `ffi-codegen-drift` job. An earlier draft of this document described one; it does not exist because there is no codegen step to drift-check yet — see Gap Status below.

### New Dependencies

| Crate | Version | Scope | Purpose |
|-------|---------|-------|---------|
| `cbindgen` | maintainer-installed tool, denied in Cargo graph | `scripts/update-ffi-header.sh` only | C header regeneration |
| `libc` | 0.2+ | `crates/ffi` macOS target | `pthread_main_np` for main-thread check |

### Forward Compatibility

- Pre-1.0 the ABI is explicitly unstable; consumers pin the artifact version alongside the cdylib version. `AD_ABI_VERSION_MAJOR` is currently `3` and evolves append-only (Phase 1.6).
- Any new `PlatformAdapter` method that lands in Phase 2/3 must add a matching `ad_*` FFI wrapper in the same PR that adds the adapter method.
- MCP server mode (Phase 4) is a parallel transport, not an FFI consumer — it calls `PlatformAdapter` directly.

### Gap Status (updated against current HEAD on `feat/foundation-playwright-grade-contract`)

**Resolved:**

- `ad_abi_version()` and `ad_init(expected_major)` ship; consumers call `ad_init` after `dlopen` for a runtime compat check. `AD_ABI_VERSION_MAJOR` is currently `3`.
- `ad_snapshot`, `ad_execute_by_ref`, `ad_wait`, `ad_version`, and `ad_status` are exported, joined since by `ad_execute_by_ref_timeout`, `ad_trace_export`, and `ad_trace_show`.
- `ad_set_log_callback(fn(level, msg))` ships; in-process consumers can install a tracing layer for debug output.

**Correction (this section previously claimed the opposite):** all 8 command-backed wrappers under `crates/ffi/src/commands/` are **hand-written today**, not generated. `crates/ffi/build.rs` is 5 lines and only emits the macOS linker `-install_name` argument. `crates/ffi/codegen_templates/` and `crates/ffi/src/commands/generated/` both exist but are empty and untracked, and `crates/ffi/src/lib.rs` has no `mod generated;` — that directory is not compiled. No `src/commands/generated.rs` file exists anywhere in the repository. The deterministic `build.rs`-driven codegen migration remains open work — see P2-O16.

**Still open:**

- No `pyo3` / `maturin` wheel or `cffi` wrapper ships with the repo — the Python consumer path is ctypes. Potential Phase 2 follow-up.
- P2-O16 (below) scopes the full registry migration: `build.rs` codegen walks a compile-time command registry and emits one `ad_<name>` wrapper per command, replacing the current 8 hand-written wrappers.

---

## Phase 1.6 — Playwright-grade Foundation Contract (Completed)

**Status: Completed** — unreleased on `feat/foundation-playwright-grade-contract` (PR #93, `feat!`); will cut the next minor per the pre-1.0 versioning policy in `CLAUDE.md`.

Phase 1.6 is a breaking hardening pass over the Phase 1 contracts, modeled on Playwright's actionability/auto-wait discipline. It does not add a platform or a command surface — every change lands in `crates/core/` (plus the macOS backfill in the same PR) and tightens what the existing 58 command names guarantee. Twenty units (U0–U19) shipped; Windows and Linux (Phase 2/3) inherit this contract rather than re-deriving it.

### What shipped

| Unit | What it is |
|------|------------|
| U0 | Capability-supertrait restructure: `PlatformAdapter` becomes `ObservationOps + ActionOps + InputOps + SystemOps` with a blanket impl; every method defaults to `not_supported()` unless a real default is documented |
| U1 | Canonical role/state vocabulary + a genuinely live `is --property visible` (previously derived from stale tree data) |
| U2 | macOS state-producer expansion onto the new vocabulary |
| U3 | Display contract: `list_displays` command, honest `--screen` targeting, per-display `scale_factor` |
| U4 | Truthful Automation permission — the probe no longer optimistically assumes granted |
| U5 | `native_id` identity spine end-to-end (macOS `AXIdentifier`) |
| U6 | Window identity promoted to a primary key for resolution, with recycled-window-id fail-closed handling |
| U7 | `LocatorQuery` + `resolve_query` + a live `find` command (not snapshot-only) |
| U8 | Default-on auto-wait pre-action gate for every ref action — 5000ms bound, `--timeout-ms 0` restores single-shot behavior (**breaking**) |
| U9 | Three-way `hit_test` / `receives_events` occlusion detection (`ReachesTarget` / `InterceptedBy` / `Unknown`) |
| U10 | `scroll_into_view` promoted to a core-owned contract, not an adapter-local convenience |
| U11 | Core accessible-name computation takes precedence over adapter-supplied `NameEvidence` |
| U12 | `supported_surfaces()` introspection ratifies `SnapshotSurface` as genuinely platform-neutral |
| U13 | Typed `ActionStep` delivery tier (`SemanticApi` vs `PhysicalSynthetic`) with a `verified` flag |
| U14 | `ProcessState` (`Running` / `Exited` / `Crashed` / `Unresponsive`) + `APP_UNRESPONSIVE` error code + envelope 2.1 |
| U15 | `LaunchOptions` (`--arg` / `--env` / `--cwd` / `--no-attach`) |
| U16 | `open_session` adapter-affinity hook returning `Box<dyn AdapterSession>` (Send+Sync) — the landing zone for the Windows COM-MTA worker thread and the Linux D-Bus connection |
| U17 | `SignalBaseline` capture + `diff_signals` + `wait --event` (window-opened/closed, app-launched/terminated, focus-changed, surface-appeared) — an in-invocation baseline-diff, **not** a push subscription |
| U18 | Typed clipboard content (`Text` / `Image` / `FileUrls`); the legacy untyped string clipboard API was removed |
| U19 | Mouse modifier chords + a mouse-wheel primitive |

Also landed in the same branch, cutting across the units above:

- `key-down` / `key-up` / `mouse-down` / `mouse-up` fail closed (`input_hold_policy::reject`) — held input is reserved for the Phase 5 daemon, which is the only thing that can guarantee a matching release.
- FFI ABI major bumped to `3`; the ABI evolves append-only. `wait`'s event-wait mode is intentionally not exposed over FFI yet (documented in `wait_args.rs`).
- Live e2e harness (`tests/e2e/run.sh`) against the SwiftUI/AppKit fixture app, both headless and `--headed`, verify-by-observation, currently 109 checks — plus a performance baseline harness (`scripts/perf-baseline-compare.sh` → `report.html`) and a performance-baseline line in `CLAUDE.md`'s Definition of Done.

### Ref System note

Every command that emits or accepts a ref now uses the snapshot-qualified form `@<snapshot_id>:e<n>` (e.g. `@s8f3k2p9:e5`). The bare legacy form `@e5` is still accepted, but only together with an explicit `--snapshot <id>`. Historical prose earlier in this document that predates the qualification (written when refs were process-global) may still show a bare `@e5` for brevity; treat it as shorthand for the qualified form.

---

## Platform Delivery Model — Sub-phases and Integration Branches

Phase 2 (Windows) and Phase 3 (Linux) do not ship as one monolithic implementation PR. Each platform ships as a sequence of dependency-ordered sub-phases against its own integration branch.

- One integration branch per platform: `feat/windows-adapter`, later `feat/linux-adapter`.
- Each sub-phase is one PR into the integration branch, sized at or under 2,000 changed lines (excluding `Cargo.lock`, the generated FFI header, and vendored fixtures), reviewed on its own.
- **Lifecycle per sub-phase:** plan (via `ce-plan`, written to `docs/plans/`) → implement → per-sub-phase review → merge to the integration branch. There is explicitly **no brainstorming stage** — this document is the finalized product contract; sub-phase planning documents implementation, not product scope.
- **The workspace stays green at every merge.** Unimplemented capabilities return `not_supported()`; the CLI ships honest `PLATFORM_NOT_SUPPORTED` envelopes on the target OS until the capability lands. No sub-phase merge may regress `main`'s CI.
- **Evidence-first rule:** adapter decisions are anchored in committed probe outputs — raw UIA / AT-SPI dumps checked in alongside the sub-phase plan — never docs-only assumptions about how a platform API behaves. The exploration sub-phases (2.0, 3.0) build the initial corpus; later sub-phases extend it.
- **Source-of-truth feedback rule:** this document was authored from documentation research in a single pass; the exploration sub-phases (2.0, 3.0) exist because platform reality outranks documentation. When a committed probe proves a documented approach behaves differently on the real platform — an API that answers differently, a pattern that is unavailable, an event that never fires — the finding ledger entry and the amendment to this document land **in the same PR**. The source of truth tracks proven platform behavior; it is never defended against evidence.
- **Integration branch → main:** only after every sub-phase for that platform has merged. Promotion to `main` runs a full multi-agent review of the whole branch, live e2e (both headless and headed) on the platform runner, a performance-baseline comparison against `main`, and the standard verification contract. It lands as one release-noted `feat!` merge — the same conventional-commit discipline as PR #93.
- **Windows first.** Phase 2 (2.x) completes before Phase 3 (3.x) starts; Linux reuses the same sub-phase template with AT-SPI2/D-Bus substituted for UIA.

Every sub-phase below follows the same rendering shape: **Goal** (one or two sentences), **Scope** (what lands), **Key APIs** (platform surface touched), **Depends on** (prior sub-phase), **Exit criteria** (what proves it's done), **Est. PR size**.

---

## Phase 2 — Windows Adapter

**Status: Planned** — delivered as sub-phases 2.0–2.15 into the `feat/windows-adapter` integration branch per the [Platform Delivery Model](#platform-delivery-model--sub-phases-and-integration-branches). This section is the public objective catalogue, the sub-phase implementation contract, and the preserved research (API mappings, capability maps, notification/tray approaches, Electron guidance) that grounds it.

### Core invariants (research-driven — from the Phase 2 plan's Headless-First Invariant)

1. **Headless-first inside the active desktop session.** Every command — existing and Phase 2 — must run without an agent-desktop GUI, foreground activation, focus steal, or physical cursor movement unless `--headed` explicitly opts into cursor input. Windows, macOS, and Linux still require the target app to exist in the current user's interactive desktop/display session for accessibility and capture APIs. Session 0, Server Core, secure desktops, locked desktops, and other-user sessions return `PLATFORM_NOT_SUPPORTED`, `PERM_DENIED`, or `WINDOW_NOT_FOUND` with `platform_detail`, not silent best effort. The invariant is enforced by integration tests: target window is NOT focused at test entry; `list-windows --focused-only` returns the same window before/after; cursor position unchanged for headless commands.
2. **Skeleton traversal is platform-agnostic.** The novel progressive skeleton pattern (depth-3 clamp + `children_count` annotation + drill-down via `--root @ref` + scoped invalidation via `RefMap::remove_by_root_ref`) lives entirely in `crates/core/src/snapshot_ref.rs`. Windows adapter contributes ~50 LOC glue: `ControlViewWalker` (NOT `RawViewWalker` or `ContentViewWalker`) + `FindAll(TreeScope_Children, TrueCondition)` for `children_count` + fresh `UICacheRequest` per drill-down.
3. **Asymmetric event threading.** The future push-based `watch` command (P2-O11) uses main-thread `AXObserver` on macOS (research-confirmed: Apple DTS says all AX is main-thread-only; AXSwift / Hammerspoon / Phoenix all do this); worker-thread MTA `IUIAutomation` event handler on Windows (Microsoft 2025 threading doc: UIA supports cross-thread event delivery). This is distinct from the already-shipped `wait --event`, which is an in-invocation `SignalBaseline` diff, not a subscription — see the naming note under 2.11 below.
4. **No `inventory` / `linkme` command registry.** Research confirmed neither survives link-GC reliably across ld64, ld-prime, GNU ld, lld, MSVC for cdylib consumers. Any future registry uses `build.rs` filesystem enumeration of `crates/core/src/commands/*.rs` — deterministic, cdylib-safe, zero linker magic. The repository's "one command per file" rule becomes the codegen contract when that migration (P2-O16) lands.
5. **FFI compatibility gates: shipped.** The ABI handshake (`ad_abi_version()`, `ad_init(expected_major)`) shipped in Phase 1.6; `AD_ABI_VERSION_MAJOR` is currently `3` and evolves append-only. Any new cross-platform ABI surface Phase 2 adds must preserve that handshake, not re-invent it.
6. **`DeliverFiles` replaces `FileDrop`.** Headless-first forbids `NSDraggingSession` on macOS; the new action uses a 4-tier fallback (URL scheme → `NSWorkspace.open` with `activates: false` → pasteboard + `Cmd-V` → AppleScript). Windows primary delivery is app/shell delivery (`ShellExecuteEx`, app URI handlers, `IFileOperation` for filesystem destinations, and `CF_HDROP` clipboard paste where accepted). `IDataObject + DoDragDrop` is an explicit policy-gated fallback/spike for targets that require drag semantics; it is never the default headless path.

### Windows Engineering Invariants (from the Phase 2 plan, Unit 3)

1. `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` at startup.
2. `CoInitializeEx(NULL, COINIT_MULTITHREADED)` on main thread and on every dedicated UIA worker thread (UIA prefers MTA).
3. Never cache `IUIAutomationElement` across apartments. Event handlers are created, registered, removed, and drained on the same dedicated MTA thread; worker code re-resolves from `RefEntry` instead of moving elements across apartments.
4. UIA-first, SendInput-fallback (UIA patterns are focus-independent; `SendInput` is focus-dependent + UIPI-blocked for elevated targets).
5. `PostMessage WM_KEYDOWN` is DEAD for Chromium/UWP/games — not a viable alternative.
6. UIPI elevation detection via `GetTokenInformation(TokenIntegrityLevel)`. Ship `uiAccess=true` as optional signed release, not default.
7. `RemoveAutomationEventHandler` with post-remove-barrier pattern (Arc<Handler> outlives final callback dispatch).
8. HRESULT format in `platform_detail`: `COM HRESULT 0x80070005 (E_ACCESSDENIED: Access is denied)`.
9. `PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)` for legacy screenshot (mitigates DWM black frames). `windows-capture` (modern) handles composition correctly.
10. `ElementFromHandle(hwnd)` is headless-safe for same-user, same-session visible/minimized windows at an accessible integrity level — the foundation of observation headlessness.
11. `Windows.Graphics.Capture` requires DWM (Windows 10 1903+) in an active interactive session; returns `PlatformNotSupported` in Session 0, Server Core, secure desktop, or locked/remote sessions where capture is unavailable.
12. Session isolation: cannot drive windows in other user sessions.
13. `SetForegroundWindow` / `SetWindowPos(HWND_TOP)` is allowed only for explicit focus/window commands whose `InteractionPolicy` permits focus steal. It is never a fallback for semantic ref actions.

Phase 2 brings agent-desktop to Windows. It is also the phase that closes the cross-platform feature-parity gaps surfaced after the v0.1.13 FFI ship — shipping Windows meaningfully requires new core abstractions (event subscriptions, text-range primitives, shell surfaces, and Windows-specific tray/taskbar affordances) that Windows UIA exposes natively and the macOS adapter does not yet surface. Phase 1.6 already delivered several of the abstractions this phase originally scoped as net-new — see the "shipped in 1.6 / remaining" split in the objectives table below. Every new trait method added here is implemented on both platforms in the same PR pair when there is a real cross-platform analogue. True Windows shell concepts return `PLATFORM_NOT_SUPPORTED` on other adapters through the same core command path, never through side-channel code. Linux (Phase 3) mirrors the portable parts against AT-SPI2.

Core engine, CLI parser, JSON contract invariants, and command-registration pattern are preserved. What Phase 2 legitimately changes: `AccessibilityNode` field set, `Action` enum variants, `ErrorCode` variants, `PlatformAdapter` trait size. Every new `Action` variant must update core actionability, capability maps, platform dispatch, CLI/FFI conversion, and contract tests in the same change; exhaustive compiler checks are the guard against adapter drift. Every macOS backfill lands atomically with the Windows implementation so the two platforms never drift.

Per the [Command Surface Architecture](#command-surface-architecture-dry-invariant) invariant, every new command added in Phase 2 (`watch`, `text select-range`, `text get-selection`, `text insert-at-caret`, etc.) lives in **exactly one file** under `crates/core/src/commands/` and is wired through the shared typed command path. If Phase 2 adds codegen, it uses deterministic `build.rs` filesystem enumeration, not linker registries. The per-platform work is the `PlatformAdapter` capability-trait method implementations (one each in `crates/macos/`, `crates/windows/`, `crates/linux/`) — nothing repeats across transports.

P2-O16 (FFI registry migration) also migrates the FFI wrappers from hand-written to codegen: a `build.rs` step in `crates/ffi/` walks the registry and emits one `ad_<name>` extern "C" function per command, using the per-type marshaling helpers in `crates/ffi/src/convert/`. After this migration, the FFI crate holds marshaling primitives, not command wrappers. The `crates/mcp/` crate (Phase 4) follows the same walk-the-registry pattern with `rmcp`'s `#[tool]` shape — so Phase 4 can ship its MCP server without hand-maintaining the tool list.

### Objectives

Core + Windows parity (original scope):

| ID | Objective | Metric |
|----|-----------|--------|
| P2-O1 | Windows adapter | `snapshot` on Windows returns valid tree for Explorer, Notepad, Settings |
| P2-O2 | All existing commands cross-platform | Identical JSON contract output on macOS and Windows for every command |
| P2-O3 | Windows input synthesis | `click`, `type`, `press`, all mouse commands working via UIA + SendInput |
| P2-O4 | Windows screenshot | `screenshot` produces PNG via `Windows.Graphics.Capture` API |
| P2-O5 | Windows clipboard | `clipboard-get` / `clipboard-set` / `clipboard-clear` working via typed `ClipboardContent` over the Win32 Clipboard API |
| P2-O6 | Windows CI | GitHub Actions Windows runner executes build, clippy, unit, contract, and non-interactive tests on every PR. UIA/shell integration tests that require Explorer, Start, Action Center, or an unlocked desktop run on a labeled interactive/self-hosted Windows job or are skipped with explicit `PLATFORM_NOT_SUPPORTED` assertions |
| P2-O7 | Windows binary release | Prebuilt `.exe` published via GitHub Releases and npm; Phase 1.5 FFI cdylib for Windows already ships |

Cross-platform core extensions (new, landed alongside Windows — each restated with its shipped/remaining split against Phase 1.6):

| ID | Objective | Metric | Shipped in 1.6 / Remaining |
|----|-----------|--------|------------------------------|
| P2-O8 | Stable-selector expansion | `AccessibilityNode.native_id` remains the portable stable-ID field. Platform adapters preserve their strongest developer ID there (Windows UIA `AutomationId`, macOS `AXIdentifier` or `AXDOMIdentifier`, Linux AT-SPI `accessible-id`), while live locator traversal may retain both native IDs internally for strict matching. Phase 2 may add separately named `subrole`, `role_description`, `placeholder`, and `dom_classes` evidence without renaming or duplicating `native_id`. Resolver tests require controls with explicit IDs to survive re-drills; controls without one continue through the fingerprint fallback | Core contract + macOS `native_id` (`AXIdentifier`) shipped (U5). **Remaining:** Windows `AutomationId` implementation (sub-phase 2.3) |
| P2-O9 | `Action` enum expansion for 2026 agent workloads | New variants: `LongPress { duration_ms }`, `ForceClick`, `ShowMenu`, `DeliverFiles(Vec<PathBuf>)` (renamed from `FileDrop` — the original name implied `NSDraggingSession` which is not headless-compatible on macOS; see Core invariant 6), `WindowRaise`, `Cancel`, `SelectRange { start, len }`, `InsertAtCaret(String)`. `watch` is a new **command**, not an `Action` variant. Each has a macOS AX API mapping (all AX calls on main thread), a Windows UIA pattern mapping, a new CLI subcommand, FFI conversion coverage where applicable, and exhaustive platform-dispatch tests in the same change | **Remaining in full** — none of these variants exist yet; still Phase 2 scope |
| P2-O10 | `ErrorCode` expansion | Consider `PermissionRevoked` (distinct from `PermDenied` — TCC yanked mid-session) and `ResourceExhausted` (refmap >1 MB, tree node-count cap) | `APP_UNRESPONSIVE` shipped (U14) — a failed read-only liveness probe upgrades a hang to `APP_UNRESPONSIVE`; ordinary AX messaging exhaustion still reports plain `TIMEOUT`. **Remaining:** `PermissionRevoked`, `ResourceExhausted` |
| P2-O11 | Event-subscription primitive (push, not poll) | New **command** `watch --event <kind> --ref @s8f3k2p9:e5 --timeout 3000`, backed by a new adapter method distinct from `capture_signal_baseline`. macOS: `AXObserverCreate` + `AXObserverAddNotification` + `CFRunLoopSource`. Windows: `IUIAutomation.AddAutomationEventHandler` + `AddFocusChangedEventHandler` + `AddPropertyChangedEventHandler`. Linux mirrors in Phase 3 via AT-SPI2 D-Bus signals | `wait --event <kind>` (baseline-diff desktop-signal wait, U17) shipped and is **not** this objective — see the naming note under sub-phase 2.11. **Remaining in full:** the push `watch` command itself |
| P2-O12 | Text range primitives | Read caret, read selection, select a range by offsets, read text at range, insert at caret. macOS: `kAXSelectedTextRangeAttribute` (settable), `AXStringForRangeParameterizedAttribute`, `AXBoundsForRangeParameterizedAttribute`, `AXRangeForLineParameterizedAttribute`, `AXValueCreate(kAXValueCFRangeType, …)`. Windows: `TextPattern.GetSelection`, `TextPattern.DocumentRange`, `TextRange.Select`, `TextRange.Move`, `TextRange.GetText`, `TextRange.GetBoundingRectangles`. Commands: `text get-selection`, `text select-range <ref> <start> <len>`, `text insert-at-caret <ref> <string>`, `text at-offset <ref> <start> <len>` | **Remaining in full** — still Phase 2 scope |
| P2-O13 | Modern per-window screenshot APIs | macOS: replace `/usr/sbin/screencapture` subprocess with `SCScreenshotManager.captureImage(contentFilter:config:)` filtered to a specific `CGWindowID` from `SCShareableContent.windows`. Windows: `Windows.Graphics.Capture` via `GraphicsCaptureItem.CreateFromWindowHandle(HWND)` + `Direct3D11CaptureFramePool` when supported by the OS/session. No subprocess on the modern path, explicit fallback to legacy capture when unavailable, and permission/support failures map to structured `PERM_DENIED` / `PLATFORM_NOT_SUPPORTED` with `platform_detail` | `list_displays` + honest `--screen` targeting + `scale_factor` shipped (U3). **Remaining:** ScreenCaptureKit modern macOS capture, `Windows.Graphics.Capture` Windows capture |
| P2-O14 | Toolbar and missing surfaces | Implement the core-predeclared surface vocabulary without changing core: `Toolbar` on both platforms; `Spotlight`, `Dock`, and `MenuBarExtras` on macOS; `Taskbar`, `SystemTray`, `SystemTrayOverflow`, `StartMenu`, `ActionCenter`, and `QuickSettings` on Windows where the current build/session exposes them. `NotificationCenter` remains the portable notification surface while `ActionCenter` names the distinct Windows shell entry point | `supported_surfaces()` introspection shipped (U12) — `SnapshotSurface` is ratified as genuinely platform-neutral, and every variant above already exists as a predeclared enum member. **Remaining:** the Windows shell surface implementations themselves |
| P2-O15 | Electron / WebView2 deep-tree toggles | macOS: `build_subtree` writes `AXEnhancedUserInterface = YES` on app root for known Electron bundle IDs (VS Code, Cursor, Slack post-Sept-2024, Teams, Discord, Figma Desktop, Notion). Windows: detect Edge WebView2 via UIA `ClassName = "Chrome_WidgetWin_1"` and the equivalent flag; apply same web-wrapper depth-skip. Both: new `--force-electron-a11y` CLI override | **Remaining in full** on Windows — macOS depth-skip already exists from Phase 1; the Windows implementation is sub-phase 2.4 |
| P2-O16 | FFI registry migration + parity expansion | Migrate `crates/ffi/` from hand-written `ad_*` wrappers to a `build.rs` codegen step that walks a compile-time command registry and emits one wrapper per command. After this, adding a CLI command automatically produces the FFI entry and the same descriptor metadata can feed JSON Schema / MCP generation in Phase 4. Marshaling helpers stay in `crates/ffi/src/convert/` — per-type, not per-command | **Remaining in full.** The ABI handshake (`ad_abi_version`, `ad_init`) and 8 command-backed entrypoints (`ad_snapshot`, `ad_execute_by_ref`, `ad_execute_by_ref_timeout`, `ad_wait`, `ad_version`, `ad_status`, `ad_trace_export`, `ad_trace_show`) shipped, but all 8 are hand-written — see Phase 1.5 Gap Status. This objective is exactly the codegen migration that turns those hand-written files into generated output |
| P2-O17 | Screen Recording / Automation permission detection | macOS exposes `PermissionReport { accessibility, screen_recording, automation }`. Automation is probed noninteractively for the remaining System Events-backed Notification Center opener; Accessibility and Screen Recording retain explicit preflight states | **Shipped** (U4 truthful Automation permission) |
| P2-O18 | Windows shell surface coverage | Add explicit shell coverage for Start menu/search, taskbar, system tray/overflow, Action Center/notification center, Quick Settings, multi-monitor/DPI, virtual desktop detection, UAC/elevated targets, RDP/locked-session behavior, and Explorer-specific file destinations. New commands are added only where a ref-based `snapshot --surface …` loop cannot expose the surface first; Windows-only behavior still routes through core command files and adapter trait defaults | **Remaining in full** — sub-phase 2.14, explicitly deferrable stretch scope |

### Cross-Platform Trait Extensions

New methods land in the appropriate capability trait under `crates/core/src/adapter/`, with default implementations returning `AdapterError::not_supported(method)`. Windows implements them natively. macOS backfills in the same PR pair. Linux (Phase 3) adds the AT-SPI2 implementations; public trait access remains through the crate-root re-exports in `crates/core/src/lib.rs`.

```rust
impl PlatformAdapter for … {
    // P2-O11 — event subscription (new `watch` command; distinct from the shipped `wait --event`)
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
    fn list_surfaces(&self, process: ProcessIdentity, deadline: Deadline) -> Result<Vec<SurfaceInfo>, AdapterError> // extended kinds via already-shipped SnapshotSurface variants
}
```

New supporting types (land in `crates/core/src/`):

- `EventKind` — `FocusChanged`, `ValueChanged`, `SelectionChanged`, `ChildrenChanged`, `WindowOpened`, `WindowClosed`, `MenuOpened`, `MenuClosed`, `NotificationPosted`, `ElementDestroyed`
- `ElementEvent` — `{ kind, handle_ref_id: Option<String>, timestamp, attr_snapshot: Option<AccessibilityNode> }`
- `TextRange` — `{ start: u32, length: u32 }` (UTF-16 code units to match both AX CFRange and UIA TextRange conventions)
- `TextSelection` — `{ range: TextRange, caret_offset: u32, lines_in_view: Vec<TextRange> }`
- `ScreenshotBackend` — `Modern` (ScreenCaptureKit / Windows.Graphics.Capture / PipeWire) or `Legacy` (preserves the Phase 1 subprocess path as fallback for restricted environments)

`PermissionReport` (`{ accessibility, screen_recording, automation }`, each `{ "state": "granted" | "denied" | "not_required" | "unknown" }`) already shipped in Phase 1 and needs no change here.

### Cross-platform capability map (P2-O8 through O17)

| Capability | macOS API | Windows API | Linux API (Phase 3) |
|------------|-----------|-------------|----------------------|
| Stable `native_id` — **shipped on macOS (U5)** | `kAXIdentifierAttribute` / `AXDOMIdentifier` | UIA `AutomationId` | AT-SPI2 `accessible-id` + GTK `gtk-id` |
| `subrole` | `kAXSubroleAttribute` | UIA `LocalizedControlType` + pattern-based heuristic | AT-SPI2 `role-name` + `state-set` |
| `role_description` | `kAXRoleDescriptionAttribute` | UIA `LocalizedControlType` | AT-SPI2 `role-description` |
| `placeholder` | `kAXPlaceholderValueAttribute` | UIA `HelpText` + `IsTextEditPatternAvailable` placeholder | AT-SPI2 `description` + HTML `placeholder` via `object-attributes` |
| `dom_id` / `dom_classes` | `kAXDOMIdentifierAttribute` / `kAXDOMClassListAttribute` | Edge WebView2 UIA `HtmlId` / `HtmlClass` properties | AT-SPI2 `object-attributes` HTML keys |
| Event subscription (`watch`) | `AXObserverCreate` + `AXObserverAddNotification` on `CFRunLoop` | `IUIAutomation.AddAutomationEventHandler` + `AddFocusChangedEventHandler` + `AddPropertyChangedEventHandler` | AT-SPI2 D-Bus signals via `zbus::StreamFactory` |
| Text range read | `AXStringForRangeParameterizedAttribute` + `AXSelectedTextRangeAttribute` | `TextPattern.GetSelection`, `TextPattern.DocumentRange.GetText` | AT-SPI2 `Text.GetText(start, end)` + `Text.GetCaretOffset` |
| Text range write | `AXSelectedTextRange = AXValueCreate(kAXValueCFRangeType, …)` | `TextRange.Select` + `TextRange.Move` | AT-SPI2 `EditableText.InsertText` + `Text.SetCaretOffset` |
| Modern per-window screenshot | `SCScreenshotManager.captureImage(contentFilter:config:)` | `GraphicsCaptureItem.CreateFromWindowHandle` + `Direct3D11CaptureFramePool` | PipeWire `org.freedesktop.portal.ScreenCast` |
| Toolbar surface — **predeclared in `SnapshotSurface` (U12)** | `AXRole == AXToolbar` or `AXUnifiedTitleAndToolbar` | UIA `ControlType.ToolBar` | AT-SPI2 `Role::ToolBar` |
| Menu-bar extras surface | `SystemUIServer` + `ControlCenter` pid walk | UIA `Shell_TrayWnd` + `NotifyIconOverflowWindow` | AT-SPI2 `StatusNotifierWatcher` D-Bus |
| Dock / taskbar surface | `Dock.app` pid walk | UIA `Shell_TrayWnd` `TaskListButton` children | AT-SPI2 per-DE panel walk |
| `LongPress` | `CGEventCreateMouseEvent(…Down…)` + sleep + `…Up` | `SendInput` hold + release | Coordinate via `ydotool/xdotool` |
| `ForceClick` | `CGEventSetIntegerValueField(kCGMouseEventPressure, …)` + `kCGEventMouseSubtypeTabletPoint` | Pen input `SendInput` with `PEN_FLAGS_BARREL` | Not natively supported — return `ActionNotSupported` |
| `ShowMenu` action | `AXPerformAction(kAXShowMenuAction)` | `ExpandCollapsePattern.Expand` + UIA right-click fallback | AT-SPI2 `Action.DoAction("popup")` |
| `WindowRaise` | `AXUIElementSetAttributeValue(kAXRaiseAction)` | `SetForegroundWindow` + `SetWindowPos(HWND_TOP)` only under explicit focus/window policy | `wmctrl -a` / `xdotool windowactivate` only under explicit focus/window policy |
| `Cancel` | `AXPerformAction(kAXCancelAction)` | UIA `WindowPattern.Close` on dialog or `InvokePattern` on cancel button | AT-SPI2 `Action.DoAction("cancel")` or synthesize Escape |
| `DeliverFiles(Vec<PathBuf>)` | 4-tier headless fallback: (1) app-native URL scheme, (2) `NSWorkspace.open(urls:withApplicationAt:configuration:)` with `activates: false`, (3) `NSPasteboard.public.file-url` + `CGEventPostToPid(cmd+v)`, (4) `osascript open`. NEVER `NSDraggingSession` (not headless-compatible — Core invariant 6) | App/shell delivery first: app URI handlers, `ShellExecuteEx`, `IFileOperation` for filesystem destinations, and `CF_HDROP` clipboard paste where accepted. `IDataObject + DoDragDrop` is policy-gated fallback/spike only | Portal/native file-transfer path where available; XDND is Phase 3 research, not default |
| Screen Recording permission | `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess` | No macOS-style TCC field. Use `GraphicsCaptureSession::IsSupported` / capture API failures to report `not_required`, `unknown`, `PERM_DENIED`, or `PLATFORM_NOT_SUPPORTED` with `platform_detail` | PipeWire portal permission dialog |
| Automation permission — **shipped, truthful (U4)** | Nonprompting System Events probe; explicit request uses the bounded isolated helper | N/A (no equivalent restriction) | N/A |

### Cross-cutting sub-phase DoD

Every sub-phase 2.0–2.15 below is held to the same definition of done, stated once here rather than repeated per sub-phase:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib --workspace`, and the relevant conformance suites (role/state vocabulary, contract tests) are green.
- Probe evidence — raw UIA dumps of the target app — is committed alongside the sub-phase's plan doc under `docs/plans/`.
- Adapters keep `not_supported()` defaults for every capability not yet landed in that sub-phase; the CLI surfaces `PLATFORM_NOT_SUPPORTED` honestly rather than a stub success.
- No core rewrites. Core changes land only via explicitly planned additive trait methods — never a signature change to something Phase 1/1.6 already shipped.
- Each sub-phase gets its own review before merging to `feat/windows-adapter`.
- `scripts/perf-baseline-compare.sh` runs on any sub-phase that touches a hot path (tree traversal, resolution, action dispatch).
- Commits follow the repository's Conventional Commits requirement.

### 2.0 — Platform Exploration & Raw Scripting (pre-Rust)

**Goal:** empirically map Windows accessibility reality with raw, no-Rust scripts before any adapter code exists, producing a committed evidence corpus the Rust sub-phases implement against — and feeding every contradiction back into this document.

**Scope:** a `probes/windows/` directory of raw scripts — PowerShell using .NET managed UIA (`System.Windows.Automation`, preinstalled with .NET Framework 4.8) plus small C# programs compiled with `csc.exe` where UIA3 COM specifics differ from the managed wrapper. The corpus must cover, each as a runnable script with captured JSON/text output committed beside it: (1) full-tree dumps of Notepad, Explorer, Settings, and one Electron app (VS Code or Slack) including every property read per node; (2) a pattern-availability census per ControlType (Invoke, Toggle, Value, RangeValue, ExpandCollapse, SelectionItem, Scroll, ScrollItem, Text, Window, LegacyIAccessible); (3) every interaction exercised raw — invoke, toggle, set value, select, expand/collapse, scroll via pattern AND wheel, text get/selection/caret/insert, focus; (4) SendInput synthesis experiments — keyboard incl. modifier chords and UTF-16 chunking limits, mouse click/move/wheel/drag; (5) `ElementFromPoint` hit-testing incl. deliberately occluded and zero-size targets; (6) `CacheRequest` batched reads timed against per-property reads; (7) AutomationId coverage census across Win32 / WinForms / WPF / Electron; (8) event-handler observations (which UIA events actually fire, ordering, MTA threading behavior); (9) elevation/UIPI behavior against an elevated process; (10) RDP-session and DPI/multi-monitor bounds behavior; (11) private-file I/O primitives — whether atomic rename over a concurrently-open handle requires `FILE_SHARE_DELETE`, whether an elevated process owns new objects as `TokenOwner` (e.g. `BUILTIN\Administrators`) rather than `TokenUser`, whether `GetFileInformationByHandleEx(FileRemoteProtocolInfo)` reliably distinguishes local from remote volumes, and what ancestor-vs-leaf ACL validation contract parity with the unix leaf-only rule actually needs (see `docs/solutions/best-practices/never-ship-platform-code-that-ci-cannot-execute.md`). Alongside the scripts: `probes/windows/FINDINGS.md` — a findings ledger mapping every experiment to observed behavior and a doc-alignment verdict (confirms this document / contradicts it / new edge case).

**Key APIs:** System.Windows.Automation, UIA3 COM (IUIAutomation) via csc.exe shims, SendInput, ElementFromPoint, CacheRequest.

**Depends on:** nothing — this is the entry point; the dev VM needs only its preinstalled .NET, PowerShell, and git.

**Exit criteria:** the script corpus and captured outputs are committed and re-runnable on the dev VM; the findings ledger covers tree, patterns, interactions, input, hit-testing, batching, identity, events, elevation, session, and private-file I/O behavior with no open "unknown" rows; every ledger entry that contradicts this document has a matching amendment to this document landed in the same PR (see the source-of-truth feedback rule in the Platform Delivery Model); no Rust adapter sub-phase (2.2 onward) starts until this exit gate is green.

**Est. PR size:** ~1.5k lines (scripts + ledger; no Rust).

### 2.1 — Toolchain, CI & COM Bootstrap

**Goal:** Stand up the Windows build/CI/session substrate so every later sub-phase lands on green CI and a constructible (if functionally empty) `WindowsAdapter`.

**Scope:**
- `windows-latest` CI job: build + clippy + lib tests, core-isolation check, size check (mirrors the existing `platform-check` matrix job, promoted to a real Windows test lane)
- Self-hosted interactive Windows runner registration for later UIA/shell integration tests, with RDP/session-isolation documented (an interactive session is required for UIA to see a real desktop; `tscon` is the documented console-reattach workaround — see Risk Register)
- `CoInitializeEx(NULL, COINIT_MULTITHREADED)` + `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` bootstrap at process start
- `WindowsAdapterSession` implementing `AdapterSession` via `open_session` — owns COM apartment state so later sub-phases don't reinvent COM lifecycle
- Re-verify the dependency pins below against crates.io + supply-chain policy (pinned at 2026-04 research time — see New Dependencies)
- Implement Windows private-file hardening from scratch, behind `PlatformAdapter` or as a Windows-gated dependency of the `agent-desktop-windows` crate — never as unconditional `agent-desktop-core` surface (see `docs/solutions/best-practices/never-ship-platform-code-that-ci-cannot-execute.md`) — satisfying, with evidence from 2.0's probes: `FILE_SHARE_DELETE` on every concurrently-open handle across an atomic replace; owner validation against `TokenOwner`, not `TokenUser`; no locality inference from `FileRemoteProtocolInfo`; and an ancestor-vs-leaf validation contract decided deliberately, matching or explicitly diverging from the unix leaf-only rule, so the private-artifact-writing path is real before any Windows code writes a refmap or trace file

**Key APIs:** `CoInitializeEx`, `SetProcessDpiAwarenessContext`, Win32 ACL / `TokenOwner` validation, `FILE_SHARE_DELETE` (private-file hardening)

**Depends on:** nothing (opening sub-phase)

**Exit criteria:** workspace green on Windows CI; `WindowsAdapter` constructs and satisfies the trait; every command returns honest `PLATFORM_NOT_SUPPORTED` on Windows; the permission probe is unit-tested against mocked COM security state; private-file hardening is unit-tested on the `windows-latest` CI lane, not merely `cargo check`-clean.

**Est. PR size:** ~1.3k LOC (bootstrap ~0.8k + from-scratch private-file hardening ~0.5k)

### 2.2 — UIA Element Wrapper & Tree Walk

**Goal:** Own an `AXElement`-equivalent wrapper for UIA elements and prove raw tree traversal against a real Windows app before any semantics land on top.

**Scope:**
- `UIAElement` ownership wrapper — `AddRef`/`Release`, `Clone`/`Drop` safety mirroring the `AXElement` pattern (`pub(crate)` inner field to prevent double-free via raw pointer extraction)
- `ElementFromHandle` roots for window entry
- `TreeWalker` traversal with an ancestor-path cycle guard (mirrors macOS: reused pointers across sibling branches, not a global visited set)
- `CacheRequest` batched attribute reads (the UIA analogue of `AXUIElementCopyMultipleAttributeValues`)
- Committed probe examples: raw UIA dumps of Notepad and Explorer, checked in as evidence alongside the sub-phase plan

**Key APIs:** `IUIAutomation.ElementFromHandle()`, `IUIAutomationTreeWalker.GetFirstChild`/`GetNextSibling`, `CacheRequest` (`uiautomation` crate 0.24+ wrapping the `windows` crate's COM bindings)

**Depends on:** 2.1

**Exit criteria:** an internal tree-dump binary prints Notepad and Explorer trees with batched reads; `CacheRequest` attribute-batching correctness is unit-tested.

**Est. PR size:** ~2k LOC

### 2.3 — Vocabulary: Roles, States, native_id, Name Evidence

**Goal:** Map UIA's vocabulary onto the canonical role/state contract Phase 1.6 (U1/U2) already established, and wire `native_id` end-to-end for Windows.

**Scope:**
- `ControlType` → unified role enum map in `tree/roles.rs`
- UIA states → canonical state vocabulary — must pass the same conformance tests U1/U2 already run for macOS, parameterized over `WindowsAdapter`
- `AutomationId` → `native_id` (completes P2-O8 on Windows; macOS/core side already shipped)
- `NameEvidence` supplier feeding the core `accname.rs` computation (`LabeledBy`, `Name`, `HelpText` precedence evidence supplied by the adapter; core computes the final name, per U11)

**Key APIs:** UIA `ControlType` enum, UIA property IDs for state, `AutomationId`, `Name`/`LabeledBy`/`HelpText` properties

**Depends on:** 2.2

**Exit criteria:** vocabulary conformance tests span every UIA `ControlType` (complete mapping coverage, not a sample) and accname tests pass on Windows.

**Est. PR size:** ~1.5k LOC

### 2.4 — Observation: Snapshot, Windows, Apps, Displays

**Goal:** Land the full read path — `snapshot`, `list-windows`, `list-apps`, `list-displays`, `focused_window` — including the Electron/WebView2 web-wrapper depth-skip that makes dense apps usable.

**Scope:**
- `get_tree` / `get_subtree` wired to the shared `SnapshotEngine`
- Skeleton glue: `ControlViewWalker` (not `RawViewWalker`/`ContentViewWalker`) + `FindAll(TreeScope_Children, TrueCondition)` for `children_count` + fresh `UICacheRequest` per drill-down (Core invariant 2 — ~50 LOC, core owns the rest)
- `list_windows` — HWND-first identity with recycled-window-id corroboration (mirrors the macOS U6 window-identity-as-primary-key pattern)
- `list_apps`, `focused_window`, `list_displays` + per-monitor DPI `scale_factor`
- **Web/Electron web-wrapper depth-skip** (Windows implementation of the pattern macOS already ships): non-semantic wrapper elements (`UIA_GroupControlTypeId` / `UIA_CustomControlTypeId`) with empty `Name` AND empty `Value` do not consume depth budget. Without this, default `--max-depth 10` finds ~3 refs in Slack; with it, 100+. Implement in `crates/windows/src/tree/builder.rs` as `is_web_wrapper`, matching the macOS logic
- **Chromium detection:** detect Chromium-based windows via UIA process name or `Chrome_WidgetWin_1` window class; if the tree is empty/minimal, the error `platform_detail` guides toward `--force-renderer-accessibility`
- **Resolver depth:** element re-identification searches to `ABSOLUTE_MAX_DEPTH` (50) — Electron elements commonly sit at depth 25+; implement in `crates/windows/src/tree/resolve.rs`
- **Surface detection for Electron:** an Electron modal (file picker, dialog) may report as the focused window itself rather than a child; surface detection must check whether the focused window IS the target surface, checking both `ControlType` and `LocalizedControlType`/UIA patterns (analogous to macOS AXRole + AXSubrole); implement in `crates/windows/src/tree/surfaces.rs`
- Progressive skeleton traversal (`--skeleton`, `--root`) needs no Windows-specific work beyond `get_subtree()` — core owns the flow

**Key APIs:** `ControlViewWalker`, `FindAll`, `UICacheRequest`, `Chrome_WidgetWin_1` class match, `IVirtualDesktopManager` (read-only, for diagnostics)

**Depends on:** 2.3

**Exit criteria:** `snapshot --app Notepad -i` and Explorer return reffed trees on the runner; skeleton drill-down works; a VS Code snapshot at default depth finds 50+ refs through web-aware depth-skip alone (no force flag), and ≥100 refs with `--force-electron-a11y`; an Electron file-picker dialog is detected as a sheet surface.

**Est. PR size:** ~2k LOC

### 2.5 — Resolution & Live Locator

**Goal:** Make refs and the live `find`/`get`/`is` commands (U7) work on Windows with the same strict-resolution guarantees macOS ships.

**Scope:**
- `resolve_element_strict*` from `RefEntry` evidence — `AutomationId`-first, fingerprint fallback, 0/1/N classification into `STALE_REF`/success/`AMBIGUOUS_TARGET`
- `get_live_value` / `get_live_state` / `get_live_actions` / `get_live_element` / `get_element_bounds`
- `resolve_query` — the `LocatorQuery` evaluator backing live `find`
- `resolve_locator_anchor` + selected-hydration completeness: actions that read state must classify a **definitive absence** separately from a **transport failure** — port the lesson already encoded in macOS's `is_definitive_absence` (`crates/macos/src/tree/action_list.rs`) rather than re-deriving it from scratch

**Key APIs:** `AutomationId` lookup, `CacheRequest`-scoped re-reads, UIA property read failure classification (COM HRESULT vs "element gone")

**Depends on:** 2.4

**Exit criteria:** `find`/`get`/`is` are live on Windows; `STALE_REF`/`AMBIGUOUS_TARGET` semantics are proven with committed probe evidence (0/1/N candidate cases).

**Est. PR size:** ~2k LOC

### 2.6 — Actionability & Occlusion

**Goal:** Port the Phase 1.6 auto-wait/occlusion gate (U8/U9) onto Windows so every ref action gets the same actionability guarantees before it fires.

**Scope:**
- `hit_test` three-way result via `ElementFromPoint` + window corroboration — `Unknown` on probe failure, never a false negative, matching `HitTestResult`'s `ReachesTarget | InterceptedBy | Unknown` contract
- `receives_events` evidence
- Visibility / enabled / offscreen evidence feeding the core auto-wait gate (U8) — no Windows-specific auto-wait logic; core drives the loop, the adapter supplies live reads
- `scroll_into_view` via `ScrollItemPattern`

**Key APIs:** `ElementFromPoint`, `ScrollItemPattern.ScrollIntoView`

**Depends on:** 2.5

**Exit criteria:** zero-bounds / disabled / occluded fixture cases produce the same envelopes as macOS (same error codes, same `disposition`).

**Est. PR size:** ~1.2k LOC

### 2.7 — Semantic Action Tier

**Goal:** Land the UIA pattern-based semantic action dispatch, with the same typed `ActionStep` delivery reporting (U13) macOS already produces.

**Scope:**
- `perform_action` via UIA patterns: `InvokePattern` (click), `TogglePattern` (toggle/check/uncheck), `ValuePattern` (set-value), `ExpandCollapsePattern` (expand/collapse), `SelectionItemPattern` (select), `RangeValuePattern`, `ScrollPattern`
- Activation chain with `ActionStep` delivery reporting + post-verification reads — honest `verified: Option<bool>` semantics, no step claims an effect it did not observe
- See the Windows API Mapping table below for the full click/set-text/expand/select/toggle/scroll pattern list

**Key APIs:** `InvokePattern.Invoke()`, `TogglePattern.Toggle()`, `ValuePattern.SetValue()`, `ExpandCollapsePattern.Expand()/.Collapse()`, `SelectionItemPattern.Select()`, `ScrollPattern.Scroll()`/`.SetScrollPercent()`

**Depends on:** 2.6

**Exit criteria:** click / set-value / clear / select / toggle / expand / collapse work headless on the fixture app via the e2e analog (sub-phase 2.12 supplies the fixture; this sub-phase's own tests use ad hoc Notepad/Explorer targets until then).

**Est. PR size:** ~2k LOC

#### Windows API Mapping (reference table for sub-phases 2.2–2.10)

| Capability | Technology | Details |
|------------|-----------|---------|
| Tree root | `IUIAutomation.ElementFromHandle()` | Via `uiautomation` crate (v0.24+) wrapping UIA COM APIs via `windows` crate |
| Children | `IUIAutomationTreeWalker.GetFirstChild` / `GetNextSibling` | With `CacheRequest` for batch attribute retrieval (3-5x faster) |
| Role mapping | `UIA ControlType` integers | Map to unified role enum in `tree/roles.rs` — e.g. `UIA_ButtonControlTypeId` → `button` |
| Click | `InvokePattern.Invoke()` | Pattern-based; coordinate click via SendInput only under explicit physical policy |
| Set text | `ValuePattern.SetValue()` | Headless value write by default; SendInput only under explicit focus/physical policy |
| Expand/Collapse | `ExpandCollapsePattern.Expand()` / `.Collapse()` | Native UIA pattern |
| Select | `SelectionItemPattern.Select()` | For combobox, listbox, tab items |
| Toggle | `TogglePattern.Toggle()` | For checkboxes, switches |
| Scroll | `ScrollPattern.Scroll()` / `ScrollPattern.SetScrollPercent()` | Native UIA scroll; mouse wheel only under explicit physical policy |
| Keyboard | `SendInput` API | `INPUT_KEYBOARD` structs with virtual key codes and scan codes |
| Mouse | `SendInput` API | `INPUT_MOUSE` structs with `MOUSEEVENTF_*` flags |
| Clipboard | `OpenClipboard` / `GetClipboardData` / `SetClipboardData` | Win32 APIs; `CF_UNICODETEXT` for text, `CF_DIB`/PNG for image, `CF_HDROP` for file lists — marshaled through typed `ClipboardContent` |
| Screenshot | `Windows.Graphics.Capture` | Modern per-window capture via `GraphicsCaptureItem.CreateFromWindowHandle` + `Direct3D11CaptureFramePool` when WGC is supported by the OS/session. No subprocess, respects DWM compositing. `BitBlt` / `PrintWindow` retained as `ScreenshotBackend::Legacy` fallback for pre-Windows-10 1903 or unavailable WGC environments |
| App launch | `CreateProcess` / `ShellExecuteEx` | Launch by name or path via `LaunchOptions` (args/env/cwd/attach-if-running), wait for main window |
| App close | `WM_CLOSE` / `TerminateProcess` | Graceful close first, force kill with `--force`; verified via `ProcessState` |
| Window ops | `SetWindowPos` / `ShowWindow` | Resize, move, minimize (`SW_MINIMIZE`), maximize (`SW_MAXIMIZE`), restore (`SW_RESTORE`) |
| Permissions | COM security / UAC | Detect elevation requirements; return `PERM_DENIED` if UIA access blocked |
| Notifications | UserNotificationListener + UIA Action Center fallback | See Notification Management approach under 2.14 |
| System tray | UIA + Shell_TrayWnd | See System Tray approach under 2.14 |
| Start menu / search | UIA + explicit shell open command | See Windows-specific command surface under 2.14 |
| Taskbar | UIA + Shell_TrayWnd task list | See Windows-specific command surface under 2.14 |
| Quick Settings | UIA shell flyout | See Windows-specific command surface under 2.14 |
| Virtual desktops | `IVirtualDesktopManager` detection | Public COM detection for "current desktop" filtering and diagnostics only. Moving windows between virtual desktops is deferred unless a stable public API path is validated |
| Multi-monitor / DPI | Per-monitor DPI + Win32 monitor APIs | All bounds are physical pixels normalized by the same DPI-aware process mode; tests cover mixed-DPI monitor layouts before any coordinate fallback ships |

### 2.8 — Input Synthesis

**Goal:** Land raw OS input (keyboard, mouse, drag) matching the macOS delivery-tracking and headed/headless policy contract.

**Scope:**
- `SendInput` keyboard map + `type_text` (UTF-16 chunking for surrogate pairs)
- Mouse events + modifier chords + wheel (mirrors macOS U19)
- Drag with delivery tracking + release guard
- Headed/headless policy parity — raw cursor commands (`hover`, `drag`, `mouse-*`) require `--headed`, same as macOS
- UIPI elevation detection (`GetTokenInformation(TokenIntegrityLevel)`) → `PERM_DENIED` with `platform_detail` in the `COM HRESULT 0x80070005 (E_ACCESSDENIED: ...)` format

**Key APIs:** `SendInput` (`INPUT_KEYBOARD` / `INPUT_MOUSE`), `GetTokenInformation`

**Depends on:** 2.7

**Exit criteria:** headed e2e gesture cases pass (once 2.12's fixture exists; interim coverage via Notepad/Explorer).

**Est. PR size:** ~2k LOC

### 2.9 — System Lifecycle

**Goal:** Land process/window lifecycle — launch, close, resize/move/minimize/maximize/restore — with the same `ProcessState` liveness contract (U14) macOS ships.

**Scope:**
- `launch_app` with `LaunchOptions` (`CreateProcess` args/env/cwd, attach-vs-fail policy via `attach_if_running`)
- `close_app` with verified termination (not an optimistic `closed: true` before the process actually exits — mirrors the v0.3.0 macOS `close-app` correction)
- `window_op` (`SetWindowPos`/`ShowWindow` for resize/move/minimize/maximize/restore)
- `ProcessState` probes: `IsHungAppWindow`/`SendMessageTimeout` → `Unresponsive`; exit-code inspection → `Exited`/`Crashed`
- `is_protected_process`
- `press_key_for_app` under the same focus policy macOS enforces

**Key APIs:** `CreateProcess`, `TerminateProcess`, `SetWindowPos`, `ShowWindow`, `IsHungAppWindow`, `SendMessageTimeout`

**Depends on:** 2.4 (window identity), 2.8 (input for `press_key_for_app`)

**Exit criteria:** lifecycle e2e (launch → interact → close) passes; `APP_UNRESPONSIVE` is reachable against a deliberately hung fixture window.

**Est. PR size:** ~1.8k LOC

### 2.10 — Capture & Clipboard

**Goal:** Ship screenshot and typed clipboard, with the modern-capture-first / legacy-fallback split P2-O13 specifies.

**Scope:**
- `ScreenshotBackend::Legacy` first (`PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)` — mitigates DWM black frames), then `ScreenshotBackend::Modern` via `windows-capture`/WGC where the session supports it (P2-O13)
- `screenshot --screen` honest display targeting (pairs with `list_displays` from 2.4)
- Typed clipboard: `CF_UNICODETEXT` → `ClipboardContent::Text`, image via `CF_DIB`/PNG → `ClipboardContent::Image`, `CF_HDROP` file lists → `ClipboardContent::FileUrls`, all written through 0600-equivalent private files (see 2.1's private-file hardening)

**Key APIs:** `PrintWindow`, `windows-capture`/`Windows.Graphics.Capture`, `OpenClipboard`/`GetClipboardData`/`SetClipboardData`

**Depends on:** 2.1 (private-file hardening), 2.4 (displays)

**Exit criteria:** screenshot + clipboard e2e pass (clipboard tests hermetic, no real-desktop clipboard pollution); WGC gracefully degrades (falls back to `Legacy`, does not fail) in RDP-limited sessions.

**Est. PR size:** ~1.8k LOC

### 2.11 — Signals & Wait Parity

**Goal:** Port `SignalBaseline`/`diff_signals`/`wait --event` (U17) to Windows so the existing `wait` command works identically cross-platform — this is explicitly NOT the future push-based `watch` command (P2-O11).

**Scope:**
- Windows `SignalBaseline` producers: windows / apps / focus / surfaces
- `wait --event` parity including `surface-appeared`
- Wait utilities operating within `Deadline` budgets, matching the core-owned deadline propagation

**Naming note:** `wait --event <kind>` is the already-shipped baseline-diff desktop-signal wait (U17) — an in-invocation snapshot-diff, not a subscription. The future push element-event subscription primitive (P2-O11) is a **different, not-yet-built command** named `watch` (e.g. `watch --event value-changed --ref @s8f3k2p9:e5`), landing later in this sub-phase sequence once `watch_element` exists on the adapter trait. Do not conflate the two.

**Key APIs:** UIA property snapshots for baseline capture (no event handlers yet — that's `watch`, still future)

**Depends on:** 2.4 (windows/apps/displays), 2.9 (process lifecycle for app-launched/terminated signals)

**Exit criteria:** an AE6-analog e2e passes — an unnamed dialog is discovered purely by baseline diff.

**Est. PR size:** ~1k LOC

### 2.12 — Fixture App & Live E2E Harness

**Goal:** Give Windows the same verify-by-observation live e2e discipline the macOS SwiftUI fixture already provides.

**Scope:**
- WinForms fixture app compiled with `csc.exe` (.NET 4.8 preinstalled on the runner — no new toolchain dependency)
- `AutomationId` set on every interactive target from day one (unlike macOS, which had to retrofit `AXIdentifier` — Windows gets this right from the start)
- Fixture targets mirroring `AgentDeskFixture.swift`: delayed-enable, zero-bounds, duplicate-title, occlusion, disclosure
- Harness port (bash via Git-Bash, or a PowerShell driver) asserting every effect by independent re-observation, never the command's own `ok:true` — same contract as `tests/e2e/run.sh`
- `windows-e2e` workflow_dispatch job on the self-hosted interactive Windows runner (registered in 2.1)

**Key APIs:** `csc.exe`, WinForms `AutomationProperties.AutomationId`

**Depends on:** 2.7, 2.8, 2.9, 2.10, 2.11 (everything the harness exercises)

**Exit criteria:** the full Windows live gate is green in both headless and headed tiers.

**Est. PR size:** ~2k LOC (mostly C#/scripts, not adapter Rust)

### 2.13 — FFI, npm, Release

**Goal:** Make the Windows adapter reachable through every distribution channel that already ships for macOS.

**Scope:**
- FFI real-adapter path validated on Windows (non-stub tests — the stub-adapter tests already run cross-platform in CI, but the real `WindowsAdapter` behind the C ABI needs its own pass)
- npm `postinstall.js` gains a `win32-x64` (+ `win32-arm64` once an ARM64 runner exists) branch
- Release matrix: `.exe` zip + attestation, using the same tarball + sha256 + Sigstore pipeline Phase 1.5 already ships
- `skills/agent-desktop-windows/SKILL.md` — see Skill Update below
- README platform table: Windows column → **Yes**

**Key APIs:** none new — this sub-phase is packaging, not adapter code

**Depends on:** 2.2 through 2.11 (needs a working adapter to package)

**Exit criteria:** `npm install -g` works on Windows; release dry-run artifacts verified.

**Est. PR size:** ~1.2k LOC

### 2.14 — Shell Surfaces & Notifications Spike (stretch, explicitly deferrable)

**Goal:** Cover the Windows-only shell surface (P2-O18) and notification/tray (P2-O14 Windows half) scope that has no macOS analogue to backfill against.

**Scope:** P2-O18 shell coverage, notification management, and system tray — all three folded in below rather than duplicated. This sub-phase **may ship after the 2.15 integration merge as its own follow-up `feat`** without blocking Phase 3 — it is explicitly the stretch/deferrable sub-phase in the sequence.

**Key APIs:** see the three subsections immediately below.

**Depends on:** 2.4 (observation), 2.7 (semantic actions)

**Exit criteria:** `open-system-surface --surface <kind>` + `snapshot --surface <kind>` round-trips for Start menu, taskbar, Quick Settings, and Action Center where the current shell exposes them, with explicit `PLATFORM_NOT_SUPPORTED` assertions (clear `platform_detail`) where it does not; notification list/dismiss/action work through at least one of the two documented paths; tray list/click work through SNI-equivalent UIA traversal.

**Est. PR size:** ~2k LOC

#### Windows-specific command surface (P2-O18)

Windows-specific commands are allowed when the operating-system concept has no portable equivalent, but they still follow the repository rules: one core command file, typed CLI/batch dispatch, adapter trait default returning `PLATFORM_NOT_SUPPORTED`, skill docs, and tests. The preferred path remains generic: expose shell UI as a surface, then let agents interact with refs.

| Command | Purpose | Platform behavior |
|---------|---------|-------------------|
| `open-system-surface --surface <kind>` | Opens an OS shell surface so agents can immediately call `snapshot --surface <kind>` and act by refs | Windows kinds: `start-menu`, `taskbar`, `system-tray`, `system-tray-overflow`, `action-center`, `quick-settings`. macOS may support `spotlight`, `dock`, `menu-bar-extras`, `notification-center`. Unsupported kinds return `PLATFORM_NOT_SUPPORTED` |
| `list-tray-items` / `click-tray-item` / `open-tray-menu` | Structured tray workflows where the shell surface is not attached to a normal app window | Windows implementation uses `Shell_TrayWnd` / `NotifyIconOverflowWindow`; macOS maps to menu bar extras. Linux maps to StatusNotifier in Phase 3 |

No Windows-specific command bypasses refs for ordinary app controls. If a Windows workflow can be represented as `snapshot --app`, `snapshot --surface`, `find`, `click`, `type`, `press`, or `wait`, it uses the existing command surface.

#### Notification Management (Windows Implementation)

Windows notification management is built from scratch here. The macOS notification implementation (completed as a Phase 1 follow-up) is the reference pattern — same `PlatformAdapter` trait methods (`list_notifications`, `dismiss_notification`, `dismiss_all_notifications`, `notification_action`), same JSON output contract, same 1-based indexing. Full parity is gated on this being a spike because Windows has two materially different surfaces: notification-listener APIs that require user permission/app identity, and shell UIA traversal that is best effort.

- **Primary list path:** `UserNotificationListener` when package identity/capability and explicit user permission are available. If permission is denied, return `PERM_DENIED` with a permission-specific suggestion.
- **Fallback list path:** open Action Center with `open-system-surface --surface action-center`; traverse exposed shell UIA elements only when they provide stable names/descriptions/action buttons.
- **Dismiss:** prefer notification-listener APIs where supported; otherwise invoke the notification's dismiss/close button through UIA. For "dismiss all", invoke the shell's "Clear all" control only when present.
- **Interact with actions:** resolve action buttons within the notification tree and invoke via the primary API or `InvokePattern`.
- **Focus Assist / Do Not Disturb:** query through supported shell APIs first; registry/WNF probes are best-effort diagnostics, not the sole source of truth.
- **Edge case:** some notifications are transient (disappear after timeout). `wait --notification` monitors via event subscription where supported; otherwise it polls the notification-listener or Action Center fallback within the normal wait deadline.

#### System Tray (Windows Implementation)

System tray interaction is built from scratch here.

- **List items:** UIA tree of the `Shell_TrayWnd` window class; tray items are children of the notification area. Overflow items live in `NotifyIconOverflowWindow`.
- **Click:** `InvokePattern` on tray items, falling back to coordinate-based `SendInput` for items that don't expose UIA patterns.
- **Open menu:** after clicking a tray item, detect the resulting popup menu via UIA focus-changed events and expose it for ref-based interaction.

### 2.15 — Hardening & Integration Review

**Goal:** Prove the assembled `feat/windows-adapter` branch is production-grade as a whole, then merge it.

**Scope:**
- Full-branch multi-agent review (small diff itself — this sub-phase is mostly verification, not new code)
- Live e2e in both headless and headed modes on the Windows runner
- Performance baseline vs `main` (`scripts/perf-baseline-compare.sh` run on Windows)
- LOC/size/isolation audits (`cargo tree -p agent-desktop-core` still zero platform crate names; binary still under 15MB)
- Docs/skills sync (this document, `skills/agent-desktop-windows/`, README)
- Merge `feat/windows-adapter` → `main` as one release-noted `feat!`

**Key APIs:** none — verification and merge only

**Depends on:** 2.0 through 2.14 (2.14 may lag as a follow-up per its own note; 2.15 does not have to wait on it if 2.14 explicitly ships later)

**Exit criteria:** every item in the Cross-cutting sub-phase DoD holds for the whole branch; `main` gains Windows support in one commit.

**Est. PR size:** small diff, large verification effort

### Minimum OS Requirements

- Windows 10 1809+ for the baseline UIA adapter, app/window operations, clipboard, and legacy screenshot fallback
- Windows 10 1903+ for `Windows.Graphics.Capture` per-window modern screenshot
- Newer Windows 10/11 builds may expose richer Quick Settings / notification / shell UIA trees; commands report `PLATFORM_NOT_SUPPORTED` or degrade to the documented fallback when a shell surface is absent
- UIA COM interfaces are available before Windows 10, but Phase 2 does not support pre-1809 as a release target
- Session 0, Server Core, secure desktop, locked desktop, and other-user sessions are explicitly unsupported for observation/action/capture

### New Dependencies

| Crate | Version | Scope | Purpose |
|-------|---------|-------|---------|
| `uiautomation` | 0.24+ | Windows | UIA client wrapper, tree walker, patterns |
| `windows` | 0.62.2 | Windows | Raw Win32 / WinRT bindings for SendInput, clipboard, `Windows.Graphics.Capture`, D3D11 frame pool. Pinned to 0.62.2 to match `windows-capture 1.5.x`'s own pin |
| `windows-capture` | 1.5.4 | Windows | Modern per-window screenshot via `Windows.Graphics.Capture` in supported interactive sessions. Replaces `PrintWindow + PW_RENDERFULLCONTENT` as default, keeps legacy fallback |
| `screencapturekit` | 1.5 (crates.io) | macOS | Published crates.io canonical crate — the doom-fish fork is the maintained successor, NOT a git-SHA pin |
| `objc2` | 0.6 | macOS (new for P2-O13 / O17) | Safe bridging to `SCScreenshotManager`, `CGPreflightScreenCaptureAccess`, and AppKit/Foundation calls scoped to screenshot/permissions code |

All five pins above were recorded at research time (2026-04); re-verify each against crates.io and the repository's supply-chain policy at the opening sub-phase of the consuming platform (2.1 for Windows, 3.1 for Linux) before adding them to `Cargo.toml`.

Added as target-gated dependencies in the owning platform crates. The binary crate only depends on the platform crate for the current target.
```toml
# src/Cargo.toml
[target.'cfg(target_os = "windows")'.dependencies]
agent-desktop-windows = { path = "crates/windows" }

[target.'cfg(target_os = "macos")'.dependencies]
agent-desktop-macos = { path = "crates/macos" }

# crates/windows/Cargo.toml
[target.'cfg(target_os = "windows")'.dependencies]
uiautomation = "0.24"
windows = { version = "0.62.2", features = ["Win32_UI_Input", "Win32_UI_Input_KeyboardAndMouse", "Win32_System_Com", "Win32_System_DataExchange", "Win32_UI_WindowsAndMessaging", "Win32_Graphics_Gdi", "Graphics_Capture", "Win32_Graphics_Direct3D11"] }
windows-capture = "1.5.4"

# crates/macos/Cargo.toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = { version = "0.6", features = ["Foundation", "AppKit"] }
screencapturekit = "1.5"
```

### Testing (cross-platform validation, beyond each sub-phase's own exit criteria)

**Cross-platform validation:**
- Same snapshot of a cross-platform app (e.g., VS Code) produces structurally identical JSON on macOS and Windows
- All error codes produce identical JSON envelope format

**Cross-platform extension tests (P2-O8 through O17):**
- Stable-selector fields: known interactive controls emit `native_id` on both platforms when the app exposes one (UIA `AutomationId` on Windows, `AXIdentifier` or `AXDOMIdentifier` on macOS); controls without stable IDs omit the field and still resolve through the fingerprint fallback
- Event subscription: `watch --event value-changed --ref @s8f3k2p9:e3 --timeout 2000` receives an event within 500 ms of a programmatic value change on both platforms
- Text ranges: `text select-range @s8f3k2p9:e1 5 10` + `text get-selection @s8f3k2p9:e1` round-trips to `{start:5, length:10}` on both platforms for a multi-line text editor (TextEdit / Notepad)
- Text insert-at-caret: `text insert-at-caret @s8f3k2p9:e1 "hello"` produces matching `value` on both platforms with the caret advanced correctly
- Modern screenshot: `screenshot --window <id>` PNG matches a reference capture within SSIM threshold on supported OS/session combinations; cold latency <50 ms on both platforms where modern capture is available (vs ~300 ms macOS subprocess baseline)
- Toolbar surface: `snapshot --surface toolbar` on Safari (macOS) and Edge (Windows) returns the toolbar's children with refs
- Electron deep-tree: VS Code snapshot with `--force-electron-a11y` exposes ≥100 refs at default depth on both platforms
- Screen Recording permission: on a macOS runner without Screen Recording, `screenshot --window` returns `PermDenied` with the Screen Recording suggestion (distinct from AX denial)
- Automation permission: `permissions` reports `granted`, `denied`, or `unknown` without prompting; explicit requests run in the bounded isolated helper, while native `close-app` needs no Apple Event authorization

**FFI parity tests (P2-O16):**
- `ad_abi_version()` returns a packed `u32` matching the Cargo version; a consumer built against an older ABI major refuses to load a newer one
- `ad_snapshot` writes a refmap and the same qualified ref resolves via `ad_execute_by_ref` without a prior CLI snapshot on disk
- `ad_execute_by_ref(adapter, "@s8f3k2p9:e5", AD_ACTION_KIND_CLICK, &out)` produces identical `AdActionResult` to `ad_resolve_element` + `ad_execute_action`
- `ad_set_log_callback` receives at least one `tracing::debug!` event during an `ad_get_tree` call
- Every new `Action` variant round-trips through the `AdAction.kind` i32 → Rust enum conversion without UB on arbitrary bit patterns (extends the existing `fuzz_arbitrary_bit_patterns_never_panic_across_all_enums` suite)
- After the P2-O16 codegen migration: adding a command file automatically produces its `ad_<name>` wrapper — a regression test asserts the generated wrapper count matches the command registry count

Integration-level tests (Explorer/Notepad/Settings snapshots, click/type/clipboard/wait/lifecycle round-trips, Chromium detection, notification/tray list-and-act) are covered as exit criteria on their owning sub-phase above (2.4, 2.5, 2.7, 2.9, 2.10, 2.11, 2.14) rather than repeated here.

### Release, Skill & Docs (folds into 2.13 / 2.15)

**Release:**
- [ ] Prebuilt Windows `.exe` binary added to the existing `.github/workflows/release.yml` `build` matrix (alongside the macOS CLI targets), using the same tarball + sha256 + attestation pipeline Phase 1.5 ships
- [ ] npm `postinstall.js` gains a `win32-x64` / `win32-arm64` branch so `npm install -g agent-desktop` works on Windows without changes to package shape
- [ ] The Phase 1.5 FFI cdylib for Windows (`x86_64-pc-windows-msvc`) already ships; Phase 2 adds `aarch64-pc-windows-msvc` for ARM64 parity
- [ ] Every new `ad_*` FFI entrypoint (P2-O16) is included in the `release-ffi` build and CI header drift check
- [ ] GitHub Release notes document Windows support and installation

**Skill Update:**
- [ ] Create `skills/agent-desktop-windows/SKILL.md`: UIA permission model and UAC handling; Windows-specific behaviors (UIA patterns, WinUI3 quirks, COM initialization, Start/taskbar/Action Center/Quick Settings shell surfaces, virtual desktop detection, mixed-DPI coordinates); Chromium/Electron compatibility (depth-skip, resolver depth, surface detection patterns); `--force-renderer-accessibility` guidance for empty trees; Windows error codes and `platform_detail` examples (HRESULT codes); troubleshooting guide (empty trees, COM errors, elevation failures)
- [ ] Update core `SKILL.md`: add Windows platform skill to the skill graph table; update platform support section
- [ ] Update `workflows.md`: add cross-platform patterns noting Windows-specific differences; add Windows-specific workflow examples (e.g., navigating UWP apps)

**README Update:**
- [ ] Platform Support table: Windows column → **Yes**
- [ ] Windows installation instructions: npm (same command, auto-detects platform); direct `.exe` download from GitHub Releases; from source: `cargo build --release` on Windows (requires MSVC toolchain)
- [ ] Windows permissions section: UIA works without special permissions for most apps; UAC elevation may be required for elevated processes; Chromium apps need `--force-renderer-accessibility`
- [ ] "From source" section updated with Windows build requirements (Rust + MSVC)

---

## Phase 3 — Linux Adapter

**Status: Planned** — delivered as sub-phases 3.0–3.15 into the `feat/linux-adapter` integration branch, mirroring the Windows sub-phase template ([Platform Delivery Model](#platform-delivery-model--sub-phases-and-integration-branches)) one-to-one with AT-SPI2/D-Bus specifics substituted for UIA. Phase 3 begins only after Phase 2 (2.0–2.15) has merged to `main`.

Phase 3 completes the three-platform story. The Linux adapter implements the original adapter surface **plus** every cross-platform extension landed in Phase 2 (event subscriptions via `watch`, text ranges, modern screenshot, stable-selector fields, the predeclared toolbar and shell-surface vocabulary, new Action variants, new ErrorCode variants). Each has a canonical AT-SPI2 / D-Bus / Wayland-portal implementation. Core engine, trait contract, command-registry, CLI dispatch, FFI wrappers, and MCP transport are all untouched — per the [Command Surface Architecture](#command-surface-architecture-dry-invariant) invariant, Phase 3 is **pure `PlatformAdapter` trait implementation code**, nothing else. No new command files, no CLI dispatch changes, no FFI wrappers, no MCP tool registrations. "Foundation contract" below means Phase 1 + Phase 1.6 + whatever Phase 2 landed on the trait — Phase 3 implements against that settled contract, it does not renegotiate it.

### Objectives

Linux parity (original scope):

| ID | Objective | Metric |
|----|-----------|--------|
| P3-O1 | Linux adapter | `snapshot` on Ubuntu GNOME returns valid tree for Files, Terminal, Settings |
| P3-O2 | All commands cross-platform | Identical JSON contract output on all 3 platforms for every command |
| P3-O3 | Linux input synthesis | `click`, `type`, `press`, all mouse commands via AT-SPI actions + xdotool/ydotool |
| P3-O4 | Linux screenshot | `screenshot` produces PNG via PipeWire ScreenCast portal (Wayland) / XGetImage (X11) |
| P3-O5 | Linux clipboard | `clipboard-get` / `clipboard-set` / `clipboard-clear` via `wl-clipboard` (Wayland) / `xclip` (X11), marshaled through typed `ClipboardContent` |
| P3-O6 | Cross-platform CI | GitHub Actions matrix: macOS + Windows + Ubuntu |
| P3-O7 | Linux binary release | Prebuilt CLI binary added to the release pipeline (Phase 1.5 already ships the Linux FFI cdylib) |

Cross-platform extensions (Linux implementations of Phase 2 primitives):

| ID | Objective | Metric |
|----|-----------|--------|
| P3-O8 | Stable-selector fields on Linux | `AccessibilityNode.native_id` populated from AT-SPI2 `accessible-id` (standard since AT-SPI 2.18) with GTK `gtk-id` / Qt `objectName` fallback; `dom_classes` may be populated from AT-SPI2 `object-attributes` HTML keys on `WebKitGTK` / `Chromium-Content` embeds |
| P3-O9 | AT-SPI2 event subscriptions (`watch`, P2-O11 parity) | `watch_element` implemented via `zbus::Proxy::receive_signal` on AT-SPI2 signals: `org.a11y.atspi.Event.Object.PropertyChange`, `ChildrenChanged`, `StateChanged:focused`, `Window:Create`, `Window:Destroy`. Same `watch --event` CLI shape as macOS/Windows (see the `wait --event` vs `watch` naming note carried over from sub-phase 2.11). Replaces polling in `crates/linux/src/system/wait.rs` before it's even written |
| P3-O10 | AT-SPI2 Text interface (P2-O12 parity) | Text range primitives via `org.a11y.atspi.Text` D-Bus methods: `GetText(start, end)`, `GetCaretOffset`, `SetCaretOffset`, `GetNSelections`, `GetSelection(n)`, `AddSelection(start, end)`, `RemoveSelection(n)`. `InsertAtCaret` uses `org.a11y.atspi.EditableText.InsertText(position, text, length)` |
| P3-O11 | PipeWire modern screenshot (P2-O13 parity) | `screenshot --window <id>` via `org.freedesktop.portal.ScreenCast` (Wayland) + `org.freedesktop.portal.RemoteDesktop` for capture permission flow. XDG desktop portal handles the user consent dialog exactly like `SCScreenshotManager` does on macOS. X11 fallback uses `XGetImage` for the lowest-permission path |
| P3-O12 | Toolbar + surfaces (P2-O14 parity) | `SnapshotSurface::Toolbar` via AT-SPI2 `Role::ToolBar` — already predeclared core-side (U12), same as Windows. Dock / taskbar surface via per-DE panel process walk (GNOME Shell process for gnome-shell extensions, Plasma `plasmashell` for KDE). StatusNotifierWatcher already scoped in the original Phase 3 tray spec (3.14) |
| P3-O13 | Action variants on Linux (P2-O9 parity) | `Action::LongPress` via timed `xdotool/ydotool` button-hold; `Action::ShowMenu` via `org.a11y.atspi.Action.DoAction("popup")`; `Action::Cancel` via `Action.DoAction("cancel")` or Escape synthesis; `Action::DeliverFiles` via portal/native file-transfer where available with XDND as a researched fallback; `Action::ForceClick` returns `ActionNotSupported` on Linux (no pressure input primitive) |
| P3-O14 | FFI cdylib continues to ship | Phase 1.5 already publishes Linux FFI for x86_64 + aarch64; Phase 3 adds each new `ad_*` entrypoint's Linux implementation and extends the header drift check. No new FFI bindings to design — implementations only; the same P2-O16 registry migration applies once it lands |
| P3-O15 | Flatpak / Snap compatibility note | AT-SPI2 requires `--talk-name=org.a11y.Bus` permission inside sandboxed runtimes. Skill docs include the exact Flatpak override and Snap plug grants, so sandboxed consumers aren't silently empty-tree |

### Sub-phase decomposition (3.0–3.15, mirrors Phase 2 one-to-one)

Same rendering shape, same [Cross-cutting sub-phase DoD](#cross-cutting-sub-phase-dod) as Phase 2 — restated once here rather than per sub-phase: fmt/clippy/`-D warnings`/lib tests/conformance green; probe evidence (AT-SPI2 D-Bus introspection dumps, not screenshots) committed with the plan; adapters keep `not_supported()` defaults for everything not yet landed; no core rewrites; per-sub-phase review; perf baseline on perf-touching sub-phases; Conventional Commits. Every "foundation contract" reference below means Phase 1 + 1.6 + Phase 2's settled trait surface.

### 3.0 — Platform Exploration & Raw Scripting (pre-Rust)

**Goal:** empirically map Linux/AT-SPI2 accessibility reality with raw, no-Rust scripts before any adapter code exists, producing a committed evidence corpus the Rust sub-phases implement against — and feeding every contradiction back into this document. Mirror of 2.0 for Linux/AT-SPI2.

**Scope:** a `probes/linux/` directory of raw scripts in Python (direct D-Bus via python-dbus/`busctl`, and pyatspi2 where it clarifies) on GNOME (X11 and Wayland sessions both): full-tree dumps (GNOME Files, Terminal, Text Editor, one Electron app), interface census per Role (Action, Text, EditableText, Value, Selection, Component, StateSet), every interaction exercised raw (DoAction by index/name, text get/caret/insert, value set, selection), input-synthesis experiments via xdotool (X11) and ydotool (Wayland), `Component.Contains`/bounds hit-testing incl. occlusion, event-signal observations over the a11y bus, an accessible-id coverage census (GTK3/GTK4/Qt/Electron), the portal screenshot consent flow, a Flatpak sandbox probe (`--talk-name=org.a11y.Bus` on/off), and bus-bootstrap behavior when at-spi is cold. Alongside the scripts: `probes/linux/FINDINGS.md` — the same findings-ledger shape as 2.0, mapping every experiment to observed behavior and a doc-alignment verdict (confirms this document / contradicts it / new edge case).

**Key APIs:** python-dbus, `busctl`, pyatspi2, `Component.Contains`, xdotool, ydotool.

**Depends on:** completion of Phase 2 (Windows ships first).

**Exit criteria:** the script corpus and captured outputs are committed and re-runnable on the dev VM; the findings ledger covers tree, interfaces, interactions, input, hit-testing, identity, events, portal-consent, sandbox, and bus-bootstrap behavior with no open "unknown" rows; every ledger entry that contradicts this document has a matching amendment to this document landed in the same PR (see the source-of-truth feedback rule in the Platform Delivery Model); no Rust adapter sub-phase (3.2 onward) starts until the ledger is complete and every contradiction has amended this document.

**Est. PR size:** ~1.5k lines (scripts + ledger; no Rust).

### 3.1 — Toolchain, CI & Bus Bootstrap

**Goal:** Stand up the Linux build/CI/session substrate, including the async runtime the rest of Phase 3 depends on.

**Scope:**
- `ubuntu-latest` CI job promoted to a real Linux test lane (build + clippy + lib tests, core-isolation, size), mirroring 2.1's Windows promotion
- AT-SPI2 `org.a11y.Bus` session-bus availability detection at adapter construction
- `zbus` (5.x) / `atspi` (0.28+) / `tokio` (1.x) dependency pins re-verified against crates.io + supply-chain policy (pinned at 2026-04 research time, same policy as Windows)
- `LinuxAdapterSession` implementing `AdapterSession` via `open_session` — owns the D-Bus connection state so later sub-phases share one connection rather than reconnecting per call
- **`tokio` enters the workspace here for the first time.** The workspace is synchronous through Phase 2 (Windows adds no async runtime — UIA is a synchronous COM API); Linux is the first platform requiring async D-Bus calls via `zbus`/`atspi`

**Key APIs:** `org.a11y.Bus` presence check, `zbus::Connection`

**Depends on:** Phase 2 fully merged to `main`

**Exit criteria:** workspace green on Ubuntu CI; `LinuxAdapter` constructs and satisfies the trait; every command returns honest `PLATFORM_NOT_SUPPORTED` on Linux; missing-bus case returns `PLATFORM_NOT_SUPPORTED` with distro/DE-specific enable instructions (see AT-SPI2 Bus Detection below); bus-availability detection is unit-tested against mocked D-Bus responses.

**Est. PR size:** ~0.8k LOC

#### AT-SPI2 Bus Detection

- Check for `org.a11y.Bus` presence on the D-Bus session bus
- If the bus is not running, return `PLATFORM_NOT_SUPPORTED` with instructions:
  - GNOME: "AT-SPI2 should be enabled by default. Check `gsettings get org.gnome.desktop.interface toolkit-accessibility`"
  - Other DEs: "Install `at-spi2-core` and ensure `at-spi-bus-launcher` is running"
  - Flatpak/Snap: "Ensure the app has `--talk-name=org.a11y.Bus` permission" (P3-O15)

### 3.2 — Accessible Wrapper & Tree Walk

**Goal:** Own an `AXElement`-equivalent async wrapper for AT-SPI2 `Accessible` objects and prove raw tree traversal against a real Linux app.

**Scope:**
- `Accessible` D-Bus proxy wrapper (async, via `zbus`) with the same cycle-guard discipline as macOS/Windows (ancestor-path set, not a global visited set)
- Batched-fetch strategy — AT-SPI2 has no single "copy multiple attributes" call like `AXUIElementCopyMultipleAttributeValues`/`CacheRequest`, so this sub-phase designs the concurrent-D-Bus-call batching strategy (e.g. `futures::join_all` over property-get calls) that stands in for it
- Committed probe examples: raw AT-SPI2 D-Bus introspection dumps (`busctl --user introspect` or the `atspi` crate's own dump) of GNOME Files / GNOME Terminal, checked in as evidence

**Key APIs:** `org.a11y.atspi.Accessible.GetChildren`, `atspi` crate (v0.28+) + `zbus` (5.x) — pure Rust, no libatspi/GLib dependency

**Depends on:** 3.1

**Exit criteria:** an internal async tree-dump binary prints GNOME Files and Terminal trees with the batching strategy in place.

**Est. PR size:** ~2k LOC

### 3.3 — Vocabulary: Role, StateSet, native_id, Name Evidence

**Goal:** Map AT-SPI2's vocabulary onto the canonical role/state contract (U1/U2), completing `native_id` (P2-O8/P3-O8) on the third platform.

**Scope:**
- AT-SPI `Role` enum → unified role enum in `tree/roles.rs`
- AT-SPI `StateSet` → canonical state vocabulary — same conformance tests U1/U2 already run, parameterized over `LinuxAdapter`
- `accessible-id` (standard since AT-SPI 2.18) → `native_id`, with GTK `gtk-id` / Qt `objectName` fallback (P3-O8)
- `NameEvidence` supplier feeding core `accname.rs` (AT-SPI `Name`, `LabelledBy` relation, `Description`)

**Key APIs:** AT-SPI `Role` enum, `StateSet`, `accessible-id`, `Name`/`LabelledBy`/`Description`

**Depends on:** 3.2

**Exit criteria:** vocabulary conformance tests span every AT-SPI `Role` (complete mapping coverage, not a sample) and accname tests pass on Linux.

**Est. PR size:** ~1.5k LOC

### 3.4 — Observation: Snapshot, Windows, Apps, Displays

**Goal:** Land the full read path via portals/randr and the web-wrapper depth-skip for Electron/WebKitGTK content.

**Scope:**
- `get_tree`/`get_subtree` wired to the shared `SnapshotEngine` via async D-Bus `GetChildren` walks
- `list_windows`/`list_apps`/`focused_window` via the window-manager-neutral AT-SPI2 `Window` interface where available, falling back to `wmctrl`/`xdg-desktop-portal` introspection
- `list_displays` via XRandR (X11) / the Wayland portal's equivalent query + per-monitor scale
- **Web-wrapper depth-skip:** non-semantic wrapper elements with AT-SPI roles `ROLE_PANEL`, `ROLE_SECTION`, or `ROLE_FILLER` that have empty `Name` AND empty `Value` do not consume depth budget — the AT-SPI equivalent of macOS `AXGroup`/`AXGenericElement` and Windows `UIA_GroupControlTypeId` skipping. Implement in `crates/linux/src/tree/builder.rs` as `is_web_wrapper`
- **Chromium detection:** process name matching (electron, chrome, chromium, code); Linux Chromium additionally respects `ACCESSIBILITY_ENABLED=1` as an alternative to `--force-renderer-accessibility`
- **Resolver depth:** element re-identification searches to `ABSOLUTE_MAX_DEPTH` (50), matching macOS/Windows; implement in `crates/linux/src/tree/resolve.rs`
- **Surface detection for Electron:** an Electron modal may report as the active window itself rather than a child; check both `Role` and `RelationSet`/`RELATION_EMBEDS` (analogous to macOS AXRole + AXSubrole); implement in `crates/linux/src/tree/surfaces.rs`

**Key APIs:** `org.a11y.atspi.Window`, XRandR / portal display enumeration, `wmctrl` (fallback)

**Depends on:** 3.3

**Exit criteria:** `snapshot --app "GNOME Files"` and GNOME Terminal return reffed trees; a VS Code snapshot at default depth finds 50+ refs through web-aware depth-skip alone (no force flag); an Electron file-picker dialog is detected as a sheet surface.

**Est. PR size:** ~2k LOC

### 3.5 — Resolution & Live Locator

**Goal:** Make refs and the live `find`/`get`/`is` commands (U7) work on Linux with the same strict-resolution guarantees.

**Scope:**
- `resolve_element_strict*` from `RefEntry` evidence — `accessible-id`-first, fingerprint fallback, 0/1/N classification
- `get_live_value`/`get_live_state`/`get_live_actions`/`get_live_element`/`get_element_bounds` via async D-Bus property reads
- `resolve_query` — `LocatorQuery` evaluator backing live `find`
- `resolve_locator_anchor` + selected-hydration completeness — the same definitive-absence-vs-transport-failure classification ported from macOS's `is_definitive_absence`, this time distinguishing "object no longer on the bus" (D-Bus `UnknownObject`/`UnknownMethod` errors) from a genuine timeout

**Key APIs:** `accessible-id` lookup, AT-SPI2 D-Bus error classification (`org.freedesktop.DBus.Error.UnknownObject` vs timeout)

**Depends on:** 3.4

**Exit criteria:** `find`/`get`/`is` are live on Linux; `STALE_REF`/`AMBIGUOUS_TARGET` semantics proven with committed probe evidence.

**Est. PR size:** ~2k LOC

### 3.6 — Actionability & Occlusion

**Goal:** Port the auto-wait/occlusion gate (U8/U9) onto Linux using AT-SPI2's `Component` interface.

**Scope:**
- `hit_test` three-way result via `Component.ContainsPoint` + bounds corroboration — `Unknown` on probe failure, never a false negative
- `receives_events` evidence
- Visibility/enabled/offscreen evidence feeding the core auto-wait gate — no Linux-specific auto-wait logic, core drives the loop
- `scroll_into_view` — AT-SPI2 has no native scroll-into-view primitive; implement via `Component.GetPosition` + coordinate-based scroll synthesis, following the same policy gating as the Phase 1 scroll command

**Key APIs:** `org.a11y.atspi.Component.ContainsPoint`, `Component.GetExtents`

**Depends on:** 3.5

**Exit criteria:** zero-bounds/disabled/occluded fixture cases produce the same envelopes as macOS/Windows.

**Est. PR size:** ~1.2k LOC

### 3.7 — Semantic Action Tier

**Goal:** Land AT-SPI2 `Action`/`EditableText`-based semantic dispatch with the same typed `ActionStep` delivery reporting.

**Scope:**
- `perform_action` via `org.a11y.atspi.Action.DoAction(0)` (click), name-based dispatch for expand/collapse/toggle (`DoAction("expand")`/`"collapse"`/`"toggle"`), `Selection.SelectChild` (select), `EditableText.InsertText` (set text, falling back to clipboard paste)
- Activation chain with `ActionStep` delivery reporting + post-verification reads, same honest `verified` semantics as macOS/Windows
- See the Linux API Mapping table below for the full pattern list

**Key APIs:** `Action.DoAction`, `Selection.SelectChild`, `EditableText.InsertText`

**Depends on:** 3.6

**Exit criteria:** click/set-value/clear/select/toggle/expand/collapse work headless on the fixture app via the e2e analog (3.12 supplies the fixture; interim coverage via GNOME Files/Terminal).

**Est. PR size:** ~2k LOC

#### Linux API Mapping (reference table for sub-phases 3.2–3.10)

| Capability | Technology | Details |
|------------|-----------|---------|
| Tree root | `atspi Accessible` on bus | Via `atspi` crate (v0.28+) + `zbus` (5.x) — pure Rust, no libatspi/GLib dependency |
| Children | `org.a11y.atspi.Accessible.GetChildren` | Async D-Bus calls to the AT-SPI2 registry daemon |
| Role mapping | AT-SPI `Role` enum | Map to unified role enum in `tree/roles.rs` — e.g. `Role::PushButton` → `button` |
| Click | `org.a11y.atspi.Action.DoAction(0)` | AT-SPI actions preferred over coordinate-based input |
| Set text | `org.a11y.atspi.Text.InsertText` | AT-SPI text interface; falls back to clipboard paste |
| Expand/Collapse | `Action.DoAction("expand")` / `Action.DoAction("collapse")` | Action name-based dispatch |
| Select | `org.a11y.atspi.Selection.SelectChild` | For combobox, listbox, tab items |
| Toggle | `Action.DoAction("toggle")` or `Action.DoAction("click")` | For checkboxes, switches |
| Scroll | Coordinate-based scroll events via xdotool/ydotool | AT-SPI has no native scroll pattern |
| Keyboard | `xdotool key` (X11) / `ydotool key` (Wayland) | Shelling out for input synthesis |
| Mouse | `xdotool mousemove/click` (X11) / `ydotool mousemove/click` (Wayland) | Display server detected at runtime |
| Clipboard | `wl-copy` / `wl-paste` (Wayland) / `xclip` (X11) | Shelling out; display server detected at runtime; marshaled through typed `ClipboardContent` |
| Screenshot | PipeWire ScreenCast portal (Wayland) / `XGetImage` (X11) | Or the `xcap` crate for consistency |
| App launch | `xdg-open` / direct process spawn | Launch by `.desktop` file or command name, via `LaunchOptions` |
| App close | `SIGTERM` / `SIGKILL` | Graceful close first, force with `--force`; verified via `ProcessState` |
| Window ops | `xdotool` / `wmctrl` | Window resize, move, minimize, maximize, restore |
| Permissions | AT-SPI2 bus availability | Check for `org.a11y.Bus` on the D-Bus session bus. Return `PLATFORM_NOT_SUPPORTED` with enable instructions if missing |
| Notifications | D-Bus `org.freedesktop.Notifications` | See Notification Management approach under 3.14 |
| System tray | D-Bus `org.kde.StatusNotifierWatcher` | See System Tray approach under 3.14 |

### 3.8 — Input Synthesis

**Goal:** Land raw OS input across the X11/Wayland split, matching the delivery-tracking and headed/headless policy contract.

**Scope:**
- Display-server-detected keyboard synthesis: `xdotool key` (X11) / `ydotool key` (Wayland)
- Mouse events + modifier chords + wheel via the same detected tool
- Drag with delivery tracking + release guard
- Headed/headless policy parity — raw cursor commands require `--headed`, same as macOS/Windows
- **`libei`** (the newer input-emulation portal API) is documented here as a researched future alternative to shelling out to xdotool/ydotool, not adopted as a dependency in this sub-phase — it would remove the subprocess dependency but isn't mature enough across desktop environments yet to be the default

**Key APIs:** `xdotool`, `ydotool` (subprocess), `libei` (documented, not used)

**Depends on:** 3.7

**Exit criteria:** headed e2e gesture cases pass on at least GNOME/X11 and GNOME/Wayland (once 3.12's fixture exists).

**Est. PR size:** ~2k LOC

### 3.9 — System Lifecycle

**Goal:** Land process/window lifecycle with the same `ProcessState` liveness contract.

**Scope:**
- `launch_app` with `LaunchOptions` (spawn via `xdg-open` or direct `Command::spawn`, args/env/cwd, attach-vs-fail policy)
- `close_app` with verified termination (`SIGTERM` then `SIGKILL` under `--force`, confirmed via a follow-up liveness probe, not assumed)
- `window_op` via `xdotool`/`wmctrl` for resize/move/minimize/maximize/restore
- `ProcessState` probes: `/proc/<pid>/stat` state character for hung detection where meaningful, exit-code inspection → `Exited`/`Crashed`
- `is_protected_process`
- `press_key_for_app` under the same focus policy

**Key APIs:** `xdg-open`, `std::process::Command`, `xdotool`/`wmctrl`, `/proc/<pid>/stat`

**Depends on:** 3.4 (window identity), 3.8 (input for `press_key_for_app`)

**Exit criteria:** lifecycle e2e (launch → interact → close) passes.

**Est. PR size:** ~1.8k LOC

### 3.10 — Capture & Clipboard

**Goal:** Ship screenshot and typed clipboard across the Wayland-portal / X11 split.

**Scope:**
- `screenshot` via `org.freedesktop.portal.ScreenCast` (Wayland, P3-O11) with `org.freedesktop.portal.RemoteDesktop` for the permission flow, falling back to `XGetImage` (X11) for the lowest-permission path
- `screenshot --screen` honest display targeting (pairs with `list_displays` from 3.4)
- Typed clipboard: `wl-clipboard` (Wayland) / `xclip` (X11) subprocess calls, marshaled into `ClipboardContent::Text`/`Image`/`FileUrls`, written through 0600-equivalent private files

**Key APIs:** `org.freedesktop.portal.ScreenCast`, `org.freedesktop.portal.RemoteDesktop`, `XGetImage`, `wl-clipboard`/`xclip`

**Depends on:** 3.1 (private-file handling — Unix permissions already real; Windows private-file hardening is still to be built from scratch in 2.1), 3.4 (displays)

**Exit criteria:** screenshot + clipboard e2e pass (clipboard tests hermetic); the PipeWire portal flow is proven — the user approves via the XDG portal dialog once, subsequent calls bypass the dialog within the session grant window.

**Est. PR size:** ~1.8k LOC

### 3.11 — Signals & Wait Parity

**Goal:** Port `SignalBaseline`/`diff_signals`/`wait --event` (U17) to Linux, with the same naming distinction from `watch` as Windows 2.11.

**Scope:**
- Linux `SignalBaseline` producers: windows/apps/focus/surfaces via AT-SPI2 polling snapshots
- `wait --event` parity including `surface-appeared`
- Wait utilities operating within `Deadline` budgets

**Key APIs:** AT-SPI2 property snapshots for baseline capture (no signal subscription yet — that's `watch`, P3-O9; sub-phase 3.2's D-Bus infrastructure feeds it but the command itself is still future scope beyond this sub-phase's baseline-diff wait)

**Depends on:** 3.4 (windows/apps/displays), 3.9 (process lifecycle)

**Exit criteria:** an AE6-analog e2e passes — an unnamed dialog is discovered purely by baseline diff.

**Est. PR size:** ~1k LOC

### 3.12 — GTK4 Fixture App & Live E2E Harness

**Goal:** Give Linux the same verify-by-observation live e2e discipline, with explicit per-desktop-environment notes since Linux fragments more than macOS/Windows.

**Scope:**
- GTK4 fixture app (GNOME primary target) with `accessible-id`/`gtk-id` set on every interactive target from day one
- Fixture targets mirroring `AgentDeskFixture.swift`/the Windows WinForms fixture: delayed-enable, zero-bounds, duplicate-title, occlusion, disclosure
- Harness port asserting every effect by independent re-observation, same contract as `tests/e2e/run.sh`
- Per-DE notes: GNOME is the primary CI target (best AT-SPI2 support); KDE Plasma 5.24+ is a secondary, manually-verified target, not a CI gate in this sub-phase
- `linux-e2e` workflow_dispatch job on an interactive Ubuntu GNOME runner

**Key APIs:** GTK4 `Accessible` widget properties (`accessible-id`)

**Depends on:** 3.7, 3.8, 3.9, 3.10, 3.11

**Exit criteria:** the full Linux live gate is green on GNOME, both headless and headed tiers.

**Est. PR size:** ~2k LOC (mostly fixture app + scripts, not adapter Rust)

### 3.13 — FFI, npm, Release

**Goal:** Make the Linux adapter reachable through every distribution channel already shipping for macOS/Windows.

**Scope:**
- FFI real-adapter path validated on Linux (non-stub tests) for both x86_64 and aarch64
- npm `postinstall.js` gains `linux-x64`/`linux-arm64` branches
- Release matrix: CLI binary added for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, reusing the runners Phase 1.5 already builds the FFI cdylib on
- `skills/agent-desktop-linux/SKILL.md` — see Skill Update below
- README platform table: Linux column → **Yes**; minimum glibc (2.35, Ubuntu 22.04 baseline) documented

**Key APIs:** none new — packaging only

**Depends on:** 3.2 through 3.11

**Exit criteria:** `npm install -g` works on Ubuntu; release dry-run artifacts verified for both architectures.

**Est. PR size:** ~1.2k LOC

### 3.14 — Shell Surfaces & Notifications Spike (stretch, explicitly deferrable)

**Goal:** Cover the Linux-only shell surface, notification, and tray scope — the DE-specific half of parity that has no single canonical implementation across GNOME/KDE/other DEs.

**Scope:** notification management and system tray, folded in below. Same stretch/deferrable status as Windows 2.14 — **may ship after the 3.15 integration merge as its own follow-up `feat`.**

**Key APIs:** see the two subsections immediately below.

**Depends on:** 3.4 (observation), 3.7 (semantic actions)

**Exit criteria:** notification list/dismiss/action work on at least GNOME via `org.gnome.Shell.Notifications`; tray list/click work via `StatusNotifierWatcher` on a DE that supports SNI (KDE, or GNOME + AppIndicator extension); unsupported DEs assert `PLATFORM_NOT_SUPPORTED` with daemon/DE-specific guidance rather than silently passing.

**Est. PR size:** ~2k LOC

#### Notification Management (Linux Implementation)

Linux notification management is built from scratch here. The macOS (completed) and Windows (Phase 2) implementations are the reference patterns — same trait methods, same JSON output contract, same 1-based indexing.

- **List notifications:** the standard `org.freedesktop.Notifications` D-Bus interface does NOT provide a "list current notifications" method; the approach varies by desktop environment:
  - GNOME: `org.gnome.Shell` exposes `org.gnome.Shell.Notifications` with `GetNotifications()` (returns an array of notification dicts)
  - KDE Plasma: `org.freedesktop.Notifications` with a `GetNotifications()` extension, or the `org.kde.notificationmanager` D-Bus interface
  - Other DEs: monitor `Notify` D-Bus signals to maintain an in-memory notification history within the daemon session
- **Dismiss:** `org.freedesktop.Notifications.CloseNotification(id)` — works across all notification daemons
- **Interact with actions:** listen for user-triggered actions, or programmatically invoke via the `ActionInvoked` signal; the D-Bus spec does not define a method to trigger actions programmatically — coordinate-based click on the notification popup via AT-SPI may be needed as a fallback
- **Do Not Disturb:** GNOME `gsettings get org.gnome.desktop.notifications show-banners`; KDE `org.kde.notificationmanager`'s `inhibited` property
- **Edge case:** notification daemon varies by DE — detect via `GetServerInformation()`; return `PLATFORM_NOT_SUPPORTED` with daemon-specific guidance if the notification interface is unreachable

#### System Tray (Linux Implementation)

System tray interaction is built from scratch here.

- **Modern tray (SNI):** most modern Linux apps use the `StatusNotifierItem` D-Bus protocol; discover items via `org.kde.StatusNotifierWatcher.RegisteredStatusNotifierItems`
- **Legacy tray (XEmbed):** older apps use the XEmbed protocol; access via the AT-SPI tree of the tray window, or coordinate-based interaction
- **List items:** query `StatusNotifierWatcher` for registered items — `Title`, `IconName`, `ToolTip`, `Menu` (D-Bus menu path) properties
- **Activate:** call `Activate(x, y)` on the `StatusNotifierItem` D-Bus interface
- **Context menu:** call `ContextMenu(x, y)`, or read the `Menu` property to get the `com.canonical.dbusmenu` path and traverse the menu tree
- **Edge case:** GNOME does not natively support SNI (requires the AppIndicator extension); detect and report via an error suggestion if no tray is available

### 3.15 — Hardening & Integration Review

**Goal:** Prove the assembled `feat/linux-adapter` branch is production-grade as a whole, then merge it — closing out the three-platform story.

**Scope:**
- Full-branch multi-agent review
- Live e2e in both headless and headed modes on the GNOME runner
- Performance baseline vs `main` (`scripts/perf-baseline-compare.sh` run on Linux)
- LOC/size/isolation audits
- Docs/skills sync
- Merge `feat/linux-adapter` → `main` as one release-noted `feat!`

**Key APIs:** none — verification and merge only

**Depends on:** 3.0 through 3.14 (3.14 may lag as a follow-up per its own note)

**Exit criteria:** every item in the Cross-cutting sub-phase DoD holds for the whole branch; `main` gains Linux support in one commit, completing the three-platform matrix.

**Est. PR size:** small diff, large verification effort

### Display Server Detection

Runtime detection required for input, clipboard, and screenshot since Linux runs either X11 or Wayland:

- Check `$WAYLAND_DISPLAY` environment variable — if set, use the Wayland path
- Check `$DISPLAY` environment variable — if set and no Wayland, use the X11 path
- If neither, return `PLATFORM_NOT_SUPPORTED` with guidance to check the display server configuration
- Input tools: verify `xdotool` (X11) or `ydotool` (Wayland) is installed; error with install instructions if missing
- Clipboard tools: verify `xclip` (X11) or `wl-clipboard` (Wayland) is installed; error with install instructions if missing

### Minimum OS Requirements

- Ubuntu 22.04+ / Fedora 38+
- GNOME 42+ (primary target), KDE Plasma 5.24+ (secondary)
- `at-spi2-core` package installed (default on GNOME)
- X11: `xdotool` installed. Wayland: `ydotool` installed

### Key Risks and Mitigations (Linux-specific — folds into the Risk Register)

| Risk | Mitigation |
|------|------------|
| Wayland a11y gaps | Focus on GNOME (best AT-SPI2 support). Prefer AT-SPI actions over coordinate input. Document known gaps clearly in skill and README. |
| AT-SPI2 bus not running | Detect on first command. Return clear enable instructions specific to the detected distro/DE. |
| Display server fragmentation | Runtime detection (X11 vs Wayland). Separate code paths for input/clipboard/screenshot. Test both. |
| Rust a11y crate maintenance stalls | Pin `atspi` and `zbus` versions. `atspi` crate backed by the Odilia accessibility project. Maintain patches if upstream stalls. |
| Input tool availability | Check for xdotool/ydotool on first use. Provide package manager install commands in the error suggestion. |

### New Dependencies

| Crate | Version | Purpose | License |
|-------|---------|---------|---------|
| `atspi` | 0.28+ | Linux AT-SPI2 client | MIT/Apache-2.0 |
| `zbus` | 5.x | D-Bus connection | MIT/Apache-2.0 |
| `tokio` | 1.x | Async runtime (required by atspi/zbus for async D-Bus) | MIT |

All three pins above were recorded at research time (2026-04); re-verify against crates.io and the repository's supply-chain policy at sub-phase 3.1 before adding them to `Cargo.toml` — same policy as the Windows dependency pins.

Added to `Cargo.toml` as a target-gated dependency:
```toml
[target.'cfg(target_os = "linux")'.dependencies]
agent-desktop-linux = { path = "crates/linux" }
```

`tokio` is introduced here for the first time. The workspace is synchronous through Phase 2 — Windows adds no async runtime, since UIA's COM APIs are synchronous. Linux is the first platform requiring async D-Bus calls via `zbus`/`atspi`.

### Testing (cross-platform validation, beyond each sub-phase's own exit criteria)

**Cross-platform validation:**
- Same snapshot of a cross-platform app (e.g., VS Code) produces structurally identical JSON on all 3 platforms
- All error codes produce identical JSON envelope format on all 3 platforms
- Notification commands return identical JSON envelope structure across all 3 platforms (list, dismiss, action)
- Tray / StatusNotifierItem commands return identical JSON envelope structure across all 3 platforms

**Extension tests for P3-O8 through O15 (Linux-specific parity):**
- AT-SPI `accessible-id` populated for every interactive node in GNOME Calculator, GNOME Files, Firefox (with `ACCESSIBILITY_ENABLED=1`)
- `watch --event value-changed` via `zbus` signal subscription delivers an event within 500 ms for a programmatic value change in a test harness app (GTK4 + pygobject)
- `text select-range` / `get-selection` / `insert-at-caret` round-trips correctly in GNOME Text Editor via `org.a11y.atspi.Text` + `EditableText`
- PipeWire portal screenshot flow: the user approves via the XDG portal dialog, subsequent calls bypass the dialog within the session grant window; screenshot matches reference
- Toolbar surface: Firefox toolbar + GNOME Files toolbar both enumerate via `Role::ToolBar`
- Flatpak compatibility: a Flatpak-packaged GNOME Text Editor snapshot is non-empty when `--talk-name=org.a11y.Bus` is granted; returns a clear diagnostic otherwise

Integration-level tests (Files/Terminal/Settings snapshots, click/type/clipboard/wait/lifecycle round-trips, bus-not-running error path, notification/tray list-and-act) are covered as exit criteria on their owning sub-phase above (3.4, 3.5, 3.7, 3.9, 3.10, 3.11, 3.14) rather than repeated here.

### Release, Skill & Docs (folds into 3.13 / 3.15)

**Release:**
- [ ] Prebuilt Linux CLI binary added to `.github/workflows/release.yml` matrix for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` (Phase 1.5 already builds the FFI cdylib for both triples on the same runners — Phase 3 reuses those runners)
- [ ] npm `postinstall.js` gains `linux-x64` / `linux-arm64` branches
- [ ] Every new `ad_*` Linux implementation from P3-O9 / O10 / O11 is covered by the existing FFI drift check + Sigstore attestation pipeline
- [ ] GitHub Release notes document Linux support, minimum glibc (2.35, Ubuntu 22.04 baseline), display-server requirements, and Flatpak/Snap compatibility

**Skill Update:**
- [ ] Create `skills/agent-desktop-linux/SKILL.md`: AT-SPI2/D-Bus setup and bus detection; Wayland vs X11 differences (input via xdotool/ydotool, clipboard via wl-clipboard/xclip, screenshot via PipeWire/XGetImage); required system tools (`xdotool` or `ydotool`, `xclip` or `wl-clipboard`); Linux error codes and `platform_detail` examples (D-Bus errors, bus not found); troubleshooting guide (bus not running, empty trees, missing tools, Flatpak/Snap permissions)
- [ ] Update core `SKILL.md`: add Linux platform skill to the skill graph table; update platform support section to show all 3 platforms
- [ ] Update `workflows.md`: add cross-platform patterns noting Linux-specific differences; add Linux-specific workflow examples (e.g., GNOME app automation); document display server detection behavior

**README Update:**
- [ ] Platform Support table: Linux column → **Yes**
- [ ] Linux installation instructions: npm (same command, auto-detects platform); direct binary download from GitHub Releases; from source: `cargo build --release` on Linux (requires `pkg-config`, `libdbus-1-dev`)
- [ ] Linux permissions section: AT-SPI2 bus must be running (default on GNOME, may need enabling on other DEs); required tools `xdotool` (X11) or `ydotool` (Wayland) for input synthesis; required tools `xclip` (X11) or `wl-clipboard` (Wayland) for clipboard; how to check: `busctl --user list | grep a11y`
- [ ] Update minimum OS versions: Ubuntu 22.04+ / Fedora 38+
- [ ] Update "From source" section with Linux build requirements

---

## Phase 4 — MCP Server Mode

**Status: Planned**

Phase 4 adds a new I/O layer. Core engine and all three platform adapters are unchanged. The MCP server wraps existing command logic in JSON-RPC tool definitions, enabling agent-desktop to work as an MCP-native desktop automation server for Claude Desktop, Cursor, VS Code Copilot, Gemini CLI, Microsoft Agent Framework 1.0, and any other MCP-compatible host.

By Phase 4 the CLI already covers the shared command surface on three platforms, the FFI ships as a shared library for in-process consumers, and the cross-platform event / text-range / stable-selector primitives from Phase 2 / 3 are in place. MCP mode is a **transport + discovery layer**, nothing more. Per the [Command Surface Architecture](#command-surface-architecture-dry-invariant) invariant at the top of this document, the MCP crate contains zero per-tool and zero per-platform code — it walks the same deterministic command descriptor registry the CLI and FFI use, and dispatches to the same `execute(args, adapter)` functions. New commands added in Phase 2 or Phase 5 (e.g. `watch`, `text select-range`, `find --visual`) become MCP tools automatically with no changes to `crates/mcp/`.

### Objectives

| ID | Objective | Metric |
|----|-----------|--------|
| P4-O1 | MCP server mode via `--mcp` | Responds to MCP `initialize` handshake, reports capabilities, per-host hello-world passes |
| P4-O2 | All commands as MCP tools | `tools/list` returns all 58 shipped command names as tools (54 operational + 4 fail-closed, matching the CLI surface 1:1) with JSON Schemas generated from the CLI arg structs via `schemars`; tool names prefixed `desktop_` |
| P4-O3 | Claude Desktop + Cursor + VS Code + Gemini CLI + MS Agent Framework validated | Each host invokes tools to control a desktop app end-to-end on all three platforms; repo ships `mcp.json` / `claude_desktop_config.json` / `.cursor/mcp.json` examples per host |
| P4-O4 | Tool annotations | `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint` on every tool; Claude Desktop surfaces destructive tools with a confirmation prompt |
| P4-O5 | Ref-based MCP tool shape (Playwright-MCP idiom) | Tools take `{ref: "@s8f3k2p9:e5"}` (the qualified snapshot-ref form) not raw `element_handle`, matching Playwright MCP's ref idiom so agents can swap between the two without relearning selectors. Tree snapshots return as MCP resources with refs inline |
| P4-O6 | MCP resource types | `agent-desktop://refmap/current`, `agent-desktop://snapshot/latest`, `agent-desktop://audit/{trace_id}` (audit log under Phase 5). `resources/list` + `resources/read` expose the current RefMap and last snapshot without re-running the command |
| P4-O7 | Tree-diff notifications | `watch` events (Phase 2 P2-O11 push subscription — distinct from the already-shipped `wait --event` baseline diff) stream as MCP `notifications/message` during a long-running wait, so the host sees value-changed / focus-changed events as they happen rather than polling |
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

The MCP crate is small and generic by design. It contains **zero per-tool files and zero per-platform code**. Per the Command Surface Architecture invariant at the top of this document, every CLI command is described through deterministic command metadata; the MCP server iterates those descriptors at startup and exposes each entry as an MCP tool.

```
crates/mcp/src/
├── lib.rs              # mod declarations + re-exports
├── server.rs           # rmcp server bootstrap, initialize handler, walks the command registry
├── transport.rs        # stdio (primary), Streamable HTTP (P4-O12), SSE (legacy)
├── capability.rs       # P4-O9 tier gating (observation / interactive / destructive)
├── resources.rs        # P4-O6 resource types (refmap / snapshot / permissions / events / audit)
├── notifications.rs    # P4-O7 watch event forwarder, P4-O8 progress forwarder
└── schema.rs            # Translates CommandDescriptor → rmcp tool definition
```

That's the whole crate. It doesn't know what `desktop_click` does — it reads generated command descriptors and forwards invocations through the same command execution function the CLI uses. Adding a command in Phase 2 (`text select-range`, `watch`) or Phase 5 (`find --visual`, `audit tail`) should mean **zero lines of MCP-specific behavior** — only shared command metadata and adapter methods change.

### MCP tool registration — the one-time rewrite

```rust
// crates/mcp/src/server.rs  (illustrative, ~80 lines total for the crate)

pub async fn serve(adapter: Box<dyn PlatformAdapter>) -> Result<()> {
    let mut server = rmcp::ServerBuilder::new("agent-desktop", env!("CARGO_PKG_VERSION"));

    // Walk generated descriptors. No hand-maintained tool list.
    for cmd in command_descriptors() {
        // Skip tools disallowed by current permission set (P4-O11).
        if !cmd.available_under(&adapter.permission_report()) { continue; }

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
| `desktop_list_displays` | `list-displays` | Array of displays with `scale_factor` |
| `desktop_list_surfaces` | `list-surfaces` | Array of surfaces (incl. Toolbar / Spotlight / Dock / MenuBarExtras and Windows shell surfaces from P2-O14/P2-O18) |
| `desktop_list_notifications` | `list-notifications` | Array of notifications |
| `desktop_screenshot` | `screenshot` | Base64 PNG (or MCP resource link) |
| `desktop_clipboard_get` | `clipboard-get` | Typed clipboard content |
| `desktop_permissions` | `permissions` | Tri-state permission report (AX + Screen Recording + Automation) |
| `desktop_status` | `status` | Daemon + adapter status |
| `desktop_version` | `version` | Version + ABI version |
| `desktop_session` | `session start / end / list / gc` | Session lifecycle (manifest, trace segments) |
| `desktop_trace` | `trace export / show` | Reliability trace export / inspection |

Interaction tools (gated by `interactive` capability):

| MCP Tool | CLI | Shape |
|----------|-----|-------|
| `desktop_click` / `desktop_double_click` / `desktop_triple_click` / `desktop_right_click` | `click @s8f3k2p9:e5` (and variants) | `{ref: "@s8f3k2p9:e5"}` |
| `desktop_type_text` | `type @s8f3k2p9:e5 "hello"` | `{ref: "@s8f3k2p9:e5", text: "hello"}` |
| `desktop_set_value` | `set-value @s8f3k2p9:e5 "hello"` | `{ref: "@s8f3k2p9:e5", value: "hello"}` |
| `desktop_clear` | `clear @s8f3k2p9:e5` | `{ref: "@s8f3k2p9:e5"}` |
| `desktop_focus` | `focus @s8f3k2p9:e5` | `{ref: "@s8f3k2p9:e5"}` |
| `desktop_select` / `desktop_toggle` / `desktop_check` / `desktop_uncheck` / `desktop_expand` / `desktop_collapse` | — | `{ref: "@s8f3k2p9:e5"}` (+ `value` for select) |
| `desktop_scroll` / `desktop_scroll_to` | `scroll <dir>` | `{ref: "@s8f3k2p9:e5", direction, amount}` |
| `desktop_press_key` | `press <keys>` | `{key, modifiers}` |
| `desktop_hover` / `desktop_drag` | `hover`/`drag` | `{ref: "@s8f3k2p9:e5"}` or `{from, to}` |
| `desktop_mouse_move` / `desktop_mouse_click` / `desktop_mouse_wheel` | — | `{x, y, button}` |
| `desktop_key_down` / `desktop_key_up` / `desktop_mouse_down` / `desktop_mouse_up` | — | `{key, modifiers}` / `{x, y, button}` — listed for CLI parity but fail closed identically to their CLI counterparts (see Phase 1) until the Phase 5 daemon owns held input |
| `desktop_wait` | `wait --element / --window / --text / --menu / --notification / --event` | `{condition, timeout_ms}` |
| `desktop_watch` (P2-O11) | `watch --event …` | `{ref: "@s8f3k2p9:e5", events: [EventKind], timeout_ms}` — streams via `notifications/message`; distinct from `desktop_wait`'s `--event` mode, which is a single baseline-diff read |
| `desktop_launch_app` / `desktop_focus_window` / `desktop_resize_window` / `desktop_move_window` / `desktop_minimize` / `desktop_maximize` / `desktop_restore` | app / window ops | App / window args |
| `desktop_clipboard_set` / `desktop_clipboard_clear` | — | typed clipboard content / `{}` |
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
| `agent-desktop://events/stream` | Merged `watch` event stream for the session | Real-time, subscribable |
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
- **Session:** On `initialize`, detect platform, probe permissions (AX + Screen Recording + Automation tri-state), report tool capabilities given current permissions. The current CLI already supports `--session <id>` as an on-disk latest-snapshot namespace and a manifest-gated session with automatic trace segments (Phase 1.6). MCP adds per-host in-memory session state keyed by `session_id`; it must not use the legacy `~/.agent-desktop/last_refmap.json` artifact and should bridge to the same explicit snapshot semantics as the CLI.

### Initialize Handler

On receiving MCP `initialize`:
1. Detect platform (macOS / Windows / Linux)
2. Check permissions (`permission_report()`)
3. Report capabilities: list of available tools, platform, permission status
4. If permissions not granted, include guidance in capabilities response

### New Dependencies

| Crate | Version | Purpose | License |
|-------|---------|---------|---------|
| `rmcp` | 0.15.0+ | Official MCP Rust SDK — `#[tool]` macro, JSON-RPC handling, transport | MIT/Apache-2.0 |
| `schemars` | 1.2+ | JSON Schema generation for tool parameter definitions | MIT/Apache-2.0 |
| `tokio` | 1.x | Async runtime (required by rmcp for MCP server event loop) | MIT |

`tokio` enters the workspace no later than Phase 3 (Linux, sub-phase 3.1); by Phase 4 it is already available.

### Binary Crate Changes

- `src/main.rs` / `src/cli/` — Add `--mcp` flag detection, route to MCP server mode
- `Cargo.toml` — Add `agent-desktop-mcp` dependency (non-platform-gated, available on all platforms)
- No changes to `src/dispatch/` or command files — MCP tools call the same `execute()` functions

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
- `desktop_watch` subscription receives `notifications/message` events for a programmatic value change within 500 ms of the change on all three platforms
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

Skill maintenance rules:

- [ ] Create `skills/agent-desktop-mcp/SKILL.md`:
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

## Phase 5 — Production Readiness

**Status: Planned**

Phase 5 transforms agent-desktop from functional to enterprise-grade. Persistent daemon process, in-memory session multiplexing for concurrent agents, the safety trio required for enterprise and regulated deployments (dry-run + confirm + audit log), an OCR/vision fallback for custom-rendered UIs where the accessibility tree is empty, OpenTelemetry-compatible trace export on top of the current JSONL reliability trace, and first-class distribution via native package managers.

### Objectives

| ID | Objective | Metric |
|----|-----------|--------|
| P5-O1 | Persistent daemon | Warm snapshot completes in <50ms (vs 200ms+ cold start) |
| P5-O2 | Daemon session multiplexing | Two agents hold independent in-memory RefMaps without interference; CLI `--session` remains the on-disk latest-snapshot namespace for non-daemon use |
| P5-O3 | Enterprise quality gates | All gates in quality gates table pass |
| P5-O4 | Package manager distribution | Available via brew (macOS), winget/scoop (Windows), snap/apt (Linux) with Sigstore attestation verification on install |
| P5-O5 | Safety trio: `--dry-run` / `--confirm` / append-only audit log | Every destructive command supports `--dry-run` (resolves ref, computes the action, emits the would-be JSON response, does not execute), `--confirm` (stderr prompt with configurable timeout), and `~/.agent-desktop/audit.jsonl` append-only log with trace_id, actor, tool, args, decision (allowed / dry-run / denied / confirmed), exit code, timestamp. Covers EU AI Act Article 14 and OWASP Agentic Top-10 (2026) requirements |
| P5-O6 | Policy allowlist / denylist | `~/.agent-desktop/policy.yaml` defines per-tool rules — e.g. "never call `desktop_close_app` for `com.apple.finder`", "require confirm for any action on bundle ID `com.apple.mail`". Loaded at daemon start, reload-on-SIGHUP. Policy decisions land in the audit log |
| P5-O7 | OCR / vision fallback (`find --visual`) | When the AX tree is empty or the target isn't exposed (Canvas apps, Flutter-desktop, games, remote desktop, Figma plugins), `find --visual "label"` falls back to a per-window screenshot + OCR to locate text. macOS: `Vision` framework `VNRecognizeTextRequest`. Windows: `Windows.Media.Ocr.OcrEngine`. Linux: Tesseract via `tesseract` crate. Returns a synthetic ref that routes to coordinate events; clearly marked `source: "visual"` in output to signal reduced reliability |
| P5-O8 | OpenTelemetry trace export | Current `--trace <path>` writes redacted JSONL reliability diagnostics. Phase 5 adds trace IDs, span structure, `agent-desktop trace view <uuid>`, and OTLP/HAR export without changing the existing stdout JSON contract |
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
- The current CLI `--session <id>` persists snapshots on disk, scopes the latest-snapshot pointer, and (Phase 1.6) gates a manifest with automatic JSONL trace segments.
- The daemon upgrades that model to warm, in-memory per-session RefMaps while preserving explicit snapshot IDs as deterministic handles.
- Sessions are isolated: agent A's latest pointer never collides with agent B's latest pointer.
- Session destroyed on disconnect or explicit `session kill`.

**Health check:**
- `agent-desktop status` returns: daemon PID, uptime, active session count, platform, permission status

### New Commands

> `session start` / `session end` / `session list` / `session gc` and `trace export` / `trace show` already ship as of Phase 1.6 (see Phase 1's Commands Shipped table). The rows below describe the **remaining** daemon-era additions layered on top of those — `session kill` (daemon-specific teardown, distinct from the already-shipped `session end`), a friendlier `trace view` pretty-printer, and OTLP/HAR export flags — not a wholesale reimplementation of session or trace management.

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
| Every command gains `--trace-id <uuid>` | Correlate the existing `--trace` JSONL events and future daemon/MCP spans; auto-generated when not provided (P5-O8) |
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

Every destructive operation — `close-app`, `dismiss-all-notifications`, `set-value` (writes), `clear`, `drag`, `deliver-files`, `notification-action`, `batch` containing any of the above — supports three layered safety primitives that compose:

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
   {"ts":"2026-…","trace_id":"9f3c…","actor":"cli|mcp:claude-desktop","tool":"close-app","args":{"app":"Finder"},"policy_decision":"allowed","user_decision":"confirmed","exit":0,"prev_hash":"sha256:…","entry_hash":"sha256:…"}
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

### Trace Export + OpenTelemetry (P5-O8)

Today, callers opt into a redacted reliability trace with `--trace <path>` and may add `--trace-strict` to fail on setup or pre-action trace write errors; Phase 1.6 also made per-session tracing automatic under a manifest-gated `session start`. Phase 5 layers trace IDs and span/export tooling on top of that existing JSONL event stream:

```json
{"ts":"…","trace_id":"9f3c…","span_id":"…","parent_span_id":"…","name":"cli.snapshot","kind":"internal","attributes":{"app":"Finder","skeleton":true,"ref_count":14,"duration_ms":87}}
{"ts":"…","trace_id":"9f3c…","span_id":"…","parent_span_id":"<snapshot span>","name":"adapter.macos.get_tree","duration_ms":72,"attributes":{"surface":"window"}}
```

Phase 5 spans are OpenTelemetry-compliant so `agent-desktop trace export <uuid> --otlp` emits a valid OTLP JSON payload ingestable by Grafana Tempo / Jaeger / Honeycomb / Datadog. `--har` exports a HAR-like envelope for quick manual inspection. Screencasts from `--record-trace` attach as trace links.

### Enterprise Quality Gates

| Gate | Requirement |
|------|-------------|
| Security | No arbitrary code execution. No privilege escalation. All actions allowlisted via `Action` enum. Daemon socket scoped to user. Policy engine denies by default when the policy file is syntactically invalid. |
| Safety | Every destructive command supports `--dry-run`; every MCP destructive tool requires the `destructive` capability + audit log; the audit log is hash-chained and tamper-detectable; policy engine evaluated on every invocation. |
| Performance | Cold start <200ms. Warm snapshot <50ms via daemon. Tree traversal timeout 5s default, configurable. `watch --event` latency <500ms (push, not poll) per P2-O11. |
| Reliability | Zero panics in non-test code. Graceful daemon recovery on crash. Stale socket cleanup on startup. FFI panic boundary in release-ffi profile (already shipping). |
| Observability | Current commands can opt into redacted JSONL via `--trace`, with automatic per-session segments under Phase 1.6. Phase 5 adds daemon metrics, trace IDs, and OpenTelemetry OTLP export via `trace export --otlp`. |
| Compatibility | Tested against target app matrix: Finder, TextEdit, Xcode, VS Code, Chrome, Slack (macOS); Explorer, Notepad, Settings, VS Code, Edge (Windows); Nautilus, Terminal, Firefox, VS Code (Linux). |
| Distribution | Single binary per platform. No runtime dependencies for the CLI. FFI cdylib tarballs signed via Sigstore (already shipping as of Phase 1.5). Formula / manifest verify Sigstore attestation before installing (P5-O10). |
| Documentation | README, CLI reference, MCP reference, per-platform setup guides, troubleshooting, audit-log format reference, policy-file reference, OpenTelemetry trace schema. |
| FFI stability | Header drift check green on every PR. ABI version exported via `ad_abi_version()` (major currently 3, append-only since Phase 1.6). Pre-1.0: minor version bump for any public struct field add; major version bump for any removed or changed signature. |

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
| macOS | Homebrew | Formula in `<owner>/homebrew-tap` | `brew install <owner>/tap/agent-desktop` | Sigstore `cosign verify-blob` against release tarball |
| Windows | winget | Manifest in `microsoft/winget-pkgs` | `winget install agent-desktop` | Sigstore attestation check via `gh attestation verify` |
| Windows | scoop | Manifest in `scoop-extras` bucket | `scoop install agent-desktop` | Sigstore attestation check |
| Linux | snap | Snap package on snapcraft.io | `snap install agent-desktop` | Snap-native signature (snapd-signed) |
| Linux | apt | `.deb` in custom PPA (`ppa:<owner>/agent-desktop`) | `apt install agent-desktop` | Debian-native `Release.gpg` signature |
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

**Trace export tests (P5-O8):**
- Commands run with `--trace <path>` write at least one redacted JSONL reliability event
- `trace export <uuid> --otlp` produces a valid OpenTelemetry JSON payload that passes `otel-cli validate`
- A multi-command batch under a single `--trace-id` produces a single-rooted span tree (batch command is the parent)
- MCP sessions propagate the `trace_id` from the host's `initialize` params if provided; otherwise generate

**Install-time Sigstore tests (P5-O10):**
- Homebrew formula `install` step fails fast if the downloaded tarball's attestation fails verification
- Winget manifest includes a pre-install script that runs `gh attestation verify`
- Tampered tarball (bit-flip) reliably fails verification

### Skill Update

Skill maintenance rules:

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
| Phase 1 | Initial README: npm + source installation, core workflow, all 58 shipped command names (54 operational + 4 fail-closed), JSON output, ref system, error codes, platform support table (macOS only) |
| Phase 1.5 | Add "Language bindings (FFI)" section: platform→artifact table, 5-line Python dlopen snippet, `shasum -a 256 -c checksums.txt` + `gh attestation verify` verification, link to `skills/agent-desktop-ffi/` |
| Phase 1.6 | Note the breaking default-on auto-wait behavior and the qualified `@<snapshot_id>:e<n>` ref form in the CLI reference; no new distribution surface |
| Phase 2 | Add Windows: `.exe` installation, Windows permissions, update platform table, Windows build instructions |
| Phase 3 | Add Linux: binary installation, AT-SPI2 setup, update platform table, Linux build instructions, minimum OS versions |
| Phase 4 | Add MCP Server: `--mcp` usage, Claude Desktop config, Cursor config, tool-to-CLI mapping |
| Phase 5 | Add daemon mode, package managers (brew/winget/snap), performance benchmarks, final troubleshooting guide |

### Skill Maintenance Rules

Skill maintenance rules:

1. **Every new command** must be added to the appropriate `commands-*.md` file
2. **Every new platform** gets its own skill directory under `skills/agent-desktop-{platform}/`
3. **Every new mode** (MCP, daemon) gets its own skill file
4. **Breaking changes** to JSON output or CLI flags must update all affected skill files
5. **Skill files are reviewed** as part of the PR checklist for any command-surface change

### Command Surface DRYness (enforced across all phases)

See [Command Surface Architecture](#command-surface-architecture-dry-invariant) for the full layering. Summary of the invariant enforced on every PR:

- A new command creates exactly **one** file under `crates/core/src/commands/`.
- CLI and batch must share the typed `Commands` enum, `CommandPolicy`, and `dispatch()` path.
- Any future registry/codegen must be deterministic `build.rs` filesystem enumeration, not `inventory` or `linkme`.
- Per-platform work is limited to the `PlatformAdapter` capability-trait implementations in `crates/{macos,windows,linux}/` — never per-transport, never per-command.
- PRs that add a command to a single transport without updating the shared registry fail review. If a task in this document sounds like it requires per-transport duplication, it's a wording bug — the actual implementation follows the registry pattern.

### CI Matrix Evolution

| Phase | CI Runners / Jobs |
|-------|-----------|
| Phase 1 | `macos-latest` (tests + CLI build) + `ubuntu-latest` (`fmt` job) |
| Phase 1.5 | Same as Phase 1 on PRs; release workflow fans out to `macos-latest` × 2 darwin arches + `ubuntu-22.04` + `ubuntu-22.04-arm` + `windows-latest` for the FFI matrix |
| Phase 1.6 (current) | `ci.yml`: `fmt` (ubuntu-latest), `msrv` (ubuntu-latest, Rust 1.89.0), `platform-check` (matrix Linux/Windows/macOS, `cargo check` only — proves the stub crates compile before any adapter lands), `test` (macos-latest, full suite), `ffi-python-smoke`, `ffi-header-drift`, `ffi-panic-guard`, `ffi-passthrough` (ubuntu-latest). Outside `ci.yml`: `native-e2e.yml` (self-hosted macOS, workflow_dispatch), `codeql.yml`, `supply-chain.yml`. `scripts/perf-baseline-compare.sh` is currently a per-PR Definition-of-Done review step, not yet a blocking CI job |
| Phase 2 | `platform-check`'s Windows leg is promoted to a real `windows-latest` test lane (build, clippy, lib tests) at sub-phase 2.1; a self-hosted interactive Windows runner is added for UIA/shell integration tests at 2.12 |
| Phase 3 | `platform-check`'s Linux leg is promoted to a real `ubuntu-latest` test lane at sub-phase 3.1; an interactive Ubuntu GNOME runner is added for AT-SPI2/shell integration tests at 3.12 |
| Phase 4 | macOS + Windows + Ubuntu (+ MCP protocol tests) |
| Phase 5 | macOS + Windows + Ubuntu (+ daemon tests, package build verification) |

All runners enforce: `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `cargo tree -p agent-desktop-core` contains zero platform crate names, binary size <15MB. Every Phase 2/3 sub-phase additionally runs `scripts/perf-baseline-compare.sh` on hot-path changes — see the [Cross-cutting sub-phase DoD](#cross-cutting-sub-phase-dod).

### Dependency Introduction Schedule

| Dependency | Introduced In | Purpose |
|------------|---------------|---------|
| `clap` 4.6, `serde`/`serde_json` 1.x, `thiserror` 2.0, `tracing` 0.1+, `base64` 0.22+ | Phase 1 | Core: CLI, JSON, errors, logging, encoding |
| `tracing-subscriber` 0.3, `rustc-hash` 2.1, `smallvec` 1.13 | Phase 1 | Log formatter, fast hashing, small vectors in hot paths |
| `accessibility-sys` 0.2.0, `core-foundation` 0.10.1, `core-foundation-sys` 0.8.7, `core-graphics` 0.25.0 | Phase 1 | macOS AX API FFI |
| `cbindgen` maintainer tool, `libc` 0.2+ | Phase 1.5 | Explicit C header regeneration + macOS `pthread_main_np` for FFI main-thread guard |
| *(no new external crates — a contract-hardening pass over existing dependencies)* | Phase 1.6 | — |
| `uiautomation` 0.24+ | Phase 2 | Windows UIA wrapper |
| `windows` 0.62.2 | Phase 2 | Win32 / WinRT bindings (pinned to match `windows-capture 1.5` pin) |
| `windows-capture` 1.5.4 | Phase 2 | Modern `Windows.Graphics.Capture` screenshot |
| `objc2` 0.6 | Phase 2 | macOS safe Objective-C bridging (scoped to `system/screenshot.rs` + `system/permissions.rs`; CI grep guard) |
| `screencapturekit` 1.5 (crates.io) | Phase 2 | ScreenCaptureKit wrapper — published canonical crate, not a git fork |
| `atspi` 0.28+ + `zbus` 5.x | Phase 3 | Linux AT-SPI2 client via D-Bus |
| `tokio` 1.x | Phase 3 | Async runtime (required by atspi/zbus) — the first async runtime in the workspace; the codebase is synchronous through Phase 2 |
| `rmcp` 0.15.0+ | Phase 4 | Official MCP Rust SDK |
| `schemars` 1.2 | Phase 4 | JSON Schema generation for MCP tool parameters (deferred from Phase 2 per plan §KD15 — no Phase 2 consumer) |

All Phase 2/3 pins above were recorded at 2026-04 research time; re-verify against crates.io and the repository's supply-chain policy at the opening sub-phase of the consuming platform, per the [Platform Delivery Model](#platform-delivery-model--sub-phases-and-integration-branches).

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
| Stable ID | `AXIdentifier` / `AXDOMIdentifier` (shipped, U5) | UIA `AutomationId` (Phase 2, sub-phase 2.3) | AT-SPI2 `accessible-id` (Phase 3, sub-phase 3.3) |
| Click | `AXPress` | `InvokePattern.Invoke()` | `Action.DoAction(0)` |
| Set text | `AXValue = val` | `ValuePattern.SetValue()` | `Text.InsertText` |
| Keyboard | `CGEventCreateKeyboard` | `SendInput` | `xdotool` / `ydotool` |
| Clipboard | `NSPasteboard`, typed `ClipboardContent` (shipped, U18) | Win32 Clipboard API, typed (Phase 2) | `wl-clipboard` / `xclip`, typed (Phase 3) |
| Screenshot | `ScreenshotBackend` over secure `screencapture` path today; ScreenCaptureKit planned (P2-O13) | `BitBlt` / `PrintWindow` legacy, `Windows.Graphics.Capture` planned (P2-O13) | `PipeWire` / `XGetImage` (P3-O11) |
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
| R5 | Rust a11y crate maintenance stalls | Low | High | Pin versions, maintain patches. `atspi` backed by the Odilia project. Fork-ready. |
| R6 | MCP spec changes break compat | Low | Medium | Pin `rmcp` version. Monitor spec under Linux Foundation governance. |
| R7 | Tree traversal too slow (>5s) | Medium | Medium | Depth limiting via `--max-depth`. Focused-window-only. Cached subtrees in Phase 5 daemon. Progressive skeleton traversal (`--skeleton` + `--root`) reduces token consumption 78-96% for dense apps. |
| R8 | Ref instability confuses agents | Medium | High | Clear docs: refs are snapshot-scoped and snapshot-qualified (`@<snapshot_id>:e<n>`). `STALE_REF` error with recovery hint. Progressive skeleton traversal with scoped invalidation provides a stable drill-down workflow. Stable `native_id` evidence (shipped on macOS; Windows/Linux land it in their own vocabulary sub-phase) reduces stale-resolution failures on Electron and localized apps. |
| R9 | Headless operation requirement | High | Critical | Phase 1 introduced `ActionRequest`/`InteractionPolicy`, default no focus steal/cursor movement, and explicit physical/headed policy paths; Phase 1.6 added default-on auto-wait and the occlusion gate on top. Phase 2/3 preserve the same contract for Windows/Linux. |
| R10 | Command registry link-GC | Medium | High | Research Topic B confirmed `inventory`/`linkme` are unreliable across linkers for cdylib consumers. Resolved by pure `build.rs` filesystem enumeration — zero linker magic, once that registry migration (P2-O16) lands. |
| R11 | Skeleton traversal cross-platform | Low | High | Core is already platform-agnostic (`crates/core/src/snapshot_ref.rs`); Windows needs ~50 LOC glue (`ControlViewWalker` + `FindAll(TreeScope_Children, TrueCondition)` + fresh `UICacheRequest` per drill-down). Research Topic 4 confirmed `ElementFromHandle(hwnd)` is headless-safe. |
| R12 | RDP / session-isolation blocks Windows dev and CI | Medium | High | UIA requires an interactive session — an RDP disconnect can drop the console session to a non-interactive state. Document the `tscon` console-reattach workaround for the self-hosted runner (sub-phase 2.1); mirrors the macOS exclusive-desktop gate (`AGENT_DESKTOP_E2E_EXCLUSIVE=1` + `interaction_lock.py`) that already serializes native e2e runs. |
| R13 | UIA event handler MTA lifecycle leaks | Medium | Medium | `RemoveAutomationEventHandler` races the final in-flight callback dispatch on the MTA worker thread if torn down naively. Use the post-remove-barrier pattern (`Arc<Handler>` outlives the final callback) documented in the Windows Engineering Invariants — apply it from the first sub-phase that registers a handler (`watch`, once P2-O11 lands), not retrofitted after a leak is observed. |
| R14 | Merge-train discipline: integration branch drifts from `main` | Medium | Medium | 16 sub-phases landing serially into `feat/windows-adapter` (then `feat/linux-adapter`) is a long-lived branch by construction. Mitigate with a rebase cadence (rebase onto `main` at the start of each sub-phase, not just before the final merge) and treat each sub-phase's own review as a checkpoint rather than deferring all review to the 2.15/3.15 hardening pass. |
