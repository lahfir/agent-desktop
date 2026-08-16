# agent-desktop — Phase Roadmap

> Public source of truth for shipped and planned platform work.

---

## Release Tracker

Most recent shipments against this roadmap:

| Version | Date | What shipped |
|---------|------|---------------|
| v0.6.0 | 2026-07-25 | **Breaking:** removed the speculative Win32 private-file layer from core (six `private_file_windows*` files, `windows-sys` dropped from core and the workspace — Windows now uses the same portable `std::fs` path as every other non-unix target) and added real `test-windows` / `test-linux` CI lanes that execute core's platform-conditional code on every PR instead of only type-checking it. Windows private-file hardening becomes from-scratch Phase 2.1 work behind the adapter boundary. See [2.1](#21--toolchain-ci--com-bootstrap) and `docs/solutions/best-practices/never-ship-platform-code-that-ci-cannot-execute.md` |
| v0.5.0 | 2026-07-24 | Foundation reliability contract (Playwright-grade, U0–U19) — capability-supertrait `PlatformAdapter` split, canonical role/state vocabulary, live `is --property visible`, `list-displays` + honest `--screen` + scale factor, truthful Automation permission, `native_id` identity spine, window-id-first resolution, `LocatorQuery` + live `find`, default-on auto-wait, three-way occlusion gate, `scroll_into_view`, core accname computation, `supported_surfaces` introspection, typed `ActionStep` delivery tier, `ProcessState` + `APP_UNRESPONSIVE`, `LaunchOptions`, `SignalBaseline` + `wait --event`, typed clipboard, mouse modifiers/wheel, FFI ABI major 3, envelope 2.1. See [Phase 1.6](#phase-16--playwright-grade-foundation-contract-completed). |
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
- Phase 2: in progress. The entire Windows implementation lands here as dependency-ordered sub-phases 2.0–2.15 into the `feat/windows-adapter` integration branch — nothing Windows defers to a later phase (see the no-convenience-deferral rule in the [Platform Delivery Model](#platform-delivery-model--sub-phases-and-integration-branches)). v0.6.0 already put a real Windows test lane in place; 2.0 (probe corpus) is the first sub-phase PR.
- Phase 3+: planned. See each phase section below for the additive platform work and trait defaults that later phases backfill.

---

## Phase Overview

| Phase | Name | Status | Platforms |
|-------|------|--------|-----------|
| 1 | Foundation + macOS MVP | **Completed** (v0.1.0 – v0.1.14) | macOS |
| 1.5 | FFI Distribution (C-ABI cdylib) | **Completed** (v0.1.13; C-ABI surface completed v0.4.1) | macOS, Windows, Linux |
| 1.6 | Playwright-grade Foundation Contract | **Completed** (PR #93) | macOS (contract in core) |
| 2 | Windows Adapter | **In progress** — sub-phases 2.0–2.15; all Windows scope lands here | macOS, Windows |
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
- `WindowOp` — Resize{w,h}, Move{x,y}, Minimize, Maximize, Restore (`crates/core/src/window_op.rs`; window and app close is `close_app(&AppInfo, force)`, not a `WindowOp` variant)
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
    "snapshot_id": "s8f3k2p9",
    "complete": true,
    "tree": { ... }
  }
}
```

`data.complete` is present on every snapshot. A snapshot that exhausts its observation budget succeeds with `"complete": false`, the tree it did observe, `"truncated": true`, and `"nodes_observed"` — envelope 2.2 replaced the 2.1 `TIMEOUT` error on that path, so a consumer that branched on the error code to detect an oversized tree must read `complete` instead. Nodes whose descendants were cut short carry `"subtree_truncated": true`, serialized only when true. A `--root` drill-down replaces refs inside an existing snapshot, so an incomplete observation there still returns `TIMEOUT` rather than a partial tree.

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
| `INVALID_ARGS` | Input | Bad CLI argument, unknown ref format, or protected-process close refusal | Fix the argument per CLI help; target a regular application when closing |
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
- `NoopAdapter` (`tests/support/noop_ops.rs`) and per-test ad-hoc doubles: in-memory `PlatformAdapter` implementations returning hardcoded trees

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
| `platform-check` | matrix: Linux / Windows / macOS | `cargo check --all-targets` per platform crate + binary — proves every crate compiles on its target |
| `test-windows` | windows-latest | `cargo test -p agent-desktop-core -p agent-desktop-windows --lib` — added in v0.6.0; the first lane that ever executed core's `#[cfg(windows)]` code. Phase 2.1 extends it to clippy, binary-crate tests, the core-isolation check, and the size check |
| `test-linux` | ubuntu-latest | `cargo test -p agent-desktop-core -p agent-desktop-linux --lib` — added in v0.6.0; Phase 3.1 extends it the same way |
| `test` | macos-latest | Dependency isolation check (`cargo tree -p agent-desktop-core` has zero platform crate names), release-consistency check, file-size rule check, `cargo clippy --all-targets -- -D warnings`, core+macos unit tests, `locator_benchmark` example, `permission-contract.sh`, binary command tests, FFI integration tests, release binary build + version-flag check + 15MB size check, FFI cdylib build (`release-ffi` profile), FFI helper-discovery smoke, npm package tests + wrapper smoke |
| `ffi-python-smoke` | macos-latest | Builds the FFI dylib with the `stub-adapter` feature, runs `tests/ffi-python/smoke.py` against it |
| `ffi-header-drift` | macos-latest | `cbindgen 0.29.4 --verify` against the committed `crates/ffi/include/agent_desktop.h` |
| `ffi-panic-guard` | macos-latest | Asserts `profile.release-ffi` keeps `panic = "unwind"`; runs `crates/ffi/tests/run_cdylib_panic_probe.sh` |
| `ffi-passthrough` | ubuntu-latest | `cargo test -p agent-desktop-ffi --features stub-adapter --test c_abi_passthrough` — Family-B entrypoints (`ad_snapshot` / `ad_status` / `ad_wait` / `ad_execute_by_ref` / `ad_version` + `ad_init` / `ad_destroy` / `ad_check_permissions`) against the stub adapter |

Three more workflows run outside `ci.yml`: `native-e2e.yml` (workflow_dispatch only, self-hosted `[self-hosted, macOS, agent-desktop-e2e]` — builds and runs the exclusive native E2E suite via `scripts/run-native-e2e-ci.sh`), `codeql.yml` (pull_request + push to main/master + weekly cron + workflow_dispatch — CodeQL analysis for GitHub Actions/JS-TS and Rust), and `supply-chain.yml` (pull_request + push to main/master + weekly cron + workflow_dispatch — release-metadata consistency, npm package/publish policy, `cargo-deny`, `zizmor` workflow security audit). `release.yml` (push to main + workflow_dispatch) runs `release-please`, the per-target `build` and `build-ffi` matrices, `ffi-release-gates`, and the `publish-github` / `publish-npm` / `publish-skills` jobs.

Every substantive change also runs a performance baseline against the merge-base before merge, with latency deltas reviewed as intentional. On macOS the vehicle is `bash scripts/perf-baseline-compare.sh` → `report.html`. On Windows that script is structurally macOS-bound; the Windows vehicle is the probe corpus cost methodology (A15-13 / A18-7). This is part of `CLAUDE.md`'s Definition of Done as of the Phase 1.6 branch tip, and is treated as a per-sub-phase gate in Phase 2/3 (see [Platform Delivery Model](#platform-delivery-model--sub-phases-and-integration-branches) and [Cross-cutting sub-phase DoD](#cross-cutting-sub-phase-dod)).

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
| U11 | Core accessible-name computation takes precedence over adapter-supplied `NameEvidence`. Shipped as a contract, not as a call graph: `crates/core/src/accname.rs` is exported and tested but has no production caller, and the precedence that actually reaches every snapshot is `crates/macos/src/tree/query/evidence_fields.rs`, which ranks `description` fifth rather than seventh and carries a per-source uncertainty channel core's version cannot express. Sub-phase 2.3 reconciled them into one shared implementation both adapters call |
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

- One integration branch per platform: `feat/windows-adapter`, later `feat/linux-adapter`. **The integration branch is the base for everything that platform does.** Every sub-phase branch is cut from it and merges back into it — never from or into `main`. `main` stays the macOS-GA line for the entire duration of the platform's phase; it receives the platform only once, at the end, as the single promotion described below. Rebasing a sub-phase onto `main` mid-phase, or opening a sub-phase PR against `main`, is a process error.
- Each sub-phase is one PR into the integration branch, sized at or under 2,000 changed lines of hand-written product code (excluding `Cargo.lock`, the generated FFI header, vendored fixtures, and committed evidence artifacts — probe corpora and their captures, tree-dump censuses, and dogfood reports), reviewed on its own. The evidence exclusion states what practice already applies: 2.0 landed 21,048 insertions, 2.1 landed 4,921 against a ~1.3k estimate, and 2.2 landed 8,244, each overwhelmingly evidence rather than product code. A sub-phase whose *product code* exceeds the cap says so in its plan with the reason, rather than the cap being quietly ignored. Sub-phase branches are named `feat/windows-<sub-phase>-<slug>` after the sub-phase's own number (e.g. `feat/windows-2.0-probes`, `feat/windows-2.12.1-window-identity`) so the base relationship is legible from the branch name alone. A sub-phase inserted between two existing numbers takes a third component rather than renumbering its successors, which would silently change what an already-referenced number means — the same move Phase 1.5 and Phase 1.6 made at phase level.
- **Lifecycle per sub-phase:** plan (via `ce-plan`, written to `docs/plans/`) → implement → per-sub-phase review → merge to the integration branch. There is explicitly **no brainstorming stage** — this document is the finalized product contract; sub-phase planning documents implementation, not product scope.
- **The workspace stays green at every merge.** Unimplemented capabilities return `not_supported()`; the CLI ships honest `PLATFORM_NOT_SUPPORTED` envelopes on the target OS until the capability lands. No sub-phase merge may regress `main`'s CI.
- **Evidence-first rule:** adapter decisions are anchored in committed probe outputs — raw UIA / AT-SPI dumps checked in alongside the sub-phase plan — never docs-only assumptions about how a platform API behaves. The exploration sub-phases (2.0, 3.0) build the initial corpus; later sub-phases extend it.
- **Source-of-truth feedback rule:** this document was authored from documentation research in a single pass; the exploration sub-phases (2.0, 3.0) exist because platform reality outranks documentation. When a committed probe proves a documented approach behaves differently on the real platform — an API that answers differently, a pattern that is unavailable, an event that never fires — the finding ledger entry and the amendment to this document land **in the same PR**. The source of truth tracks proven platform behavior; it is never defended against evidence.
- **No convenience deferral.** The whole platform implementation lands inside its own phase's sub-phases — Windows under Phase 2, Linux under Phase 3. Scope may move *between* sub-phases of that phase; it may not move out of the phase because it is hard, large, or late. The only sanctioned deferral is **proven impossibility**: the platform genuinely cannot do it, evidenced by a probe row in that platform's findings ledger, and the command then ships an honest structured `PLATFORM_NOT_SUPPORTED` (or the applicable code) with `platform_detail` rather than a silent gap. Core features — observation, interaction, input, lifecycle, capture, clipboard, waits, and the shell surfaces the OS does expose — are a must, never a stretch.
- **Integration branch → main:** only after every sub-phase for that platform has merged **and the platform is production-solid as a whole** — not sub-phase-by-sub-phase, and not early because the branch has grown long-lived. Promotion to `main` runs a full multi-agent review of the whole branch, live e2e (both headless and headed) on the platform runner, a performance-baseline comparison against `main`, and the standard verification contract. It lands as one release-noted `feat!` merge — the same conventional-commit discipline as PR #93. Until that merge, `main` ships macOS only and makes no Windows claim.
- **Windows first.** Phase 2 (2.x) completes before Phase 3 (3.x) starts; Linux reuses the same sub-phase template with AT-SPI2/D-Bus substituted for UIA.

Every sub-phase below follows the same rendering shape: **Goal** (one or two sentences), **Scope** (what lands), **Key APIs** (platform surface touched), **Depends on** (prior sub-phase), **Exit criteria** (what proves it's done), **Est. PR size**.

---

## Phase 2 — Windows Adapter

**Status: In progress** — the entire Windows implementation is delivered as sub-phases 2.0–2.15 into the `feat/windows-adapter` integration branch per the [Platform Delivery Model](#platform-delivery-model--sub-phases-and-integration-branches), under its no-convenience-deferral rule: scope moves between these sub-phases, never out of Phase 2. v0.6.0 landed the prerequisite Windows test lane and removed core's unexecutable Win32 layer. This section is the public objective catalogue, the sub-phase implementation contract, and the preserved research (API mappings, capability maps, notification/tray approaches, Electron guidance) that grounds it.

### Core invariants (research-driven — from the Phase 2 plan's Headless-First Invariant)

1. **Headless-first inside the active desktop session.** Every command — existing and Phase 2 — must run without an agent-desktop GUI, foreground activation, focus steal, or physical cursor movement unless `--headed` explicitly opts into cursor input. Windows, macOS, and Linux still require the target app to exist in the current user's interactive desktop/display session for accessibility and capture APIs. Session 0, Server Core, secure desktops, locked desktops, and other-user sessions return `PLATFORM_NOT_SUPPORTED`, `PERM_DENIED`, or `WINDOW_NOT_FOUND` with `platform_detail`, not silent best effort. The invariant is enforced by integration tests: target window is NOT focused at test entry; `list-windows --focused-only` returns the same window before/after; cursor position unchanged for headless commands.
2. **Skeleton traversal is platform-agnostic.** The novel progressive skeleton pattern (depth-3 clamp + `children_count` annotation + drill-down via `--root @ref` + scoped invalidation via `RefMap::remove_by_root_ref`) lives entirely in `crates/core/src/snapshot_ref.rs`. Windows adapter contributes ~50 LOC glue: `FindAll(TreeScope_Children, TrueCondition)` for `children_count` + fresh `UICacheRequest` per drill-down. The enumeration walker itself is the **raw view**, which is what sub-phase 2.2 shipped (`crates/windows/src/tree/walker_source.rs:31`); the control view is a filter applied to that node set via `IsControlElement`, carried as evidence from 2.3 onward, not a second walk. Raw is a superset of control, so a role map total over `ControlType` stays valid under either.
3. **Asymmetric event threading.** The future push-based `watch` command (P2-O11) uses main-thread `AXObserver` on macOS (research-confirmed: Apple DTS says all AX is main-thread-only; AXSwift / Hammerspoon / Phoenix all do this); worker-thread MTA `IUIAutomation` event handler on Windows (Microsoft 2025 threading doc: UIA supports cross-thread event delivery). This is distinct from the already-shipped `wait --event`, which is an in-invocation `SignalBaseline` diff, not a subscription — see the naming note under 2.11 below.
4. **No `inventory` / `linkme` command registry.** Research confirmed neither survives link-GC reliably across ld64, ld-prime, GNU ld, lld, MSVC for cdylib consumers. Any future registry uses `build.rs` filesystem enumeration of `crates/core/src/commands/*.rs` — deterministic, cdylib-safe, zero linker magic. The repository's "one command per file" rule becomes the codegen contract when that migration (P2-O16) lands.
5. **FFI compatibility gates: shipped.** The ABI handshake (`ad_abi_version()`, `ad_init(expected_major)`) shipped in Phase 1.6; `AD_ABI_VERSION_MAJOR` is currently `3` and evolves append-only. Any new cross-platform ABI surface Phase 2 adds must preserve that handshake, not re-invent it.
6. **`DeliverFiles` replaces `FileDrop`.** Headless-first forbids `NSDraggingSession` on macOS; the new action uses a 4-tier fallback (URL scheme → `NSWorkspace.open` with `activates: false` → pasteboard + `Cmd-V` → AppleScript). Windows primary delivery is app/shell delivery (`ShellExecuteEx`, app URI handlers, `IFileOperation` for filesystem destinations, and `CF_HDROP` clipboard paste where accepted). `IDataObject + DoDragDrop` is an explicit policy-gated fallback/spike for targets that require drag semantics; it is never the default headless path.

### Windows Engineering Invariants (from the Phase 2 plan, Unit 3)

1. `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` at startup. Microsoft recommends setting process-default DPI awareness by application manifest rather than by API call; the API call is a deliberate divergence, justified because a cdylib has no manifest of its own. A second call fails `ERROR_ACCESS_DENIED`, which means the host already decided and is tolerated, never fatal. V2 is never asserted by reading awareness back: the V2 call succeeds on the 1809 floor but `GetProcessDpiAwareness` has no V2 enumerant and reports the V1 string `PROCESS_PER_MONITOR_DPI_AWARE`.
2. `CoInitializeEx(NULL, COINIT_MULTITHREADED)` on main thread and on every dedicated UIA worker thread (UIA prefers MTA).
3. Never cache `IUIAutomationElement` across apartments. Event handlers are created, registered, and removed on one dedicated MTA thread; they are not drained on it. Delivery is multi-threaded — callbacks arrive on several UIA-owned threads concurrently, the registering worker among them and the main thread never — so handler state must be safe for concurrent delivery rather than merely for one worker. Worker code re-resolves from `RefEntry` instead of moving elements across apartments.
4. UIA-first, SendInput-fallback (UIA patterns are focus-independent; `SendInput` is focus-dependent + UIPI-blocked for elevated targets).
5. `PostMessage WM_KEYDOWN` is DEAD for Chromium/UWP/games — not a viable alternative.
6. UIPI elevation detection via `GetTokenInformation(TokenIntegrityLevel)`. Ship `uiAccess=true` as optional signed release, not default.
7. `RemoveAutomationEventHandler` with post-remove-barrier pattern (Arc<Handler> outlives final callback dispatch).
8. HRESULT format in `platform_detail`: `COM HRESULT 0x80070005 (E_ACCESSDENIED: Access is denied)`.
9. `PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)` for legacy screenshot (mitigates DWM black frames). Modern capture is direct `Windows.Graphics.Capture` through the `windows` crate (`GraphicsCaptureItem` + `Direct3D11CaptureFramePool`), not the `windows-capture` crate — the mandated diff-audit rejected that crate as a video-recording library whose feature set includes `Win32_UI_Shell` (KTD2; crates.io 2.0.1 supersedes the once-recorded 2.0.0 pin).
10. `ElementFromHandle(hwnd)` is headless-safe for same-user, same-session visible/minimized windows at an accessible integrity level — the foundation of observation headlessness.
11. `Windows.Graphics.Capture` needs an active interactive session with DWM composition. Gate on the runtime `GraphicsCaptureSession::IsSupported` predicate and on successful interop activation, not on build number — A22-1 measured `IsSupported: true` at build 17763, below the documented 1903 floor. Where modern capture is unavailable or fails to activate (Session 0, Server Core, secure desktop, locked/remote sessions, or interop failure despite `IsSupported`), the product attempts Legacy silently rather than surfacing unavailability as an error; a `LEGACY_DEADLINE_FLOOR` reserves 200ms of the deadline for that attempt, so budget exhaustion or a Legacy failure can still error rather than guaranteeing success (P2-O13; dogfood J0).
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
| P2-O8 | Stable-selector expansion | `AccessibilityNode.native_id` remains the portable stable-ID field. Platform adapters preserve their strongest developer ID there (Windows UIA `AutomationId`, macOS `AXIdentifier` or `AXDOMIdentifier`, Linux AT-SPI `accessible-id`), while live locator traversal may retain both native IDs internally for strict matching. Phase 2 may add separately named `subrole`, `role_description`, `placeholder`, and `dom_classes` evidence without renaming or duplicating `native_id`. Resolver tests require controls with explicit IDs to survive re-drills; controls without one continue through the fingerprint fallback | Core contract + macOS `native_id` (`AXIdentifier`) shipped (U5). Windows `AutomationId` → `IdentifierEvidence` shipped in **2.2**, ahead of its nominal sub-phase: `crates/windows/src/tree/element_properties.rs` builds it typed as `IdentifierKind::AutomationId`, filters blank values, and reports a failed read as incomplete evidence rather than an absent identifier. Sub-phase 2.3 verifies and pins that behaviour and measures real coverage; it does not re-implement it. **Remaining:** `subrole`, `role_description`, `placeholder` and `dom_classes`, all assigned to sub-phase 2.4 |
| P2-O9 | `Action` enum expansion for 2026 agent workloads | New variants: `LongPress { duration_ms }`, `ForceClick`, `ShowMenu`, `DeliverFiles(Vec<PathBuf>)` (renamed from `FileDrop` — the original name implied `NSDraggingSession` which is not headless-compatible on macOS; see Core invariant 6), `WindowRaise`, `Cancel`, `SelectRange { start, len }`, `InsertAtCaret(String)`. `watch` is a new **command**, not an `Action` variant. Each has a macOS AX API mapping (all AX calls on main thread), a Windows UIA pattern mapping, a new CLI subcommand, FFI conversion coverage where applicable, and exhaustive platform-dispatch tests in the same change | **Remaining in full** — none of these variants exist yet; still Phase 2 scope |
| P2-O10 | `ErrorCode` expansion | Consider `PermissionRevoked` (distinct from `PermDenied` — TCC yanked mid-session) and `ResourceExhausted` (refmap >1 MB, tree node-count cap) | `APP_UNRESPONSIVE` shipped (U14) — a failed read-only liveness probe upgrades a hang to `APP_UNRESPONSIVE`; ordinary AX messaging exhaustion still reports plain `TIMEOUT`. **Remaining:** `PermissionRevoked`, `ResourceExhausted` |
| P2-O11 | Event-subscription primitive (push, not poll) | New **command** `watch --event <kind> --ref @s8f3k2p9:e5 --timeout 3000`, backed by a new adapter method distinct from `capture_signal_baseline`. macOS: `AXObserverCreate` + `AXObserverAddNotification` + `CFRunLoopSource`. Windows: `IUIAutomation.AddAutomationEventHandler` + `AddFocusChangedEventHandler` + `AddPropertyChangedEventHandler`. Linux mirrors in Phase 3 via AT-SPI2 D-Bus signals | `wait --event <kind>` (baseline-diff desktop-signal wait, U17) shipped and is **not** this objective — see the naming note under sub-phase 2.11. **Remaining in full:** the push `watch` command itself |
| P2-O12 | Text range primitives | Read caret, read selection, select a range by offsets, read text at range, insert at caret. macOS: `kAXSelectedTextRangeAttribute` (settable), `AXStringForRangeParameterizedAttribute`, `AXBoundsForRangeParameterizedAttribute`, `AXRangeForLineParameterizedAttribute`, `AXValueCreate(kAXValueCFRangeType, …)`. Windows: `TextPattern.GetSelection`, `TextPattern.DocumentRange`, `TextRange.Select`, `TextRange.Move`, `TextRange.GetText`, `TextRange.GetBoundingRectangles`. Commands: `text get-selection`, `text select-range <ref> <start> <len>`, `text insert-at-caret <ref> <string>`, `text at-offset <ref> <start> <len>` | **Remaining in full** — still Phase 2 scope |
| P2-O13 | Modern per-window screenshot APIs | macOS: replace `/usr/sbin/screencapture` subprocess with `SCScreenshotManager.captureImage(contentFilter:config:)` filtered to a specific `CGWindowID` from `SCShareableContent.windows`. Windows: `Windows.Graphics.Capture` via `GraphicsCaptureItem.CreateFromWindowHandle(HWND)` + `Direct3D11CaptureFramePool` when supported by the OS/session. No subprocess on the modern path, explicit fallback to legacy capture when unavailable, and permission/support failures map to structured `PERM_DENIED` / `PLATFORM_NOT_SUPPORTED` with `platform_detail` | `list_displays` + honest `--screen` targeting + `scale_factor` shipped (U3). **Windows direct WGC + silent Legacy fallback shipped in 2.10** (A22-1; dogfood J0 — do not gate on build; interop activate can fail with Legacy still succeeding). **Remaining:** ScreenCaptureKit modern macOS capture |
| P2-O14 | Toolbar and missing surfaces | Implement the core-predeclared surface vocabulary without changing core: `Toolbar` on both platforms; `Spotlight`, `Dock`, and `MenuBarExtras` on macOS; `Taskbar`, `SystemTray`, `SystemTrayOverflow`, `StartMenu`, `ActionCenter`, and `QuickSettings` on Windows where the current build/session exposes them. `NotificationCenter` remains the portable notification surface while `ActionCenter` names the distinct Windows shell entry point (Win10's Action Center; Windows 11 split it into Notification Center on Win+N and the separate Quick Settings on Win+A, so on Win11 the `ActionCenter` kind maps to the Win+N Notification Center) | `supported_surfaces()` introspection shipped (U12) — `SnapshotSurface` is ratified as genuinely platform-neutral, and every variant above already exists as a predeclared enum member. **Remaining:** the Windows shell surface implementations themselves |
| P2-O15 | Electron / WebView2 deep-tree toggles | macOS: `renderer_activation.rs` sets `AXManualAccessibility` on the app root after a capability probe (`renderer_probe.rs`) — **no Electron bundle-ID list**. Windows: detect Chromium/WebView2 via UIA `ClassName = "Chrome_WidgetWin_1"` (class still current). Chromium 138+ (Chrome shipped native UIA on by default, Aug 2025) exposes a UIA tree to any UIA client with no flag — built asynchronously, so a first read lands on the pre-activation shell and a tree is only judged thin after re-reading past a settle — and the web-wrapper depth-skip is the primary lever on modern builds; `--force-renderer-accessibility` guidance applies to pre-138/pinned builds or trees still thin after that settle. Both: new `--force-electron-a11y` CLI override | **Shipped in sub-phase 2.4** — Windows detection, settle and wrapper skip implemented (`crates/windows/src/tree/chromium.rs`, `wrapper.rs`); macOS's actual mechanism corrected in place |
| P2-O16 | FFI registry migration + parity expansion | Migrate `crates/ffi/` from hand-written `ad_*` wrappers to a `build.rs` codegen step that walks a compile-time command registry and emits one wrapper per command. After this, adding a CLI command automatically produces the FFI entry and the same descriptor metadata can feed JSON Schema / MCP generation in Phase 4. Marshaling helpers stay in `crates/ffi/src/convert/` — per-type, not per-command | **Remaining in full.** The ABI handshake (`ad_abi_version`, `ad_init`) and 8 command-backed entrypoints (`ad_snapshot`, `ad_execute_by_ref`, `ad_execute_by_ref_timeout`, `ad_wait`, `ad_version`, `ad_status`, `ad_trace_export`, `ad_trace_show`) shipped, but all 8 are hand-written — see Phase 1.5 Gap Status. This objective is exactly the codegen migration that turns those hand-written files into generated output |
| P2-O17 | Screen Recording / Automation permission detection | macOS exposes `PermissionReport { accessibility, screen_recording, automation }`. Automation is probed noninteractively for the remaining System Events-backed Notification Center opener; Accessibility and Screen Recording retain explicit preflight states | **Shipped** (U4 truthful Automation permission) |
| P2-O18 | Windows shell surface coverage | Add explicit shell coverage for Start menu/search, taskbar, system tray/overflow, Action Center/notification center, Quick Settings, multi-monitor/DPI, virtual desktop detection, UAC/elevated targets, RDP/locked-session behavior, and Explorer-specific file destinations. New commands are added only where a ref-based `snapshot --surface …` loop cannot expose the surface first; Windows-only behavior still routes through core command files and adapter trait defaults | **Remaining in full** — sub-phase 2.14, which ships inside Phase 2 before the 2.15 merge |

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
| `dom_id` / `dom_classes` | `kAXDOMIdentifierAttribute` / `kAXDOMClassListAttribute` | Windows: no producer — `uiautomation` 0.25.0 exposes no UIA `HtmlClass`/`HtmlId`, and A16-6 measured `AriaProperties` carrying no DOM class tokens; schema ships unproduced until a stack presents one | AT-SPI2 `object-attributes` HTML keys |
| Event subscription (`watch`) | `AXObserverCreate` + `AXObserverAddNotification` on `CFRunLoop` | `IUIAutomation.AddAutomationEventHandler` + `AddFocusChangedEventHandler` + `AddPropertyChangedEventHandler` | AT-SPI2 D-Bus signals via `zbus::StreamFactory` |
| Text range read | `AXStringForRangeParameterizedAttribute` + `AXSelectedTextRangeAttribute` | `TextPattern.GetSelection`, `TextPattern.DocumentRange.GetText` | AT-SPI2 `Text.GetText(start, end)` + `Text.GetCaretOffset` |
| Text range write | `AXSelectedTextRange = AXValueCreate(kAXValueCFRangeType, …)` | `TextRange.Select` + `TextRange.Move` | AT-SPI2 `EditableText.InsertText` + `Text.SetCaretOffset` |
| Modern per-window screenshot | `SCScreenshotManager.captureImage(contentFilter:config:)` | `GraphicsCaptureItem.CreateFromWindowHandle` + `Direct3D11CaptureFramePool` | PipeWire `org.freedesktop.portal.ScreenCast` |
| Toolbar surface — **predeclared in `SnapshotSurface` (U12)** | `AXRole == AXToolbar` or `AXUnifiedTitleAndToolbar` | UIA `ControlType.ToolBar` | AT-SPI2 `Role::ToolBar` |
| Menu-bar extras surface | `SystemUIServer` + `ControlCenter` pid walk | UIA `Shell_TrayWnd` + overflow flyout (`TopLevelWindowForOverflowXamlIsland` on Win11 22H2+/build 22623+; `NotifyIconOverflowWindow` only before that) | AT-SPI2 `StatusNotifierWatcher` D-Bus |
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

Every Phase 2 sub-phase below is held to the same definition of done, stated once here rather than repeated per sub-phase:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib --workspace`, and the relevant conformance suites (role/state vocabulary, contract tests) are green.
- Probe evidence — raw UIA dumps of the target app — is committed alongside the sub-phase's plan doc under `docs/plans/`.
- Adapters keep `not_supported()` defaults for every capability not yet landed in that sub-phase; the CLI surfaces `PLATFORM_NOT_SUPPORTED` honestly rather than a stub success.
- No core rewrites. Core changes land only via explicitly planned additive trait methods — never a signature change to something Phase 1/1.6 already shipped.
- Each sub-phase gets its own review before merging to `feat/windows-adapter`.
- Hot-path sub-phases (tree traversal, resolution, action dispatch) run a performance baseline against the merge-base before merge. On macOS the vehicle is `bash scripts/perf-baseline-compare.sh` → `report.html`. On Windows that script is structurally macOS-bound (it `open`s the `.app` fixture bundle); the Windows vehicle is the probe corpus cost methodology — min-of-seven with discarded warm-up, reported as min with median and max beside it (A15-13; applied to 2.6's `ElementFromPoint` costs in A18-7).
- **Every `FINDINGS.md` row whose action column names this sub-phase is disposed of before the sub-phase closes** — implemented, or re-assigned in this document with the reason. A row can assign work to a sub-phase and nothing today notices when that sub-phase ships without it: A1-3 assigned the UWP `CoreWindow` descent to 2.4 in sub-phase 2.0, 2.4 closed without it, and the gap surfaced only when an agent hit `ref_count: 0` against Settings six sub-phases later (§2.4.1). Listing the rows that name a sub-phase is mechanical and belongs in `13-ledger-check.ps1`; judging whether each was honored is not, and is the reviewer's obligation at close.
- **Dogfood is a gate, and its findings become tests. In force from §2.11 onward.** Every sub-phase drives its own surface against real software, not only fixtures, and commits a judged report. Three rules make that a gate rather than a ritual:
  - **A report with no findings is a failed dogfood, not a passed one.** Every sub-phase from 2.2 forward found something. A clean run means the run was too easy, and it is re-scoped against harder targets rather than accepted.
  - **Every finding gets exactly one of three dispositions, named in the report.** *Fixed here* — and it names the test that fails without the fix. *Owned elsewhere* — and it is written into the receiving sub-phase's scope in this document in the same PR. *Accepted* — and it states why closing it is not worth it. **"Recorded" is not a disposition**, and a finding left at "noted for later" fails the sub-phase's review.
  - **A finding disposed as fixed is not done until its test is invert-verified**: break the fix, watch the named test fail, restore. A regression test nobody has seen fail is an assertion about the fix, not a guard against its return.
- **Exit criteria enumerate; they do not gesture.** Every capability a sub-phase's Scope names appears in its Exit criteria, so a sub-phase cannot be declared done on scope it never proved. §2.12's exit criteria must name multi-monitor `list_displays` verification and split-integrity verification alongside every other Scope item — a gap 2.10 flagged when A22-7 / A22-8 deferred further capture legs onto those same items.
- **A sub-phase that adds a `probes/windows/<nn>-*` area registers that area in `.github/workflows/windows-capability-probe.yml`** in the same PR — both the `paths` filter and a run step — so a `-ci` capture label names the lane that produced it. Area 21's `-ci` captures landed before the workflow listed the area; 2.10 registered areas 21 and 22 with area 22 so the omission is not repeated.
- **Every requirement in the sub-phase's plan maps to at least one test that would fail if that requirement were violated.** A requirement with no such test is an open gap, not a documented one, and the plan's own Verification Contract states the mapping rather than leaving a reviewer to reconstruct it.
- Commits follow the repository's Conventional Commits requirement.

### 2.0 — Platform Exploration & Raw Scripting (pre-Rust)

**Goal:** empirically map Windows accessibility reality with raw, no-Rust scripts before any adapter code exists, producing a committed evidence corpus the Rust sub-phases implement against — and feeding every contradiction back into this document.

**Scope:** a `probes/windows/` directory of raw scripts — PowerShell using .NET managed UIA (`System.Windows.Automation`, preinstalled with .NET Framework 4.8) plus small C# programs compiled with `csc.exe` where UIA3 COM specifics differ from the managed wrapper. The corpus must cover, each as a runnable script with captured JSON/text output committed beside it: (1) full-tree dumps of Notepad, Explorer, Settings, and one Electron app (VS Code or Slack) including every property read per node — noting that "Notepad" is two different apps: Server SKUs ship the classic Win32 Edit-control Notepad while Windows 11 clients ship the Store/MSIX RichEdit Notepad, so tree expectations must name which variant they were captured against; (2) a pattern-availability census per ControlType (Invoke, Toggle, Value, RangeValue, ExpandCollapse, SelectionItem, Scroll, ScrollItem, Text, Window, LegacyIAccessible); (3) every interaction exercised raw — invoke, toggle, set value, select, expand/collapse, scroll via pattern AND wheel, text get/selection/caret/insert, focus; (4) SendInput synthesis experiments — keyboard incl. modifier chords and UTF-16 chunking limits, mouse click/move/wheel/drag; (5) `ElementFromPoint` hit-testing incl. deliberately occluded and zero-size targets; (6) `CacheRequest` batched reads timed against per-property reads; (7) AutomationId coverage census across Win32 / WinForms / WPF / Electron; (8) event-handler observations (which UIA events actually fire, ordering, MTA threading behavior); (9) elevation/UIPI behavior against an elevated process; (10) RDP-session and DPI/multi-monitor bounds behavior; (11) private-file I/O primitives — whether atomic rename over a concurrently-open handle requires `FILE_SHARE_DELETE`, whether an elevated process owns new objects as `TokenOwner` (e.g. `BUILTIN\Administrators`) rather than `TokenUser`, whether `GetFileInformationByHandleEx(FileRemoteProtocolInfo)` reliably distinguishes local from remote volumes, and what ancestor-vs-leaf ACL validation contract parity with the unix leaf-only rule actually needs (see `docs/solutions/best-practices/never-ship-platform-code-that-ci-cannot-execute.md`). Alongside the scripts: `probes/windows/FINDINGS.md` — a findings ledger mapping every experiment to observed behavior and a doc-alignment verdict (confirms this document / contradicts it / new edge case).

**Key APIs:** System.Windows.Automation, UIA3 COM (IUIAutomation) via csc.exe shims, SendInput, ElementFromPoint, CacheRequest.

**Depends on:** nothing — this is the entry point; the dev VM needs only its preinstalled .NET, PowerShell, and git.

**Exit criteria:** the script corpus and captured outputs are committed and re-runnable on the dev VM; the findings ledger covers tree, patterns, interactions, input, hit-testing, batching, identity, events, elevation, session, and private-file I/O behavior with no open "unknown" rows; every ledger entry that contradicts this document has a matching amendment to this document landed in the same PR (see the source-of-truth feedback rule in the Platform Delivery Model); no Rust adapter sub-phase (2.2 onward) starts until this exit gate is green.

**Est. PR size:** ~1.5k lines (scripts + ledger; no Rust).

### 2.1 — Toolchain, CI & COM Bootstrap

**Goal:** Stand up the Windows build/CI/session substrate so every later sub-phase lands on green CI and a constructible (if functionally empty) `WindowsAdapter`.

**Scope:**
- Extend the existing `test-windows` lane (shipped v0.6.0, runs core + `agent-desktop-windows` lib tests) to the full adapter surface: clippy `-D warnings` over `agent-desktop-core`/`agent-desktop-windows`/`agent-desktop`/`agent-desktop-ffi`, binary-crate tests (`cargo test -p agent-desktop` — `--lib` alone skips it, that crate has no lib target), core-isolation check, and a Windows-native release-binary size check. The binary-crate arm has a source prerequisite: `src/tests/snapshot_test.rs` builds the binary path without `std::env::consts::EXE_SUFFIX`, so `cargo test -p agent-desktop` fails 3 of 128 on Windows today. That fix lands before the lane extension — `src/tests/cli_process.rs` already uses `env!("CARGO_BIN_EXE_agent-desktop")` — or the lane lands red
- COM apartment and DPI bootstrap at process start, with different primitives for the two consumers. The CLI owns its process and uses `CoInitializeEx(NULL, COINIT_MULTITHREADED)`. The cdylib cannot: `CoInitializeEx` fails `RPC_E_CHANGED_MODE` against any host thread already in an STA, and its balance is per-thread, so it can never be released from a `Drop` running on another thread. The library path uses `CoIncrementMTAUsage`, whose cookie is thread-agnostic and which creates the MTA without converting a host thread that already chose STA. `RPC_E_CHANGED_MODE` means "borrowed the host's apartment, do not uninitialise" and is tolerated, never reported as failure. MTA is a requirement and not a preference — UIA documents that STA can prevent a client from removing event handlers. `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` runs in the same bootstrap on Invariant 1's terms: the API call is a documented divergence from Microsoft's manifest recommendation, `ERROR_ACCESS_DENIED` on a second call means the host already decided and is tolerated, and V2 is never asserted by reading awareness back
- `WindowsAdapterSession` implementing `AdapterSession` via `open_session` — owns COM apartment state so later sub-phases don't reinvent COM lifecycle
- Record the dependency pins below (re-verified against crates.io + supply-chain policy on 2026-07-25 during sub-phase 2.0 — see New Dependencies) without adding `uiautomation` (first consumed in 2.2) or taking the later-rejected `windows-capture` crate; 2.10 lands modern capture as direct WGC through the existing `windows` crate instead (KTD2). The Win32 bindings this sub-phase's own scope calls are added here, not deferred: `CoInitializeEx`/`CoIncrementMTAUsage`, `SetProcessDpiAwarenessContext`, and the ACL, `TokenOwner` and atomic-replace surface are implemented and unit-tested in 2.1
- Implement Windows private-file hardening from scratch. The seam is the sub-phase plan's decision and is not pre-empted here, but it is constrained on both sides and neither obvious option is reachable as stated: every private-artifact write site lives in `agent-desktop-core` (`refs_store.rs`, `session/mod.rs`, `trace.rs`, `trace_artifact_budget.rs`, `commands/clipboard_get.rs`) and none of them holds an adapter handle, so routing the hardening behind `PlatformAdapter` means threading one in first; and core may not depend on `agent-desktop-windows`, which dependency inversion forbids and `ci.yml`'s isolation check enforces. What is non-negotiable is the rule that killed the previous attempt: no platform code lands in core that no CI lane executes (see `docs/solutions/best-practices/never-ship-platform-code-that-ci-cannot-execute.md`). The hardening satisfies, with evidence from 2.0's probes: `ReplaceFile` — not `MoveFileEx` — for an atomic replace whose destination a validation handle holds open, plus `FILE_SHARE_DELETE` on every concurrently-open handle, which is necessary but never sufficient on its own: `MoveFileEx` issues `ReplaceIfExists` rather than POSIX-semantics rename and fails `ERROR_ACCESS_DENIED (5)` over an open target even with share-delete, while `ReplaceFile` honors share-delete on the destination and refuses an open handle on the source, so the two APIs have opposite tolerances on opposite sides and the failure to expect is error 5, not the `ERROR_SHARING_VIOLATION (32)` the source-side case returns; owner validation against `TokenOwner`, not `TokenUser`; locality inference from `GetFileInformationByHandleEx(FileRemoteProtocolInfo, class 13)` only behind a control call on a known-good info class, because the API signals "local" by failing with `ERROR_INVALID_PARAMETER (87)` instead of returning a local protocol value and an out-of-range info class returns that same 87, making the code ambiguous without the control; and an ancestor-vs-leaf validation contract decided deliberately, matching or explicitly diverging from the unix leaf-only rule, so the private-artifact-writing path is real before any Windows code writes a refmap or trace file

**Key APIs:** `CoInitializeEx`, `CoIncrementMTAUsage`, `SetProcessDpiAwarenessContext`, `ReplaceFileW`, `GetFileInformationByHandleEx(FileRemoteProtocolInfo)`, Win32 ACL / `TokenOwner` validation, `FILE_SHARE_DELETE` (private-file hardening)

**Depends on:** nothing (opening sub-phase)

**Exit criteria:** the Windows-relevant package set is green on Windows CI — `agent-desktop-macos` does not compile on Windows, so "workspace" invocations scope to `agent-desktop-core`, `agent-desktop-windows`, `agent-desktop`, and `agent-desktop-ffi`, exactly as the `test-windows` lane already scopes them; `WindowsAdapter` constructs and satisfies the trait; every adapter-backed command returns honest `PLATFORM_NOT_SUPPORTED` on Windows, while the commands that reach no adapter — `version`, `skills`, `session`, `trace`, `status`, `permissions` — succeed, `status` already reporting `platform: "windows"` with an empty `supported_surfaces`; the permission probe is unit-tested against mocked COM security state; private-file hardening is unit-tested on the `windows-latest` CI lane, not merely `cargo check`-clean, and asserts only what travels off the machine 2.0 measured — `windows-latest` now resolves to Server 2025 while every private-file observation was taken on Server 2019 build 17763, so the tests assert that a new file's owner equals `TokenOwner` and that validation resolves the nearest protected ancestor, never the specific SIDs or ancestor chain this one VM presented, and never the non-admin case 2.0 had no account to exercise.

**Est. PR size:** ~1.3k LOC (bootstrap ~0.8k + from-scratch private-file hardening ~0.5k)

### 2.2 — UIA Element Wrapper & Tree Walk

**Goal:** Own an `AXElement`-equivalent wrapper for UIA elements and prove raw tree traversal against a real Windows app before any semantics land on top.

**Scope:**
- `UIAElement` ownership wrapper — `AddRef`/`Release`, `Clone`/`Drop` safety mirroring the `AXElement` pattern (`pub(crate)` inner field to prevent double-free via raw pointer extraction)
- `ElementFromHandle` roots for window entry
- `TreeWalker` traversal with an ancestor-path cycle guard, never a global visited set. The macOS pointer-reuse rationale does not transfer and 2.0's probes measured nothing that would support it — neither 2.0 walker so much as calls `GetParent`: UI Automation hands back a *new* `IUIAutomationElement` proxy for every query, so pointer identity carries no information and the guard keys on runtime id (`GetRuntimeId`), with `CompareElements` as the fallback where a runtime id is unavailable. The ancestor-path requirement itself is unchanged, since a global visited set still prunes real subtrees
- `CacheRequest` attribute reads (the UIA analogue of `AXUIElementCopyMultipleAttributeValues`), batched conditionally rather than always. A6-1 measured a 220-node Explorer window at 2.69x overall with the find phase 1.5x *slower* and the read phase ~298x faster, and A6-2 measured classic Notepad — 3 nodes, served by `UIAutomationClientsideProviders` inside the client process, so an uncached read costs no cross-process RPC and the request adds pure setup — as a net pessimization at 0.5763x overall and 0.436x on the find phase. The rule that ships is the one the Windows API Mapping table below already carries: cache only the properties that will be read, and skip caching when a root-level `ProviderDescription` read indicates an in-process client-side provider. There is deliberately no node-count arm — the node count is unknown at the moment the cache request is built
- Committed probe examples: raw UIA dumps of Notepad and Explorer, checked in as evidence alongside the sub-phase plan

**Key APIs:** `IUIAutomation.ElementFromHandle()`, `IUIAutomationTreeWalker.GetFirstChild`/`GetNextSibling`, `CacheRequest` (`uiautomation` crate 0.25+ wrapping the `windows` crate's COM bindings; construct the client by direct `CoCreateInstance` on **`CUIAutomation8`**, never through `UIAutomation::new()`. `new()` calls `CoInitializeEx(None, COINIT_MULTITHREADED)` itself and proceeds on any non-negative HRESULT, so on a thread already in the MTA it reads `S_FALSE` as success and permanently leaks one initialization count — the type has no `Drop` and the crate never calls `CoUninitialize` — a leak Phase 5's long-lived daemon accumulates, and on any STA host thread it returns `Err(RPC_E_CHANGED_MODE)` outright. `UIAutomation::new_direct()` avoids both, but it is `CoCreateInstance(&CUIAutomation, …)`, and that object returns `E_NOINTERFACE` for `IUIAutomation2` — so its calls carry no timeout and `ElementFromHandle` against a window that stopped dispatching blocks indefinitely (A14-11, A14-12: 59.09 s through a 30 s watchdog, against `UIA_E_TIMEOUT` in 2.05 s once `SetConnectionTimeout` is set). `CUIAutomation8` is the same direct `CoCreateInstance` with the same no-`CoInitializeEx` property, and it exposes the timeout; there is no fallback to the unbounded client)

**Depends on:** 2.1

**Exit criteria:** an internal tree-dump binary prints Notepad and Explorer trees, batching reads only where the provider class warrants it — A6-2 measured unconditional batching against an in-process client-side provider as a pessimization, so "with batched reads" is not a criterion the dump can be held to; `CacheRequest` attribute-batching correctness is unit-tested, asserting that a cached read equals its uncached counterpart rather than that it is faster, since A6-1 and A6-2 disagree on the multiplier by provider class.

**Est. PR size:** ~2k LOC

### 2.3 — Vocabulary: Roles, States, native_id, Name Evidence

**Goal:** Map UIA's vocabulary onto the canonical role/state contract Phase 1.6 (U1/U2) already established, and wire `native_id` end-to-end for Windows.

**Scope:**
- `ControlType` → unified role enum map in `tree/roles.rs`. `ControlType` alone is not sufficient: A2-4 measured classic Notepad's edit surface as `ControlType.Document` on the COM client, and UIA has no control type for several canonical roles. The map keys on `ControlType` and refines with pattern availability and `ClassName`
- UIA states → canonical state vocabulary. Every emitted token must be a member of `crates/core/src/state.rs`'s `STATE_VOCABULARY`, asserted with a negative control so the assertion cannot pass vacuously. There is **no** adapter-parameterized conformance harness to reuse: Phase 1.6's U1/U2 left core-owned, adapter-agnostic predicates (`Role::is_canonical`, `roles::is_canonical_role`, `state::assert_states_in_vocabulary`) plus a per-platform test *pattern* proven once by macOS at `crates/macos/src/tree/roles.rs:150-216`. Windows writes its own module against those predicates
- Pattern availability → `available_actions`. The `Is*PatternAvailable` reads that refine the role map are the same ones the action list needs, and A2-1's action clause assigns 2.3 both the role and actionability tables. Core's ref allocation consumes the list (`crates/core/src/ref_alloc.rs:70-78`), so a snapshot without it under-refs. **`LegacyIAccessible` availability is not an affordance** — A2-2 measured it on 141 of 141 COM-walked elements, so mapping it to an action refs every node in every tree
- `AutomationId` → `native_id`: **the mechanism shipped in 2.2**, which builds `IdentifierEvidence::typed` with `IdentifierKind::AutomationId`, filters blank values, and reports a failed read as incomplete evidence. 2.3 verifies and pins it rather than re-implementing it, and measures real coverage
- `NameEvidence` supplier feeding one shared accessible-name computation. The `states` slot needs plumbing built, not a seam filled — 2.2 threads `role` and `available_actions` as parameters but hardcodes `states: LocatorField::Unknown`

**Key APIs:** UIA `ControlType` enum, UIA property IDs for state and pattern availability, `AutomationId`, `Name`/`LabeledBy`/`HelpText`/`FullDescription`. Pattern-derived state (`ToggleToggleState`, `ExpandCollapseExpandCollapseState`, `SelectionItemIsSelected`, `ValueIsReadOnly`, `SelectionCanSelectMultiple`, `WindowIsModal`) is read as **plain automation properties**, never by instantiating a pattern per node. `haspopup` and `busy` have no UIA property at all — Microsoft's ARIA mapping records them as MSAA `STATE_SYSTEM_HASPOPUP`/`STATE_SYSTEM_BUSY`, reachable only through `LegacyIAccessibleState`

**Depends on:** 2.2

**Exit criteria:** the `ControlType` → role map is **total by compilation** — an exhaustive `match` over the crate's 41-variant control-type enum with no catch-all arm, so coverage is a compiler guarantee rather than a test. Correctness is separate and is established by evidence: every emitted token is canonical; every arm is a member of the admissible set Microsoft's published `ControlType`↔ARIA-role table gives for that control type, which is many-to-one and therefore a containment check; every `INTERACTIVE_ROLES` member the adapter claims has a producer, and every unproduced role is listed; live fixture assertions hold; and the vocabulary is run against real applications with the findings recorded. **A per-arm equality table transcribed from the map's own arms does not satisfy this** — that is a test that cannot fail, the anti-pattern recorded in `docs/solutions/best-practices/never-ship-platform-code-that-ci-cannot-execute.md`. Accname tests pass on Windows.

**PR size (as shipped):** 3,580 insertions of hand-written product Rust (3,928 changed lines) plus ~16.3k lines of evidence artifacts — the vocabulary probe and its two-environment captures, the four dogfood captures, the ledger rows and the report. The `~1.5k LOC` estimate this line carried predated three things it could not have priced: the `states` slot had no plumbing at all rather than a seam to fill, the accessible-name reconciliation turned out to require one shared core implementation because the documented precedence had no production caller, and the available-actions table moved into this sub-phase. The product figure exceeds the 2,000-line cap and says so here rather than being defined away; the evidence figure is excluded by the cap's own list.

### 2.4 — Observation: Snapshot, Windows, Apps, Displays

**Goal:** Land the full read path — `snapshot`, `list-windows`, `list-apps`, `list-displays`, `focused_window` — including the Electron/WebView2 web-wrapper depth-skip that makes dense apps usable.

**Scope:**
- `observe_tree` is the seam; `get_tree` is the FFI compatibility wrapper built on top of it, and `get_subtree` stays unimplemented (no live caller on any platform). The CLI snapshot path drives `ObservationOps::observe_tree` through core's `renderer_accessibility::observe_tree` retry loop; Windows mirrors macOS's shape exactly (`crates/macos/src/tree/adapter.rs`) rather than a shared `SnapshotEngine`
- Skeleton glue: `FindAll(TreeScope_Children, TrueCondition)` for `children_count` + fresh `UICacheRequest` per drill-down (Core invariant 2 — ~50 LOC, core owns the rest). **The enumeration walker is the raw view**, which is what 2.2 shipped (`crates/windows/src/tree/walker_source.rs:31`); the control view is a filter over it, not a different walk. 2.3 carries `IsControlElement` and `IsContentElement` as evidence so a view filter is a decision this sub-phase can make on the node set rather than a walker choice baked in upstream. The role map is total over `ControlType` and therefore valid under either view, since raw is a superset
- **The four P2-O8 evidence fields land here**, with their sources identified and measured by this sub-phase's A16 probe family (sub-phase 2.4 U1): `role_description` ← `LocalizedControlType` (display text — never a mapping key, since Microsoft documents it as locale-dependent or provider-chosen), `placeholder` ← `HelpText` where it is not already serving as the description, `subrole` ← `AriaRole`, and `dom_classes`. **`dom_classes` has no Windows producer: `uiautomation` 0.25.0's `UIProperty` enum carries no `HtmlClass` (vendored `types.rs`, ids 30000-30159), and A16-6 measured `AriaProperties` carrying no class token — so the field ships as core schema with no producer, for whichever platform first presents one.** 2.3 deliberately left `LocalizedControlType` and `AriaRole` unread because nothing consumed them and each costs a property in every node's prefetch; they ride this sub-phase's batch instead. Until they land an agent receives none of the four on any platform
- **Two ref-able role arms, one now exercised and one still not.** 2.3 exercised every other `ControlType` that maps into `INTERACTIVE_ROLES` across four UI stacks — `Tab`, `TabItem`, `Spinner` and `DataItem` were observed for the first time after its dogfood run extended the scratch fixtures. **`Button` refined to `switch` is resolved**: sub-phase 2.4's fixture extension (a WPF `ToggleButton` advertising `Button`+`Toggle`) makes the snapshot emit `role:"switch"`, observed in the 2.4 dogfood run. **`DataItem` refined to `cell` is still not emitted**, with the reason now measured (probe A16-10 and the 2.4 dogfood report): WPF `DataGrid` cells carry `GridItem`/`TableItem` availability but present `ControlType.Custom` (50025) — which resolves to `Role::Unknown`, not `DataItem` (50029) — so neither the WinForms `DataGridView` (no patterns) nor the WPF `DataGrid` (Custom-typed cells) produces a `cell` ref. Whether `Custom` + `GridItem`/`TableItem` should refine to `cell` is a decision recorded in the 2.4 dogfood report for the reader. Evidence: `docs/dogfood-reports/2026-07-31-feat-windows-2-3-vocabulary-dogfood.md` and `docs/dogfood-reports/2026-08-01-feat-windows-2-4-observation-dogfood.md`
- `list_windows` — HWND-first identity with recycled-window-id corroboration (mirrors the macOS U6 window-identity-as-primary-key pattern)
- `list_apps`, `focused_window`, `list_displays` + per-monitor DPI `scale` (core's field is `scale`, not `scale_factor` — derived from effective DPI)
- **Web/Electron web-wrapper depth-skip** (Windows implementation of the pattern macOS ships): non-semantic wrapper elements (`UIA_GroupControlTypeId` / `UIA_CustomControlTypeId`) with empty `Name` AND empty `Value` — and empty `AutomationId` and no advertised action, gated on detected Chromium provenance — do not consume depth budget. The predicate consumes evidence already read (KTD6), so the skip costs nothing per node. Implement in `crates/windows/src/tree/wrapper.rs` as `is_web_wrapper`, mirroring macOS's `is_transparent_wrapper` (`crates/macos/src/tree/query/node_evidence.rs:13-38`). The gate matters: ungated, the predicate would also skip the anonymous `Group`/`Pane` containers native stacks are full of.
- **Chromium detection:** detect Chromium-based windows via the `Chrome_WidgetWin_1` top-level window class (A4-4 measured it on Obsidian's top-level and render-host windows). Chromium 138+ (Aug 2025) enables native UIA automatically when a UIA client connects, but builds the tree asynchronously — a first read returns the pre-activation shell and understates the tree by an order of magnitude, so thinness is only concluded after re-reading past a settle (core's activation loop: the adapter's `activate_renderer_accessibility` is the connection-plus-settle, and a Windows interaction-lease override unblocks it). When a tree is still minimal after the settle, the error `platform_detail` guides toward `--force-renderer-accessibility`, and the `--timeout-ms` snapshot flag (added by U1 item 11) covers the cold settle window
- **Resolver depth:** element re-identification searches to a resolve-scoped depth-50 constant (`MAX_RESOLVE_DEPTH`, mirroring macOS's `crates/macos/src/tree/resolve.rs:15`) — a distinct constant from `element.rs`'s `ABSOLUTE_MAX_DEPTH`; Electron elements commonly sit at depth 25+. Implement in `crates/windows/src/tree/resolve.rs`
- **Surface detection for Electron:** an Electron modal (file picker, dialog) may report as the focused window itself rather than a child; surface detection checks the focused window's own `WindowIsModal` before any child is consulted (mirroring `crates/macos/src/tree/surfaces.rs:128-146`), classifying a Chromium modal as a `Sheet` surface; implement in `crates/windows/src/tree/surfaces.rs`
- Progressive skeleton traversal (`--skeleton`, `--root`) needs no Windows-specific work beyond drill-down resolution — core owns the flow
- **What 2.2 hands over unresolved, and 2.4 must not assume away.** A target that dies part-way through a walk is **not detectable from the tree API**: A14-4 measured `get_next_sibling` returning the exact `code()`/`result()` pair a live provider returns at end-of-list, and A14-9 measured property reads answering locally with an empty string and no error, so a subtree whose provider died mid-list reports *complete* with fewer siblings than exist. Descent (`get_first_child`) does surface it as `E_FAIL`. A snapshot that must distinguish "this app has no more children" from "this app went away" needs an independent liveness check — re-resolving the root, or a process read — not the sibling terminator. The hang bound is the client's `ConnectionTimeout`, not the operation `Deadline`, which cannot interrupt a blocking `SendMessage` (A14-11)

**Key APIs:** `RawViewWalker` (shipped in 2.2) with `IsControlElement`/`IsContentElement` as the view filter, `FindAll`, `UICacheRequest`, `LocalizedControlType`, `AriaRole`, `Chrome_WidgetWin_1` class match

**Depends on:** 2.3

**Exit criteria:** every gate is rule-shaped rather than app-named, because nothing establishes which applications the runner image carries and a ref count or tree shape is an `app/provider` fact no CI assertion may rest on — A6-2 records the environment dependency that makes such numbers unportable, and 2.0's own scope rule already forbids generalizing them: `snapshot` against a resolvable window root returns a reffed tree with a non-empty descendant set; skeleton drill-down works; web-aware depth-skip demonstrably reduces the depth budget consumed on any wrapper-bearing target, and `--force-electron-a11y` returns no fewer refs than the same target without it; a modal dialog raised by a Chromium-based target is detected as a sheet surface. A gate whose target is absent from the runner image skips with the reason recorded, never a false green.

**Est. PR size:** ~2k LOC

### 2.4.1 — UWP Frame Descent

**Goal:** Make `ApplicationFrameHost`-hosted apps observable and driveable, by resolving a UWP window to the `CoreWindow` its content actually lives in rather than to the frame that hosts it.

**Scope:**
- **The gap, and it is user-visible.** A1-3 measured the shape in 2.0: Settings presents an `ApplicationFrameWindow` owned by `ApplicationFrameHost` containing a `Windows.UI.Core.CoreWindow` owned by `SystemSettings` — the top-level window a UWP app presents does not belong to the app's pid. The product consequence was reproduced by hand on the dev box during 2.10's planning: `snapshot` against Settings returns `ref_count: 0` and a bare `{"name":"Settings","role":"window"}` node, because the walk roots at the frame and never descends. Every ref-based command is therefore unusable against a UWP target, and deep-link `ms-settings:` URIs are the only navigation available. **This observation is a hand-reproduced field report, not a ledger row** — no capture backs it yet, and this sub-phase's first unit owes the probe and the row rather than citing prose as evidence
- **Why it was missed, which matters more than the miss.** A1-3's action column already assigned the fix: "2.4 UWP targeting must descend to the `CoreWindow` rather than match the top-level window's ProcessId." Sub-phase 2.4 carried it as two *measurement* legs, shipped without the descent, and closed. `crates/windows/src` contains no `CoreWindow` or `ApplicationFrame` handling of any kind. Nothing failed, because nothing checks that a row's action column was honored by the sub-phase it names — see the disposition rule added to the Cross-cutting sub-phase DoD
- **What the descent has to change.** Window-root resolution must recognize an `ApplicationFrameWindow` and root the walk at the hosted `CoreWindow`; `list_windows` / `list_apps` must attribute such a window to the **hosted app's** pid rather than the frame host's, or `--app Settings` never resolves; and `process_instance` identity, on which every stored ref and every strict re-resolution depends, must follow the hosted pid. That is observation and resolution work — 2.4's and 2.5's domain — not capture or input
- **The unmeasured part, settled first.** A1-3 proved the `CoreWindow` exists under the frame with the app's pid. Nobody has established that its *subtree* reads cleanly once descended into. If it is thin or empty the fix is a different shape entirely, so the probe leg runs before the design commits
- **Cloaked frames ride along.** A16-1's census recorded cloaked windows (`DWMWA_CLOAKED`) in the top-level enumeration, and UWP frames are the population that produces them; the descent has to decide what a cloaked frame means for `list_windows` rather than inherit whatever falls out
- **Relationship to §2.12.** §2.12 owns the `focused_window` frame-versus-`CoreWindow` *identity* question, which it can only answer on a rig with a modern-shell population. This sub-phase owns the *tree and resolution* behavior, which needs neither that rig nor §2.12's fixture — Settings is present and dumpable on the dev box today. If this sub-phase's descent settles the identity mapping as a side effect, §2.12's item narrows to confirming it on its own runner

**Key APIs:** `IUIAutomationElement` descent from the frame element, `ClassName` match on `ApplicationFrameWindow` / `Windows.UI.Core.CoreWindow`, `GetWindowThreadProcessId`, `DwmGetWindowAttribute(DWMWA_CLOAKED)`

**Depends on:** 2.4 (the walk and the inventories this corrects), 2.5 (stored-evidence resolution and `process_instance` identity)

**Sequencing:** lands after 2.11 and **before 2.13**, which packages the adapter for npm and release. **This blocks the Phase 2 promotion**: an adapter that returns zero refs against the most native app on the platform does not meet the "production-solid as a whole" bar that gates Windows reaching `main`.

**Exit criteria:** `snapshot --app Settings` returns a reffed tree with a non-empty interactive descendant set; a ref allocated inside a UWP window re-resolves strictly and survives a `--root` drill-down; `list-windows` and `list-apps` attribute a UWP window to the hosted app rather than to `ApplicationFrameHost`; the probe row backing the descent cites a committed capture; a gate whose target is absent from the runner image skips with the reason recorded, never a false green.

**Est. PR size:** ~1.2k LOC — the frame-descent predicate and walk rooting, pid re-attribution across both inventories, identity threading, the probe area and its rows, and the regression tests

### 2.5 — Resolution & Live Locator

**Goal:** Make refs and the live `find`/`get`/`is` commands (U7) work on Windows with the same strict-resolution guarantees macOS ships.

**Scope:**
- `resolve_element_strict*` from `RefEntry` evidence — `AutomationId`-first, fingerprint fallback, 0/1/N classification into `STALE_REF`/success/`AMBIGUOUS_TARGET`
- `get_live_value` / `get_live_state` / `get_live_actions` / `get_live_element` / `get_element_bounds`
- **Stored-evidence window resolution now corroborates handle ownership, not only process generation.** `WindowIdentityEvidence::verify_stored`, which `resolve_window_root` routes through, verifies both that the process at the stored pid is still the same generation (the token) and that the live HWND is currently owned by that pid (`GetWindowThreadProcessId` equality) before a stored ref's window root is trusted — closing the cross-process recycle case, where a destroyed HWND reused by a different process's window used to pass a token-only check. **A residual this does not close is stated rather than assumed away:** an HWND destroyed and reused by another window of the **same still-running process** resolves against the recycled window, and element-level exact-evidence resolution does not catch it — two instances of one dialog present identical `AutomationId`, `ControlType`, and `Name`, so the candidate matches and the sole-candidate arm resolves with no geometric corroboration. Bounds corroboration cannot close it either: `bounds_hash` is exact over absolute screen coordinates, so demanding it would fail every ref whose window or layout moved between snapshot and action, which is the common case. Closing it needs a per-window immutable identity `RefEntry` does not carry (the window's UIA `RuntimeId` or a creation ordinal); `RefEntry` cannot gain fields in this sub-phase. macOS is unaffected — `CGWindowID` is a per-session monotonic counter, not a recycled handle-table slot, so this is a Windows-specific schema question, not a resolver bug. Unmeasured: no probe has established the HWND uniqueness-counter wrap rate under real churn, and §2.12 measures it on the fixture and interactive runner that first make staged window churn observable. The schema question is §2.12.1's scope, not this sub-phase's, and §2.12.1 decides it on §2.12's measurement
- **The snapshot `value` slot, which is unclaimed until here.** `LocatorEvidence.value` has been populated on Windows from `ValuePattern.Value` since 2.2's read set, but no sub-phase scope claimed it, so nothing tests it and no plan describes it. It lands with the live readers because a control's value is live state rather than vocabulary — `crates/core/src/roles.rs:87-100` treats it as mutable and explicitly not stable identity. 2.5 owns its semantics, its coverage, and the **read side** of its secure-field behaviour: the content-free fingerprint evidence (bounds hash, child-index path, process token) that resolves a secure field, the `IsPassword`-gated candidate reads that ride the shared read, and the reader-path withholding (KTD10). The **action side** — typing into secure fields, action-failure echoes, post-action state reads — is owned by 2.7, not 2.6: 2.6's scope is hit-test occlusion and `scroll_into_view`, neither of which dispatches an action or reads post-action state, while 2.7 is exactly where `ValuePattern.SetValue()` and the `ActionStep` post-verification reads already land (KTD10's split, so the next security review verifies closure rather than rediscovering it; 2.7's own scope entry carries the detail)
- **`AutomationId` is not sufficient alone as a resolution key.** A7-3 measured Explorer keying list rows by row index: of 29 unique `AutomationId` keys after a folder gained and lost files, 29 re-resolved, 0 were lost, and **5 landed on a different element**. A7-1 measured coverage varying by an order of magnitude across stacks — 100% of WPF interactive elements, 97.6% of Explorer's, and **0% of Electron's 8**. Both are why `resolve_element_strict` needs role-conditional stable text identity alongside the id, and why it must be able to return `STALE_REF` for an id that still resolves
- `resolve_query` is **core-owned**: core builds and evaluates the `LocatorQuery` over `observe_tree` and calls `adapter.resolve_locator_anchor` only for selected-match hydration (`crates/core/src/live_locator/hydrate.rs`). Windows ships the anchor resolver plus the evidence the walk already produces, never a query evaluator (KTD2)
- `resolve_locator_anchor` + selected-hydration completeness: actions that read state must classify a **definitive absence** separately from a **transport failure** — port the lesson already encoded in macOS's `is_definitive_absence` (`crates/macos/src/tree/action_list.rs`) rather than re-deriving it from scratch
- **The resolver's error payload is mirrored from macOS per adapter, not shared with it.** Windows's `identity_unknown_error` and `mark_deadline_elapsed` construct the same `kind` / `complete` / `retryable` / `deadline_elapsed` `details` object macOS's `identity_unknown` and `mark_deadline_elapsed` construct, and core derives an error's retryability from that object (`crates/core/src/retryability.rs`), so the shape is a contract both adapters restate rather than one either owns. Sharing it needs a second core touch beyond this sub-phase's single sanctioned visibility promotion, and it needs the macOS crate — the GA line for the whole platform phase — changed to consume the core version. §2.15 owns that promotion and states what closes it; this sub-phase ships the mirrored constructors
- `get_live_actions` ships as a free projection with no production caller in core today — only test doubles invoke it (KTD9); it costs nothing beyond the shared read, so parity is kept for the 2.6 actionability preflight that is its intended neighbour
- the graded fallback's aggregate `STALE_REF` rate **inside web content** is measured by §2.12 against a target whose window state is controlled; the fixture-driven 0/1/N cases are this sub-phase's committed proof of the tier's semantics (A17-8, U7). A17-8's shell reading is not evidence that a dev box cannot reach web content: the same box returns a complete snapshot with refs against a restored Chromium/Electron target, and the eighteen-node shell it recorded is what a *minimized* one presents to any client for as long as it is held

**Key APIs:** `AutomationId` lookup, `CacheRequest`-scoped re-reads, UIA property read failure classification (COM HRESULT vs "element gone")

**Depends on:** 2.4

**Exit criteria:** `find`/`get`/`is` are live on Windows; `STALE_REF`/`AMBIGUOUS_TARGET` semantics are proven with committed probe evidence (0/1/N candidate cases).

**Est. PR size:** ~2k LOC

### 2.6 — Actionability & Occlusion

**Goal:** Port the Phase 1.6 auto-wait/occlusion gate (U8/U9) onto Windows so every ref action gets the same actionability guarantees before it fires.

**Scope:**
- `hit_test` three-way result via `ElementFromPoint` with window attribution consulted in every arm — same-root unrelated hits intercept on UIA's verdict alone; cross-window interception requires UIA and Win32 (`WindowFromPoint` → `GA_ROOT`) to agree on another window; any divergence (including the Win32-skip cell where Win32 still names the target's root against a differing hit root) yields `Unknown`. Probe failure, pre-probe guard failure, and ancestor-of-target landings also yield `Unknown`, never a false negative, matching `HitTestResult`'s `ReachesTarget | InterceptedBy | Unknown` contract. Against Chromium, a settled first-contact shell with no positive-area leaf is the measured host shape (A18-3; U6 dogfood J5); an ancestor landing on the render-host pane is the designed `Unknown` outcome for web content that is not yet hit-addressable, never a false interception. Hang defense is connection-timeout-bounded: `ElementFromPoint` against a never-pumping window fails inside ~2× the client's `ConnectionTimeout` and swallows to `Unknown` (A18-5) — no `WindowFromPoint`-first pre-probe required by that measurement. Provider bounding rectangles that extend outside their scroll viewport (A18-2) force viewport-intersection demotion for both the same-root out-of-viewport candidate-point guard and scroll-verification visibility — a full unclipped rect is not treated as fully visible
- `receives_events` is core-owned and derived from `hit_test` — Windows ships no separate evidence producer (`crates/core/src/actionability/receives_events.rs` is the single producer; the macOS crate carries zero occurrences of the check)
- hit-test-based occlusion evidence feeding the core auto-wait gate (U8) — the live-read `enabled`/`offscreen` `ElementState` fields that also feed the gate ship in 2.5 with `get_live_element` (KTD6, A17-8), and 2.6 owns the hit-test occlusion, not the live-read fields
- `scroll_into_view` via `ScrollItemPattern` — the only call in this sub-phase that mutates the target rather than reading it; delivery is judged by observation (KTD5), not by the write HRESULT, so a surface that invokes but does not prove visibility within the verification window reports `delivered_unverified` rather than verified auto-scroll (U6 dogfood J4 on Explorer). Its HRESULT-to-delivery classification is completed by 2.7's mutation-path delivery classifier, which routes this call site through itself when it lands
- **`resolve_window_strict` and `focus_window`, the minimum enabling surface for the headed path that exercises the gate.** These are not actionability evidence and would otherwise sit outside this sub-phase, but core's headed ref-action pipeline focuses the target window *before* the battery runs (`crates/core/src/ref_action_single.rs` → `headed_focus.rs`), and `hit_test` is only reachable under a headed policy — so without them no headed click could reach the occlusion gate at all and the sub-phase's own exit criteria would be unprovable. What ships is deliberately minimal: strict re-resolution of a stored window by HWND with pid and process-generation corroboration, and a focus path that re-reads the owning pid immediately before every native window-state write and qualifies success by live ownership, so a destroyed-and-recycled HWND fails closed as not-delivered rather than foregrounding another process's window. Fuller activation and focus policy — restore ordering, focus-steal budgets, cross-desktop and UIPI-boundary behaviour — is **2.9**'s

**Key APIs:** `ElementFromPoint`, `ScrollItemPattern.ScrollIntoView`

**Depends on:** 2.5

**Exit criteria:** zero-bounds / disabled / occluded envelopes match macOS (same error codes, same `disposition`) on the surfaces that exist before 2.12's fixture app — the probe scratch fixtures, the crate's fixture lane, and fake-driven pins. WPF exposes a live zero-size walk target (`0×0` bounds; A18-8); WinForms same-root in-window overlay is the live occlusion proof on a HWND-bearing stack (U6 dogfood J3), while HWND-less WPF peer overlays cannot be force-covered for a five-point intercept. The occlusion / zero-bounds / delayed-enable fixture targets that mirror the macOS e2e fixture remain 2.12 deliverables.

**Est. PR size:** ~1.2k LOC

### 2.7 — Semantic Action Tier

**Goal:** Land the UIA pattern-based semantic action dispatch, with the same typed `ActionStep` delivery reporting (U13) macOS already produces.

**Scope:**
- `execute_action` via UIA patterns: `InvokePattern` (click), `LegacyIAccessible.DoDefaultAction` (legacy-only click when Invoke is absent — A19-6 measured a functional effect on the COM stack and Notepad Document; the advertisement's Legacy arm stands per A2-2), `TogglePattern` (toggle/check/uncheck), `ValuePattern` (set-value), `ExpandCollapsePattern` (expand/collapse), `SelectionItemPattern` (select), `RangeValuePattern`, `ScrollPattern`
- Activation chain with `ActionStep` delivery reporting + post-verification reads — honest `verified: Option<bool>` semantics, no step claims an effect it did not observe
- **The mutation-path delivery classifier, which is what makes that reporting honest.** Every pattern invocation and property write turns its `UiaFailure` (HRESULT or sentinel, never collapsed) into a delivery outcome exactly as macOS's `ax_mutation::classify` (`crates/macos/src/actions/ax_mutation.rs`) does for `AXError`: **delivered** on a clean write; the affordance **genuinely absent** (`UIA_E_NOTSUPPORTED` or the empty-pattern sentinel at `get_pattern` — A19-2) as `Ok(false)`, the chain's fall-through signal rather than an error; **not-delivered** denials and rejections (`E_ACCESSDENIED` → `PERM_DENIED`, `UIA_E_ELEMENTNOTAVAILABLE` → `STALE_REF`, `E_INVALIDARG` → `INVALID_ARGS`, `UIA_E_ELEMENTNOTENABLED` → `ACTION_FAILED` — A19-2 live-staged the enabled and unavailable arms); and **delivery-uncertain** transport / unclassified failures (`RPC_E_SERVERFAULT`, `RPC_E_DISCONNECTED`, `RPC_S_SERVER_UNAVAILABLE`, `RPC_S_CALL_FAILED`, `UIA_E_TIMEOUT`, and every unmapped HRESULT/sentinel) carrying `DeliveryDisposition::DeliveryUncertain` with `RetryDisposition::Unsafe`, the way macOS routes `kAXErrorCannotComplete` through `DeliveryTracker::uncertain`. The shipped read-path classifier does not answer this and must not be overloaded to: `hresult_record` / `ReadDisposition` (`crates/windows/src/system/hresult.rs`) classifies whether a failed **read** may be repeated, and the transport codes it marks `Retryable` are exactly the ones a **write** may already have delivered — reporting those as safe to retry is how one click gets dispatched twice. Sub-phase 2.5 kept the two apart deliberately and had no mutation call to classify. This sub-phase is the first that invokes a pattern, and the classifier's output *is* the `ActionStep` delivery report it already owns, so it lands here rather than in 2.6, whose single mutating call — `ScrollItemPattern.ScrollIntoView` — is routed through the classifier as part of this work (2.7 depends on 2.6, so the retrofit runs in that direction and never the other)
- **The ancestor-scroll fallback ladder for elements without `ScrollItemPattern`**, deferred from 2.6 (which mutates only via `ScrollIntoView`) and shipped here. Mirrors macOS's `scroll_ancestor_until_visible` (`crates/macos/src/actions/scroll_into_view.rs:35-60`): walk scrollable ancestors via `ScrollPattern` until the target is visible or the chain is exhausted (A19-7 measured a two-ancestor geometry ladder). An exhausted ladder reports `ACTION_FAILED` with `delivered_unverified`. Thin-`ScrollItem` surfaces that invoke but do not prove visibility keep the observation-judged `delivered_unverified` arm (2.6 U6 dogfood J4; 2.7 dogfood J3 re-judged Explorer click through `InvokePattern` with honest `delivered_unverified`). Shell virtualization that never realizes below-fold rows is owned by §2.12's denser Explorer / fixture surface — the ladder is present; unrealized Items View rows are not a missing-ladder defect (2.7 dogfood J3 residual)
- **`Action::SetFocus` policy.** UIA `SetFocus` moves the desktop foreground even when `SetForegroundWindow` is never called (A3-4; A19-5 re-confirmed on the COM product stack), so element-level `SetFocus` is headed-only with focused-element verification and fails headless with `POLICY_DENIED`. Headless dispatch never calls `SetFocus` implicitly as a chain step. Window-activation focus-steal budget (restore-versus-raise, cross-desktop, UIPI) remains §2.9 — that sub-phase inherits only the window half, never re-decides this element-level gate
- **The secure-field action side, split from 2.5's read side (KTD10).** 2.5 closes the read side — content-free fingerprint resolution, `IsPassword`-gated candidate reads, reader-path withholding. 2.7 closes the rest: `ValuePattern.SetValue()` into an `IsPassword` element goes through the same headless-first policy as any other field, but the `ActionStep` delivery report and any post-action state read must withhold the field's content exactly as `get_live_value` already does — the activation chain does not get to leak what the reader path was built to withhold (A19-3: write lands, marker never echoes, verification cannot observe the secret). An action-failure echo (the error path when `SetValue()` rejects the write) is held to the same rule: it may report failure, never the value it attempted or found
- See the Windows API Mapping table below for the full click/set-text/expand/select/toggle/scroll pattern list

**Key APIs:** `InvokePattern.Invoke()`, `LegacyIAccessible.DoDefaultAction()`, `TogglePattern.Toggle()`, `ValuePattern.SetValue()`, `ExpandCollapsePattern.Expand()/.Collapse()`, `SelectionItemPattern.Select()`, `RangeValuePattern.SetValue()`, `ScrollPattern.Scroll()`/`.SetScrollPercent()`, `UIElement.SetFocus()`

**Depends on:** 2.6

**Exit criteria:** click / set-value / clear / select / toggle / expand / collapse work headless on the fixture app via the e2e analog (sub-phase 2.12 supplies the fixture; this sub-phase's own tests use ad hoc Notepad/Explorer targets until then); every UIA write in the adapter — including 2.6's `ScrollItemPattern.ScrollIntoView` — reaches its caller through the mutation-path classifier, with one case per outcome pinning the mapping: an unsupported pattern falls through the chain rather than erroring, a denied write reports `PERM_DENIED` with `not_delivered`, a vanished target reports `STALE_REF` with `not_delivered`, a not-enabled write reports `ACTION_FAILED` with `not_delivered`, and a transport or unclassified failure reports `delivery_uncertain` with retry `unsafe`; headless `SetFocus` reports `POLICY_DENIED` with `not_delivered` (A19-5); no write path consults `classify_read_hresult`.

**Est. PR size:** ~2k LOC, of which the mutation-path classifier and its per-outcome tests are a few hundred. This sub-phase is the closest to the 2,000-line cap of any in the phase; if it presses past, the pattern dispatch splits from the activation chain along the seam the classifier already draws, never by dropping the classifier

#### Windows API Mapping (reference table for sub-phases 2.2–2.10)

| Capability | Technology | Details |
|------------|-----------|---------|
| Tree root | `IUIAutomation.ElementFromHandle()` | Via `uiautomation` crate (v0.25+) wrapping UIA COM APIs via `windows` crate. Construct by direct `CoCreateInstance` on `CUIAutomation8`, never `UIAutomation::new()` — the latter initializes COM itself, takes `S_FALSE` as success on an MTA thread and leaks one initialization count per call, and returns `Err(RPC_E_CHANGED_MODE)` on an STA host thread. `UIAutomation::new_direct()` avoids that but builds `CUIAutomation`, which has no `IUIAutomation2` and therefore no call timeout (A14-12); `CUIAutomation8` sets `ConnectionTimeout`, which is what stops a non-dispatching target hanging the caller |
| Children | `IUIAutomationTreeWalker.GetFirstChild` / `GetNextSibling` | With `CacheRequest` for batch attribute retrieval. The speedup is a phase split, not one multiplier: measured on UIA3 COM over a 220-node Explorer window reading 8 properties, building the cache makes the find pass *slower* (180 ms vs 117 ms) and the property-read pass ~300x faster (372 ms vs 1.2 ms), netting ~2.7x for a single full-tree read. Cache only the properties that will be read, and expect the win from repeated reads over a cached tree rather than from the walk. On plain Win32 trees served by in-process client-side providers, unconditional caching is a net pessimization |
| Role mapping | `UIA ControlType` integers | Map to unified role enum in `tree/roles.rs` — e.g. `UIA_ButtonControlTypeId` → `button` |
| Click | `InvokePattern.Invoke()` / `LegacyIAccessible.DoDefaultAction()` | Pattern-based primary activation; Legacy rung ships for Invoke-absent surfaces (A19-6, A2-2). Coordinate click via SendInput only under explicit physical policy (§2.8) |
| Set text | `ValuePattern.SetValue()` | Headless value write by default; SendInput only under explicit focus/physical policy |
| Expand/Collapse | `ExpandCollapsePattern.Expand()` / `.Collapse()` | Native UIA pattern |
| Select | `SelectionItemPattern.Select()` | For combobox, listbox, tab items |
| Toggle | `TogglePattern.Toggle()` | For checkboxes, switches |
| Scroll | `ScrollPattern.Scroll()` / `ScrollPattern.SetScrollPercent()` | Native UIA scroll; mouse wheel only under explicit physical policy |
| Keyboard | `SendInput` API | `INPUT_KEYBOARD` structs with virtual key codes and scan codes |
| Mouse | `SendInput` API | `INPUT_MOUSE` structs with `MOUSEEVENTF_*` flags |
| Clipboard | `OpenClipboard` / `GetClipboardData` / `SetClipboardData` | Win32 APIs; `CF_UNICODETEXT` for text, `CF_DIB`/PNG for image, `CF_HDROP` for file lists — marshaled through typed `ClipboardContent` |
| Screenshot | `Windows.Graphics.Capture` (direct) | Runtime precedence Modern → Legacy: prefer `GraphicsCaptureItem.CreateFromWindowHandle` + `Direct3D11CaptureFramePool` when `GraphicsCaptureSession::IsSupported` is true and interop activates (A22-1 — do not gate on build number). No subprocess, respects DWM compositing. `BitBlt` / `PrintWindow` (`PW_RENDERFULLCONTENT`) is the silent Legacy fallback when modern is unavailable or fails to activate; never via the rejected `windows-capture` crate (KTD2) |
| App launch | `CreateProcess` / `ShellExecuteEx` | Launch by name or path via `LaunchOptions` (args/env/cwd/attach-if-running), wait for main window |
| App close | `WM_CLOSE` / `TerminateProcess` | Graceful close first, force kill with `--force`; verified via `ProcessState` |
| Window ops | `SetWindowPos` / `ShowWindow` | Resize, move, minimize (`SW_MINIMIZE`), maximize (`SW_MAXIMIZE`), restore (`SW_RESTORE`) |
| Permissions | COM security / UAC | Detect elevation requirements; return `PERM_DENIED` if UIA access blocked |
| Notifications | UserNotificationListener + UIA Action Center fallback | See Notification Management approach under 2.14 |
| System tray | UIA + Shell_TrayWnd | See System Tray approach under 2.14 |
| Start menu / search | UIA + explicit shell open command | See Windows-specific command surface under 2.14 |
| Taskbar | UIA + Shell_TrayWnd task list | See Windows-specific command surface under 2.14 |
| Quick Settings | UIA shell flyout | See Windows-specific command surface under 2.14 |
| Virtual desktops | `VirtualDesktopManager` CLSID constant only — dropped from 2.4 | A16-9 measured that the `VirtualDesktopManager` CLSID constant exists in `windows-sys` 0.61's `Win32_UI_Shell`, but neither `windows-sys` nor `windows` generates the `IVirtualDesktopManager` **interface** it would activate; reaching it needs a hand-declared COM binding, a new dependency 2.4 does not take. "Current desktop" filtering and diagnostics are dropped from 2.4 on that measurement; moving windows between virtual desktops remains deferred unless a stable public API path is validated |
| Multi-monitor / DPI | Per-monitor DPI + Win32 monitor APIs | All bounds are physical pixels normalized by the same DPI-aware process mode; tests cover mixed-DPI monitor layouts before any coordinate fallback ships |

### 2.8 — Input Synthesis

**Goal:** Land raw OS input (keyboard, mouse, drag) matching the macOS delivery-tracking and headed/headless policy contract.

**Scope:**
- Three `InputOps` methods: `mouse_event` and `drag` functional via `SendInput`; `key_event` an honest rejection stub (held input is daemon-owned, KTD7); clipboard methods stay defaulted (`§2.10`)
- `SendInput` keyboard map + `type_text` with UTF-16 chunking for surrogate pairs (A4-1)
- Mouse events, modifier chords, and wheel (A4-2, A4-3); coordinate transform handles primary-monitor and virtual-desktop normalization (A20-5 single-monitor branch)
- Physical `execute_action` legs for `TypeText`, `PressKey`, `DoubleClick`, `TripleClick`, and `RightClick` — ref-addressed paths verify focus persisted before injection (A20-4)
- Drag with delivery tracking and release guard (A20-3)
- Windows blocked-combo list wired into `is_blocked_combo` (`alt+f4`, `win+l`, `win+d`, `alt+tab` and aliases)
- Headed/headless policy parity — raw cursor commands (`hover`, `drag`, `mouse-*`) require `--headed`, same as macOS
- UIPI elevation detection via `GetTokenInformation(TokenIntegrityLevel)` → `PERM_DENIED` with `platform_detail` in the `COM HRESULT 0x80070005 (E_ACCESSDENIED: ...)` format (A9-2, A20-1). A19-4's residual is **closed on the detection surface**; the cross-boundary input-write *effect* was unmeasurable on every probe host (`Start-MediumIntegrityProcess` privilege gate, A19-4/A20-2) and is owned by §2.12's split-integrity item
- **`type` divergence (KTD8):** Windows UIA has no insert-at-selection semantic path — `ValuePattern.SetValue` replaces the whole value (`set-value` is the headless text write) and `TextPattern` is read-only for insertion — so `type` is physical synthesis under the focus-fallback/headed policy and strict-headless `type` fails at policy where macOS would succeed via `AXSelectedText`. Settlement at §2.15
- Ref-addressed `--from <sliderRef>` drag resolves the element bounds center; WinForms `TrackBar` thumb pickup may need a thumb-row Y offset because UIA center Y misses the horizontal track on some hosts (2026-08-07-002 dogfood J6) — core owns `point_resolve`; slider-aware pickup is a future refinement

**Key APIs:** `SendInput` (`INPUT_KEYBOARD` / `INPUT_MOUSE`), `GetTokenInformation`

**Depends on:** 2.7

**Exit criteria:** `InputOps::mouse_event`, `drag`, and `key_event` stub live; no physical `execute_action` arm returns `PLATFORM_NOT_SUPPORTED` for a capability this sub-phase owns; UIPI detection proven by local token read plus synthetic-SID unit tests (A20-1), with PERM_DENIED mapping riding A9-2; headed input dogfood passes against repo-controlled targets (2026-08-07-002 dogfood report — Notepad A4-1 matrix, ScratchForms mouse/drag/multi-click); hot-path cost baseline committed (A20-6). Full headed e2e gesture matrix waits on §2.12's fixture; interim coverage is the dogfood run above.

**Est. PR size:** ~2k LOC

### 2.9 — System Lifecycle

**Goal:** Land process/window lifecycle — launch, close, resize/move/minimize/maximize/restore — with the same `ProcessState` liveness contract (U14) macOS ships.

**Scope:**
- `launch_app` with `LaunchOptions` via `CreateProcessW` (args/env/cwd — Windows honours a launch `cwd` where macOS Launch Services rejects it — attach-vs-fail via `attach_if_running`). The `id` is an absolute path (drive + backslash, UNC, or `\\?\`) or a bare name that resolves only against the system directories (`System32`, the Windows directory) — never the calling-process directory, the current directory, or `PATH`, so a planted same-named binary cannot hijack execution (A21-1; `launch_path.rs`). Any other identifier is `INVALID_ARGS` before a native call. `CreateProcessW` is chosen over `ShellExecuteEx` because it is manifest-ready under already-enabled `Win32_System_Threading` and returns a `PROCESS_INFORMATION` handle that is a strong single identity primitive, so Windows needs less "launched" verification ceremony than macOS's four-signal `NSWorkspace` dance (A21-1, A21-8). Launch-by-display-name / AUMID (`ShellExecuteExW` / `IApplicationActivationManager`) is out of this sub-phase: A21-8 validated the shell binding in standalone scratch with `expand_2_9: false`, and §2.14 owns the capability
- `close_app` with verified termination — success only after the process is observed gone via creation-time token + `WaitForSingleObject`/`GetExitCodeProcess` (A21-3), never an optimistic `closed: true` before exit (mirrors the v0.3.0 macOS `close-app` correction). Graceful posts `WM_CLOSE` to every top-level window owned by the target pid; force uses `TerminateProcess`. A protected process is refused with `INVALID_ARGS` + `not_delivered` before any native close (`crates/core/src/commands/close_app.rs` via `invalid_input_with_suggestion`; dogfood J2) — not `PERM_DENIED`
- `window_op` (`SetWindowPos`/`ShowWindow` for resize/move/minimize/maximize/restore), verified by `GetWindowPlacement`/`GetWindowRect` re-read within an 8 px tolerance after ~80 ms wait-then-re-read (A21-5) — never UIA `-32000`/`IsOffscreen` (A1-2/A5-3/A14-8)
- `ProcessState` probes: `IsHungAppWindow` as a cheap pre-check that agrees with authoritative `SendMessageTimeout(WM_NULL, SMTO_ABORTIFHUNG)` → `Unresponsive` (A21-4); exit-code inspection → `Exited`/`Crashed` (A21-3 NTSTATUS high-nibble rule)
- `is_protected_process` — exact `.exe` image-name match, case-insensitive, for session- and shell-critical processes (`csrss.exe`, `wininit.exe`, `winlogon.exe`, `services.exe`, `lsass.exe`, `smss.exe`, `lsaiso.exe`, `dwm.exe`, `explorer.exe`)
- `press_key_for_app` composing 2.8's `synthesize_key` under this sub-phase's window-activation policy: re-verify process identity, verify (never re-activate) foreground/owned when policy permits focus steal — headed activation already ran in core's `headed_focus` — confirm keyboard focus, synthesize, return `delivered_unverified`. Windows is synthesis-only (no `AXMenuBar` accelerator path) and headless delivery to a non-foreground target fails closed; both divergences are settled in §2.15
- **Window activation and focus policy in full, over the minimal enabling surface 2.6 shipped.** 2.6 landed `resolve_window_strict` plus a `focus_window` that corroborates ownership on both sides of every native write and qualifies success by live ownership, because the headed path had to reach the occlusion gate. **Element-level `Action::SetFocus` is already settled by §2.7** — headed-only with focused-element verification, `POLICY_DENIED` headless, on A3-4 / A19-5 — and this sub-phase must not re-open that gate. What remains here is the **window-activation** half: restore-versus-raise ordering when the target is minimized behind other windows, a bounded focus-steal budget of 2, confirmed by ownership re-read rather than trusted from the API's return value (A21-6: neither the first nor the second uncontended attempt landed foreground ownership in this capture, 0/5 each; contended re-measure owned by §2.12), cross-virtual-desktop behaviour as attempt-and-verify-ownership (no `IVirtualDesktopManager` binding, A16-9), and the UIPI boundary — integrity comparison up front; a strictly-higher target that silently no-ops fails closed via the ownership-qualified foreground re-read as activation-worded `PERM_DENIED`/`not_delivered`. Cross-integrity activation *effect* stays unmeasurable on probe hosts (A21-7, same privilege gate as A18-4/A19-4/A20-2); §2.12's split-integrity runner owns the live confirmation. The residual TOCTOU on a recycled HWND is irreducible at the Win32 API level; 2.6 made losing that race fail closed, and this sub-phase confirms nothing stronger is warranted

**Key APIs:** `CreateProcessW`, `TerminateProcess`, `WaitForSingleObject`, `GetExitCodeProcess`, `SetWindowPos`, `ShowWindow`, `GetWindowPlacement`, `GetWindowRect`, `IsHungAppWindow`, `SendMessageTimeout`, `SetForegroundWindow`, `AttachThreadInput`

**Depends on:** 2.4 (window/app identity, `list_windows`/`list_apps`), 2.6 (`resolve_window_strict` / `focus_window` ownership skeleton), 2.8 (`synthesize_key`, integrity primitive)

**Exit criteria:** lifecycle e2e (launch → interact → close) passes on a repo-controlled target; `APP_UNRESPONSIVE` is reachable against a deliberately hung fixture window (`StalledFixture`); dogfood report judged (`docs/dogfood-reports/2026-08-08-001-feat-windows-2-9-system-lifecycle-dogfood.md`).

**Est. PR size:** ~1.8k LOC

### 2.10 — Capture & Clipboard

**Goal:** Ship screenshot and typed clipboard, with the modern-capture-first / legacy-fallback split P2-O13 specifies.

**Scope:**
- **Runtime precedence Modern → Legacy** (P2-O13): prefer direct `Windows.Graphics.Capture` (`GraphicsCaptureItem` + `Direct3D11CaptureFramePool` through the `windows` crate) when `GraphicsCaptureSession::IsSupported` is true and interop activates; degrade silently to `ScreenshotBackend::Legacy` (`PrintWindow` with `PW_RENDERFULLCONTENT`, then bare; display targets via `BitBlt`) when modern is unavailable or fails to activate. Do not gate on build number — A22-1 measured `IsSupported: true` at build 17763, and dogfood J0 showed interop activate can still fail with Legacy succeeding. **Implementation order was Legacy → Modern** because A10-5 / the 17763 host could not exercise a working modern path (A22-1); that build order is not runtime precedence. Modern capture is **not** the `windows-capture` crate — the mandated diff-audit rejected it as a video-recording library whose feature set includes `Win32_UI_Shell` (KTD2; crates.io 2.0.1 supersedes the once-recorded 2.0.0 pin)
- `screenshot --screen` honest display targeting (pairs with `list_displays` from 2.4)
- Typed clipboard: `CF_UNICODETEXT` → `ClipboardContent::Text`, image via `CF_DIB`/`CF_DIBV5`/registered PNG → `ClipboardContent::Image`, `CF_HDROP` file lists → `ClipboardContent::FileUrls`. Default clipboard-image destinations (and default screenshot destinations with no user path) write through 0600-equivalent private files (2.1's private-file hardening); a user-named `clipboard-get --out` path or `screenshot` positional PATH routes through `write_user_atomic`, which bypasses the `PrivateFileOps` seam so network shares, reparse-traversing paths, and foreign-owned directories remain writable
- `probe_capture_availability` is truthful so `PermissionReport.screen_recording` reports `not_required` where capture works — Windows has no screen-recording consent gate — and `unknown` only where the session cannot support capture (dogfood: `screen_recording.state` = `not_required`)
- Clipboard hermeticity uses save/restore + serialization lock — window-station isolation was unavailable (`CreateWindowStationW` privilege 1314, A22-5). Delay-rendered `GetClipboardData` is unbounded against a non-pumping owner, so the clipboard read path uses worker-thread abandonment (A22-3); `PrintWindow` is bounded and keeps the shipped `window_is_responsive` pre-probe alone

**Key APIs:** `PrintWindow`, `BitBlt`, direct `Windows.Graphics.Capture` / D3D11 frame pool, WIC (`Win32_Graphics_Imaging`), `OpenClipboard`/`GetClipboardData`/`SetClipboardData`

**Depends on:** 2.1 (private-file hardening), 2.4 (displays)

**Exit criteria:** fixture-driven live verification of all four `ScreenshotTarget` variants against a known painted pattern; hermetic clipboard text/image/file-url round-trips under the save/restore + lock envelope (A22-5) with no lasting machine-clipboard pollution; forced-unavailable modern capture degrades silently to Legacy (seam + dogfood J0 / A22-1); `PermissionReport.screen_recording` is honest; judged dogfood report committed (`docs/dogfood-reports/2026-08-09-001-feat-windows-2-10-capture-clipboard-dogfood.md`, release binary 2,275,840 bytes from a 2,180,608 baseline). The Windows live e2e harness is §2.12's; genuine RDP/locked/Session-0 session-degradation, per-display capture on a second monitor (A22-8: not manufacturable here), and cross-integrity capture's live Medium→High effect (A22-7: manufacture partial, live effect still open) land in §2.12's scope and exit criteria. Modern pixel success is not an unconditional §2.12 exit criterion — the capability-probe lane on `windows-latest` still owes the hosted-runner WGC reading (A22-1); when that lane proves modern pixels, the proof belongs to 2.10's evidence rather than duplicating an exit gate on §2.12; when that lane finds WGC unsupported or interop-incapable, live modern success attaches to §2.12's interactive-runner item.

**Est. PR size:** ~2.4k LOC (owner settled one PR; exceeds the ~2k guidance knowingly)

### 2.11 — Signals & Wait Parity

**Goal:** Port `SignalBaseline`/`diff_signals`/`wait --event` (U17) to Windows so the existing `wait` command works identically cross-platform — this is explicitly NOT the future push-based `watch` command (P2-O11).

**Scope:**
- Windows `SignalBaseline` producers: windows / apps / focus / surfaces
- `wait --event` parity including `surface-appeared`
- Wait utilities operating within `Deadline` budgets, matching the core-owned deadline propagation
- **`wait --menu` / `wait --menu-closed` parity**, which needs a Windows menu-open detection primitive. `SystemOps::wait_for_menu` (`crates/core/src/adapter/system.rs`) is defaulted `not_supported` on Windows, and no menu-open detector exists in `crates/windows/src` (macOS backs it with `tree::surfaces::is_menu_open`). `wait --menu`/`wait --menu-closed` are shipped commands on the 58-command surface, so making the existing `wait` command "work identically cross-platform" (this sub-phase's goal) owns the Windows menu-surface detector and the `wait_for_menu` override. Discovered by §2.9's inventory: §2.9 is System Lifecycle and does not touch menu surfaces, so the parity hole lands here rather than staying unassigned
- **The menu detector is two complementary measured sources, not one.** Area 23 measured that classic `GetGUIThreadInfo` menu-mode flags, read per thread of the target pid, cover Win32 menu-bar, context and system menus and are silent at idle, but never fire for WPF in any state (A23-1, A23-2); and that a root-level UIA child of the pid carrying `WS_EX_TOOLWINDOW` with a Menu-family element reachable at or under it covers WPF dropdown and context menus, catches Win32 context menus, and is silent at idle on both stacks (A23-11). Bare "a Menu-family element is reachable" is **constant-true at idle on both stacks** and is not a usable predicate — a detector built on it reports a menu open for any application that merely has a menu bar. `ExpandCollapseState == Expanded` and `IsOffscreen` plus a non-empty rect were both measured and rejected (A23-12)
- **Deadline and race hardening for the shipped inventories.** `list_windows` and `list_apps` accepted a `Deadline` and ignored it while doing unbounded per-window process-handle work, and `list_windows_live`'s mid-walk identity refusal aborted `wait --window`, whose retryable set does not include `WINDOW_NOT_FOUND`. Both are fixed here because `wait --window` is one of the "existing `wait` command" surfaces this sub-phase's goal names. The bounded re-walk is consolidated onto one shared helper across its three call sites
- **Not owned here:** same-process HWND recycling inside one poll interval stays with §2.12.1, which owns the `RefEntry` identity field that closes it

**Naming note:** `wait --event <kind>` is the already-shipped baseline-diff desktop-signal wait (U17) — an in-invocation snapshot-diff, not a subscription. The future push element-event subscription primitive (P2-O11) is a **different, not-yet-built command** named `watch` (e.g. `watch --event value-changed --ref @s8f3k2p9:e5`), landing later in this sub-phase sequence once `watch_element` exists on the adapter trait. Do not conflate the two.

**Key APIs:** UIA property snapshots for baseline capture (no event handlers yet — that's `watch`, still future)

**Depends on:** 2.4 (windows/apps/displays), 2.9 (process lifecycle for app-launched/terminated signals)

**Exit criteria:** the AE6 analog passes as a fixture-driven in-crate test — a modal dialog opened mid-wait is discovered by `wait --event surface-appeared --app <name>` **and** by `wait --event window-opened`, with the caller naming neither its title nor its id. It is deliberately not phrased as an e2e: the Windows live e2e harness is §2.12's deliverable and §2.12 depends on this sub-phase, so an exit criterion written against that harness could never be discharged here. §2.12 re-runs the analog through the harness once it exists. Beyond it: all seven `--event` tokens fire on a fixture-caused transition and produce nothing on a capture pair with no transition; `wait --menu` and `wait --menu-closed` both directions against a real menu; a capture failure mid-wait degrades to `last_error` rather than aborting the wait; and a judged dogfood report is committed (`docs/dogfood-reports/2026-08-15-001-feat-windows-2-11-signals-wait-parity-dogfood.md`).

**Est. PR size:** ~5.7k LOC of Rust plus ~4.7k of probe corpus and docs. The original `~1k` estimate counted only the two adapter methods and missed what makes them correct: a single-pass inventory with its own race and completeness semantics, a two-source menu predicate, an app-scoped surface producer, the deadline and race hardening of two shipped inventories, three new test fixtures, and the live breadth that proves each token in both directions.

### 2.12 — Fixture App & Live E2E Harness

**Goal:** Give Windows the same verify-by-observation live e2e discipline the macOS SwiftUI fixture already provides.

**Scope:**
- WinForms fixture app compiled with `csc.exe` (.NET 4.8 preinstalled on the runner — no new toolchain dependency)
- `AutomationId` set on every interactive target from day one (unlike macOS, which had to retrofit `AXIdentifier` — Windows gets this right from the start)
- Fixture targets mirroring `AgentDeskFixture.swift`: delayed-enable, zero-bounds, duplicate-title, occlusion, disclosure
- Harness port (bash via Git-Bash, or a PowerShell driver) asserting every effect by independent re-observation, never the command's own `ok:true` — same contract as `tests/e2e/run.sh`
- **Off-screen fixture parking is display-size-dependent and must be re-decided against this runner's actual desktop.** The Windows lib tests park non-visual fixture windows at a fixed `(2000, 2000)` (`fixture_window::OFFSCREEN_LEFT`/`TOP`, and `(2100, 2100)` in `resolve_pair_window`), which is off-screen only on a display smaller than that in both axes — true of the dev box at 1639×732, not guaranteed of any runner. On a virtual screen wider and taller than 2000 px those windows land *inside* the desktop while holding none of the serialization the on-screen legs use, and the on-screen slot grid would itself span that origin (column 4 covers x = 1824…2244). Sub-phase 2.6 serialized every leg that knowingly stages on-screen and left this one alone rather than changing a parking constant on an unmeasured assumption. This sub-phase owns the measurement — read the runner's virtual-screen rect, then either move the parking origin outside it or bring those fixtures under the same stage lock
- Self-hosted interactive Windows runner registration — this is the first sub-phase whose gate needs a real desktop, so the runner is registered here rather than standing idle through the sub-phases that do not use it. A service-mode runner has no interactive desktop and cannot see UIA at all, so the runner launches from a Task Scheduler task triggered at log-on that runs `run.cmd` inside the interactive session
- Public-repo hardening on that registration. This repository is public, and GitHub's own guidance is that self-hosted runners should almost never be used for public repositories, because any user can open a pull request and compromise the environment: the runner's workflow is `workflow_dispatch`-triggered only and never `pull_request`, the fork-PR approval policy is written down, and ephemeral/JIT versus persistent registration is an explicit recorded decision rather than a default
- Registration is a measurement obligation as much as infrastructure: it creates the first non-console session this project can observe, and 2.12 closes 2.0's deferred RDP/session-isolation row by measuring it there rather than by documenting it (an interactive session is required for UIA to see a real desktop; `tscon` is the documented console-reattach workaround and leaves the machine unlocked — see Risk Register). Until that measurement lands, no Windows adapter behavior assumes console-session semantics and this document claims no RDP or remote-session support
- `app/provider` — A14-1 measured the hosted runner rather than inferring it, finding `windows-latest` resolved to Server 2025 build 26100 on image `win25-vs2026` 20260714.173.1, with `qwinsta` reporting `>console` Active at session id 2, the probe process running in that session, `[Environment]::UserInteractive` true, window station `WinSta0` and desktop `Default` — one image on one date rather than a product contract, and contrary to Microsoft's own guidance for its hosted agents ([Configure for UI testing](https://learn.microsoft.com/en-us/azure/devops/pipelines/test/ui-testing-considerations)), which is why the case for this sub-phase's runner rests on presenting real applications on a representative shell and never on whether a session exists
- `windows-e2e` workflow_dispatch job on that runner
- **`focused_window`'s frame-vs-`CoreWindow` identity for `ApplicationFrameHost`-hosted targets.** A16-2 could not settle which HWND shape a UWP host presents to `GetForegroundWindow` and `list_windows`, because no environment measured so far carries a modern-shell population (A10-7) — A1-3's split between the `ApplicationFrameWindow` frame and its hosted `Windows.UI.Core.CoreWindow` was observed once, on Settings, and never against the foreground path. The fixture population and self-hosted runner this sub-phase creates are what let that be measured against a real target rather than left as an unverified mapping
- **Split-integrity verification for observation reads, input writes, window activation/focus, *and* capture.** A16-12 measured a Medium-integrity client reading a High-integrity WPF window's root and process-generation token with neither read denied — but the probe's High and Medium processes share one user on the dev box, so KTD3's fail-closed elevated-process branch (`process_instance: None` on a failed token read) never fired there. A rig with a genuinely split-integrity boundary — a Medium-integrity session observing a window an elevated, Administrator-token process owns — is what this sub-phase's runner registration is positioned to provide, and the verification lands here. **The write half lands here too:** A9-2 measured that across a UIPI boundary reads cross while `SendInput` silently does not land, and A9-3 measured `SendInput` reporting success in both arms, so only an independent re-read separates them — but neither §2.7 nor §2.8 could stage the boundary to observe the effect, because `Start-MediumIntegrityProcess` fails with "A required privilege is not held by the client" on both probe hosts (A19-4, and A18-4 before it). §2.8 therefore ships and unit-tests UIPI *detection* (a local `GetTokenInformation(TokenIntegrityLevel)` read plus the integrity comparison) and maps the denial onto `PERM_DENIED` from A9-2's already-measured contract; what stays open is the live confirmation that a Medium→High pattern write or `SendInput` really does not land and really does surface as that envelope. This sub-phase's runner is the first rig able to hold both integrity levels, so it owns that confirmation — not a re-implementation of the detection, which is already shipped and pinned. **The activation half lands here too:** §2.9's window-activation policy reaches `SetForegroundWindow`/`AttachThreadInput` across the same boundary, and A9-2's "reads cross, writes do not" makes a Medium→High activation a plausible silent no-op that §2.9 ships fail-closed (a strictly-higher target proceeds attempt-and-verify and the ownership-qualified foreground re-read catches the no-op as `not_delivered`). §2.9 unit-tests the integrity comparison against synthetic SIDs but cannot stage the live effect on any probe host to date (the same `Start-MediumIntegrityProcess` gate), so this runner owns confirming that a cross-integrity focus write really does not land and really does surface as the fail-closed envelope — alongside the input-write confirmation, not as a separate rig. **The capture half lands here too:** A22-7 manufactured a Medium-integrity caller on the 2.10 host but could not run `PrintWindow`/WGC inside that process to observe the Medium→High capture effect (`printwindow.cross_direction.branch: medium_printwindow_launch_failed`; WGC `supported_cross_direction_not_instrumented`), so live cross-integrity capture effect — Legacy and modern where supported — is owned here rather than re-implemented in 2.10
- **Multi-monitor `list_displays` verification and per-display capture.** A10-3 and A16-4 both measured `list_displays` and per-monitor `scale` against the single 96-DPI display present in every environment measured to date, so the aware-versus-unaware bounds delta those rows report as zero is a fact about that display, not yet about the DPI-scaling code path. A22-8 measured that a software/virtual second display cannot be manufactured on the 2.10 host (`manufacturable: false`, `branch: no_virtual_display_adapter_available`), so per-display `screenshot --screen <index>` correctness joins the multi-monitor item rather than closing inside 2.10. A rig with more than one monitor, ideally at mismatched DPI, closes both legs, and none has existed before this sub-phase's runner
- **Capture session-degradation on the interactive runner.** 2.10 proves silent Modern→Legacy fallback at the forced-unavailable seam and on the 17763 dogfood host where interop fails despite `IsSupported` (A22-1; dogfood J0). Genuine RDP / locked-desktop / Session-0 degradation — a session someone can disconnect or lock — needs the interactive runner this sub-phase registers and cannot be closed by any measurement available to 2.10. Live modern-capture pixel success is **not** an unconditional obligation of this item: the capability-probe lane on `windows-latest` still owes the hosted-runner WGC reading (A22-1); when that lane proves modern pixels, the proof stays with 2.10's evidence; when that lane finds WGC unsupported or interop-incapable, live modern success attaches here beside session-degradation
- **The graded fallback's aggregate `STALE_REF` rate inside web content.** A17-8 measured this on the 2.5 dev box's first-contact Chromium shell only — Obsidian's file tree never reaches a fresh client within the settle window there, so the fixture-driven 0/1/N cases are 2.5's committed proof of the tier's semantics but the real-world aggregate rate stayed unmeasurable (2026-08-03-001 dogfood report, `closure: 2.12`). **Control the target's window state before attributing anything to the environment.** A restored-but-not-activated Chromium/Electron target on the 2.5 dev box exposes its content tree to the product's own COM path immediately — `snapshot` returns a complete tree with refs allocated — while the same process minimized returns `TIMEOUT` and presents only an eighteen-node shell to a raw walk. Holding one client open against the minimized window for ninety seconds adds nothing, and neither does the `WM_GETOBJECT`/`OBJID_CLIENT` handshake the window answers: the tree is wholly present at the first read once the window is restored, and wholly absent while it is not. Settle time is therefore not the lever this row assumed, and the dev box is not disqualified by the shell it was measured against. This sub-phase measures the rate against real web content rather than a first-contact shell number
- **Chromium / Electron semantic-action proof on a settled, self-hosted desktop.** §2.7's dogfood drove one Obsidian semantic click and recorded honest `TIMEOUT` / `not_delivered` (J9); the same host's shell-bound Chromium shape is A18-3. Neither invents semantic success. This sub-phase's interactive runner and controlled Chromium target own proving a positive-area leaf through the semantic dispatch tier without coercing the environment-bound failure into a pass
- **Explorer below-fold / Items View virtualization surface for the ancestor-scroll ladder.** §2.7 shipped the `ScrollPattern` ancestor ladder (A19-7) and re-judged the 2.6 Explorer residual as dispatch-honest Invoke with `delivered_unverified` (2.7 dogfood J3). On that host the realized option set had no off-screen row among virtualized Items View children (24 of 40 synthetic files visible), so the ladder was not forced through a below-fold geometry change. This sub-phase owns a denser or unrealized Explorer / fixture surface that forces the ladder — not a re-implementation of the ladder itself
- **The HWND uniqueness-counter wrap rate under real window churn.** No probe has established it — `probes/windows/FINDINGS.md` carries no row — and sub-phase 2.5 identified the gap while closing the cross-process HWND-recycle case and leaving the same-process one open. Measuring it means staging churn on a desktop this project owns for as long as the handle table's uniqueness counter takes to recycle: windows created and destroyed in bulk by one still-running process, with each handle's identity observed across the wrap. That is a fixture-driven workload on an interactive session, and this sub-phase's fixture app and self-hosted runner are the first rig that can host one. What it feeds: §2.12.1 fixes the same-process recycle gap and decides the fix's shape on this rate — a wrap rate unreachable under realistic churn leaves the fix purely structural (a per-window immutable identity field on `RefEntry`), a reachable one adds a wrap-handling rule beside it
- **Menu-open detection for the stacks area 23 could not stage.** §2.11's detector is two measured sources whose union covers Win32 and WPF, and both were verified round-tripping false-to-true-to-false. Two stacks stayed unmeasured and are recorded `measurable: false` rather than assumed: **Chromium/Electron** (A23-3 — no menu surface was reachable by generic staging against the installed build across three real-input attempts, so neither source could be evaluated), and **WinUI/UWP** (no modern-shell host exists in any environment measured to date, the same A10-7 population gap §2.4.1 exists for). This sub-phase's fixture app and controlled Chromium target are the first rig able to stage both, so it owns evaluating the two shipped sources against them — not re-implementing the detector, which ships and is unit-tested against what could be measured
- **The mid-walk identity race rate under a standard-user integrity level.** A23-5 measured 0 hits in 120 iterations of the agent-facing window enumeration on this host, idle and under churn, at High-integrity Administrator with up to 286 windows per iteration. That is not a contradiction of the race being reachable — §2.11 observed it firing directly in the launch path and ships a bounded re-walk for it — but it is a rate this host could not reproduce, so the budget of 5 attempts rests on the sighting rather than on a measured distribution. A standard-user runner with tighter timing is where that distribution can actually be taken
- **Whether a window this process can never identify is a real population.** §2.11 accepts that a window whose owning-process identity is unreadable is excluded from the signal inventory and therefore permanently invisible to the diff — a `window-opened` for it will not fire. That is the honest limit of what a Medium-integrity observer can see, and A23-9 measured 0 such windows on this desktop, which is a floor taken at High integrity rather than a general answer. The split-integrity rig this sub-phase registers is the first environment that can bound it properly
- **The `cell` ref-able role arm.** A16-10 and the 2.4 dogfood report (`docs/dogfood-reports/2026-08-01-feat-windows-2-4-observation-dogfood.md`) measured that neither fixture available to 2.4 produces the `DataItem` + `GridItem`/`TableItem` shape the `cell` refinement needs: WPF's `DataGrid` exposes the patterns on `ControlType.Custom` cells instead, and WinForms' `DataGridView` exposes neither pattern on its rows. This sub-phase's fixture app is where a grid target that resolves the question belongs — either reproducing the `Custom`+pattern shape against a fixture the harness controls, or observed against a real grid application — settling what the dogfood report left open: whether `Custom` + `GridItem`/`TableItem` should also refine to `cell`

**Key APIs:** `csc.exe`, WinForms `AutomationProperties.AutomationId`

**Depends on:** 2.7, 2.8, 2.9, 2.10, 2.11 (everything the harness exercises)

**Exit criteria:** the full Windows live gate is green in both headless and headed tiers on the self-hosted interactive runner; the runner's `workflow_dispatch`-only triggering, fork-PR approval policy and ephemeral-versus-persistent decision are written down alongside it; 2.0's deferred RDP/session-isolation row is closed by measurement on that runner, **including capture session-degradation** (RDP / locked / Session 0 silently falls back to Legacy and succeeds); multi-monitor `list_displays` verification and per-display capture are proven on a rig that presents more than one monitor (A22-8); split-integrity verification covers observation reads, input writes, window activation/focus, **and** the live Medium→High capture effect (A22-7); and the HWND uniqueness-counter wrap rate is measured under staged churn and recorded as a `probes/windows/FINDINGS.md` row §2.12.1 can decide on. Live modern-capture pixel success is an exit criterion here only when the `windows-latest` capability-probe lane has not already proven modern pixels for 2.10 (A22-1).

**Est. PR size:** ~2.5k LOC (mostly C#/scripts plus runner registration and its hardening policy, not adapter Rust)

### 2.12.1 — Window Identity in Stored Refs

**Goal:** Close the same-process HWND-recycle resolution gap with a per-window immutable identity on `RefEntry`, in a PR that reviews as the core schema change it is.

**Scope:**
- **The gap.** An HWND destroyed and reused by another window of the same still-running process resolves against the recycled window: `resolve_window_root`'s stored-evidence check (pid + handle-ownership + process-generation token, `WindowIdentityEvidence::verify_stored`) verifies the window, not which element inside it is correct, and element-level exact-evidence resolution does not catch the substitution either — two instances of one dialog present identical `AutomationId`, `ControlType`, and `Name` to `candidate_outcome` (`crates/windows/src/tree/resolve_match.rs`), so the candidate matches and `classify_search`'s (`crates/windows/src/tree/resolve.rs`) sole-candidate arm resolves with no geometric corroboration. Bounds corroboration cannot substitute: `bounds_hash` is exact over absolute screen coordinates, so demanding it on every resolve would fail any ref whose window or layout moved between snapshot and action — the common case, not the exceptional one. Sub-phase 2.5 identified the gap and could not close it, being scoped shut against `RefEntry` schema changes as 2.4 was before it
- **The field.** A per-window immutable identity on `RefEntry` — the window's UIA `RuntimeId` or a creation ordinal — threaded through the Windows walk and through stored-evidence window resolution, and consulted before a stored ref's window root is trusted
- **Optional by construction.** `RefEntry` is serialized into `refmap.json` under `~/.agent-desktop/snapshots/`, so the field is omitted when absent per this document's serialization rules and a refmap written without it still deserializes; a ref carrying no window identity resolves exactly as it does today. The addition can refute a substitution and must never invent a new stale-ref failure
- **A wrap-handling rule if the measurement calls for one.** The field itself is unconditional — the fix is structural in either branch of 2.12's measurement. What the measured wrap rate decides is what ships beside it: a rate unreachable under realistic churn leaves the field standing alone, a reachable one adds the rule that keeps the stored identity honest across a counter wrap
- **The regression test.** Pin the same-process-recycle case with a test that fails when the new corroboration is removed — a stale ref whose window was destroyed and whose HWND was reused by another window of the same process resolves `STALE_REF`, never the recycled window
- macOS is unaffected by the underlying hazard — `CGWindowID` is a per-session monotonic counter, not a recycled handle-table slot — so this is a Windows-only schema addition, not a cross-platform contract change

**Key APIs:** `IUIAutomationElement::GetRuntimeId`, `GetWindowThreadProcessId`

**Depends on:** 2.12 (its measured wrap-rate row decides whether the wrap-handling rule ships), 2.5 (the resolver and stored-evidence window check this corroborates)

**Exit criteria:** a fixture window destroyed and its HWND reused by another window of the same process resolves the stale ref to `STALE_REF` rather than to the recycled window; the regression test fails when the corroboration is removed; refmaps written without the field still deserialize and resolve unchanged; `cargo tree -p agent-desktop-core` still names no platform crate.

**Est. PR size:** ~600 LOC — the core schema field and its serialization, the Windows walk and window-resolution threading, and the recycle fixture with its regression test

### 2.13 — FFI, npm, Release

**Goal:** Make the Windows adapter reachable through every distribution channel that already ships for macOS.

**Scope:**
- FFI real-adapter path validated on Windows (non-stub tests — the stub-adapter tests already run cross-platform in CI, but the real `WindowsAdapter` behind the C ABI needs its own pass)
- npm `postinstall.js` gains `win32-x64` and `win32-arm64` branches — hosted `windows-11-arm` runners have been GA for public repos since 2025-08-07, so ARM64 validation is no longer deferred
- Release matrix: `.exe` zip + attestation, using the same tarball + sha256 + Sigstore pipeline Phase 1.5 already ships
- `skills/agent-desktop-windows/SKILL.md` — see Skill Update below
- README platform table: Windows column → **Yes**

**Key APIs:** none new — this sub-phase is packaging, not adapter code

**Depends on:** 2.2 through 2.11 (needs a working adapter to package)

**Exit criteria:** `npm install -g` works on Windows; release dry-run artifacts verified.

**Est. PR size:** ~1.2k LOC

### 2.14 — Shell Surfaces & Notifications

**Goal:** Cover the Windows-only shell surface (P2-O18) and notification/tray (P2-O14 Windows half) scope that has no macOS analogue to backfill against.

**Scope:** P2-O18 shell coverage, notification management, and system tray — all three folded in below rather than duplicated. This sub-phase ships **inside Phase 2, before the 2.15 integration merge**, under the no-convenience-deferral rule in the [Platform Delivery Model](#platform-delivery-model--sub-phases-and-integration-branches).

- **Launching an installed app by display name or AUMID, which §2.9's `launch_app` cannot do.** §2.9 launches via `CreateProcessW` with system-dirs-only bare-name resolution (`System32` / Windows directory — never caller directory, current directory, or `PATH`; A21-1), so it never reads the `App Paths` registry key or Start Menu entries; macOS resolves any installed app through `NSWorkspace` by bundle id or display name. Reaching parity needs `ShellExecuteExW` (a `Win32_UI_Shell` manifest feature) or `IApplicationActivationManager` for packaged/UWP apps (an interface the pinned `windows`/`windows-sys` crates do not generate, the same class of gap as `IVirtualDesktopManager` at line 1146). **A21-8** validated `ShellExecuteExW` binding and launch in a standalone scratch that requires `Win32_UI_Shell` **and** `Win32_System_Registry`, recorded `expand_2_9: false`, and named ownership `section_2_14_owns_by_name_aumid` — the positive binding does not pull the capability back into §2.9. §2.9 deliberately did not take that dependency — the manifest surface is a supply-chain-reviewed decision rather than a probe outcome, and `CreateProcessW` is load-bearing for its launch verification design. What lands here: the by-name/AUMID launch path behind the existing `launch_app` contract, or an explicit recorded decision not to take it. §2.15 separately settles whether the cross-platform `launch` contract normalizes or ratifies the divergence

- **The signal path emits surface kinds `snapshot --surface` does not advertise, and closing that gap is this sub-phase's.** §2.11 emits `SurfaceSignal { kind: Menu }` and `{ kind: Sheet }` from `capture_signal_baseline`, because core never validates a signal's surface kind against `supported_surfaces()` — its only consumers are `status`'s report and `surface_scope::require_supported`, which `snapshot` and `find` call to validate a *requested* surface. Windows therefore advertises `[Window, Focused, Sheet]` while `wait --event surface-appeared` can legitimately report a `menu` surface. §2.11 deliberately did not add `Menu` to the advertised list: doing so would make both `snapshot --surface menu` and `find --surface menu` claim a capability `crates/windows/src/tree/surfaces.rs` refuses at its catch-all, turning an honest asymmetry into a lie in `status`. Extending the advertised surface set is this sub-phase's work, and it is what removes the asymmetry.

**Key APIs:** see the three subsections immediately below.

**Depends on:** 2.4 (observation), 2.7 (semantic actions), 2.9 (`launch_app`, whose by-name/AUMID gap this sub-phase inherits)

**Exit criteria:** `open-system-surface --surface <kind>` + `snapshot --surface <kind>` round-trips for Start menu, taskbar, Quick Settings, and Action Center where the current shell exposes them, with explicit `PLATFORM_NOT_SUPPORTED` assertions (clear `platform_detail`) where it does not; notification list/dismiss/action work through at least one of the two documented paths; tray list/click work through SNI-equivalent UIA traversal.

**Est. PR size:** ~2k LOC

#### Windows-specific command surface (P2-O18)

Windows-specific commands are allowed when the operating-system concept has no portable equivalent, but they still follow the repository rules: one core command file, typed CLI/batch dispatch, adapter trait default returning `PLATFORM_NOT_SUPPORTED`, skill docs, and tests. The preferred path remains generic: expose shell UI as a surface, then let agents interact with refs.

| Command | Purpose | Platform behavior |
|---------|---------|-------------------|
| `open-system-surface --surface <kind>` | Opens an OS shell surface so agents can immediately call `snapshot --surface <kind>` and act by refs | Windows kinds: `start-menu`, `taskbar`, `system-tray`, `system-tray-overflow`, `action-center`, `quick-settings`. macOS may support `spotlight`, `dock`, `menu-bar-extras`, `notification-center`. Unsupported kinds return `PLATFORM_NOT_SUPPORTED` |
| `list-tray-items` / `click-tray-item` / `open-tray-menu` | Structured tray workflows where the shell surface is not attached to a normal app window | Windows implementation uses `Shell_TrayWnd` plus the overflow flyout (`TopLevelWindowForOverflowXamlIsland` on Win11 22H2+; `NotifyIconOverflowWindow` before that); macOS maps to menu bar extras. Linux maps to StatusNotifier in Phase 3 |

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

- **List items:** UIA tree of the `Shell_TrayWnd` window class; tray items are children of the notification area. Overflow items live in `TopLevelWindowForOverflowXamlIsland` on Win11 22H2+ (build 22623+); `NotifyIconOverflowWindow` exists only on earlier builds.
- **Click:** `InvokePattern` on tray items, falling back to coordinate-based `SendInput` for items that don't expose UIA patterns.
- **Open menu:** after clicking a tray item, detect the resulting popup menu via UIA focus-changed events and expose it for ref-based interaction.

### 2.15 — Hardening & Integration Review

**Goal:** Prove the assembled `feat/windows-adapter` branch is production-grade as a whole, then merge it.

**Scope:**
- Full-branch multi-agent review — the whole assembled branch, not only this sub-phase's own diff
- Live e2e in both headless and headed modes on the Windows runner
- Performance baseline vs `main` (Windows vehicle: probe corpus cost methodology per A15-13 / A18-7; `scripts/perf-baseline-compare.sh` is macOS-bound)
- LOC/size/isolation audits (`cargo tree -p agent-desktop-core` still zero platform crate names; binary still under 15MB)
- Docs/skills sync (this document, `skills/agent-desktop-windows/`, README)
- **Settle the `offscreen` cross-platform contract.** The token means different things on the two adapters and this is deliberate for Phase 2, not an oversight: macOS computes it geometrically from element-versus-window bounds (`crates/macos/src/tree/state_reader.rs:86-95`), while Windows emits UIA's provider-reported per-element value (sub-phase 2.3). They genuinely disagree — a virtualized row scrolled out of view is geometrically inside the window but offscreen to the provider — and A14-8 measured a UIA value contradicting itself *within one window*, where a minimized top level reported `IsOffscreen` true while every descendant reported false, which is why no adapter may propagate a container's value to its subtree. Decide here whether the contract specifies one meaning, and if so which, now that both adapters exist and there is evidence from both. Identical JSON across platforms is a product promise, so a permanent divergence is a decision to take explicitly rather than inherit. This gate owns the decision and the record of it, not its implementation: if the decision is to normalize, the normalization changes shipped behaviour on at least one adapter, and it lands as its own sub-phase PR into `feat/windows-adapter` — verified there against both adapters — before this gate merges. Ratifying the divergence needs no successor PR
- **Settle the `button`/`switch` role and `pressed`/`checked` state divergence for the same toggle control.** This is deliberate for Phase 2, not an oversight: macOS keeps the control's role as `button` and reads its toggle value as `pressed` (`crates/macos/src/tree/state_reader.rs:57-59`), while Windows reclassifies any `Button` control type that advertises `ToggleAvailable` to `Role::Switch` before states resolve (`crates/windows/src/tree/roles.rs`'s `button_role`), so the same control surfaces as `switch` with state `checked` instead and `pressed` stays permanently unproduced on Windows — `crates/windows/src/tree/states.rs`'s `resolve_states` doc comment explains why that precondition can never hold there. Decide here whether the contract normalizes the role, the state token, or both, or ratifies the divergence explicitly, using the same identical-JSON-is-a-product-promise standard the `offscreen` decision above uses — and on the same terms: a normalization is a shipped-behaviour change that lands as its own sub-phase PR before this gate merges, while a ratified divergence is recorded here and needs no successor
- **Promote the resolution error payload into `agent-desktop-core` and make both adapters consume it.** Windows's `identity_unknown_error` (`crates/windows/src/tree/resolve_search.rs`) and `mark_deadline_elapsed` (`crates/windows/src/tree/resolve.rs`) are verbatim copies of macOS's `identity_unknown` (`crates/macos/src/tree/resolve_errors.rs`) and `mark_deadline_elapsed` (`crates/macos/src/tree/resolve.rs`). What they duplicate is not a message string but a payload core reads back: `AdapterError::with_details` derives `Retryability` from the `retryable` key (`crates/core/src/retryability.rs`), and that derivation is the sole input to `is_explicitly_retryable` / `permits_retry_by_default`, which gate every core retry consumer — `crates/core/src/live_locator/hydrate.rs`, `commands/wait_element.rs`, `commands/wait_selector.rs`, and `AdapterError::is_retryable_resolution_failure`. Duplicated per adapter, the contract drifts silently and in one direction that has no alarm: a renamed `kind`, a `complete`/`retryable` value changed on one side, or a merge branch that drops `retryable` while stamping `deadline_elapsed` (`mark_deadline_elapsed`'s non-object fallback nests the prior details under `evidence`, so the top-level key is re-derived from what survives) turns a retryable incomplete into an unretried failure **on one OS only**, while both crates' tests stay green because each asserts against its own constructor. **What closes it:** one core-owned constructor pair for these two errors, called by macOS and Windows alike, with the payload's keys and their retryability consequence pinned by a core test that fails when a key is renamed or dropped; the resolver behaviour that legitimately differs — which errors each adapter's own retry loop classifies as retryable — stays adapter-side. Phase 3's Linux adapter then inherits the constructors instead of making a third copy. **Why here:** the promotion changes the macOS crate to consume the core version, and macOS is the GA line for the whole platform phase, so it is a cross-platform refactor that belongs at the one gate where both adapters are reviewed, e2e'd and perf-baselined together — not inside a Windows sub-phase, and not in 2.5, which was budgeted for exactly one core visibility promotion and spent it. **Evidence:** sub-phase 2.5's review finding #11 and the residual row in `docs/dogfood-reports/2026-08-03-001-feat-windows-2-5-resolution-live-locator-dogfood.md`
- **Settle whether dangerous-shortcut matching is superset-aware on both adapters.** Windows matches a blocked shortcut when the pressed key matches and the pressed modifiers are a *superset* of the entry's, because `alt+shift+tab` steals the foreground exactly as `alt+tab` does and an equality check waves it through (found by review on §2.8). macOS still compares canonical strings for equality and handles the same problem by enumerating variants — it lists `cmd+q` and `cmd+shift+q` as separate entries. Both guards are honest today, but they answer "is this combo dangerous" by different rules, and the enumeration approach cannot cover a variant nobody listed. Decide here whether macOS adopts superset matching, whether Windows narrows to enumeration, or whether the divergence is ratified — on the same identical-behaviour-is-a-product-promise standard the other entries use. **Evidence:** §2.8 `crates/windows/src/input/blocked_combo.rs`, `crates/macos/src/input/blocked_combo.rs`
- **Settle the `type` command's cross-platform contract, which has no semantic-headless path on Windows.** macOS headless `type` inserts at the selection by writing `AXSelectedText` (`crates/macos/src/input/type_text.rs`), a semantic write needing no focus steal, so strict-headless `type` succeeds there. Windows UIA exposes no insert-at-selection equivalent: `ValuePattern.SetValue` replaces the whole value (that is `set-value`, §2.7's headless text path) and `TextPattern` is read-only for insertion, so §2.8's `type` is physical synthesis under the focus-fallback/headed policy and strict-headless `type` fails at policy where macOS would have typed. Decide here — with both adapters in review — whether the contract normalizes (for example by defining `type` as semantic-where-available and documenting the per-platform policy floor), or ratifies the divergence explicitly, on the same identical-JSON-is-a-product-promise standard the `offscreen` and role/state decisions above use and the same terms: a normalization is a shipped-behaviour change landing as its own sub-phase PR before this gate merges, while a ratified divergence is recorded here and needs no successor. **Evidence:** §2.8 scope (`type` physical-only), A4-1 (`KEYEVENTF_UNICODE` synthesis measured working), §2.7's `set-value` headless path
- **Settle the two `press --app` divergences Windows cannot match.** Both stem from Windows `SendInput` injecting into the foreground queue with no per-pid targeting, where macOS delivers to a specific pid. **(a) No semantic accelerator path:** macOS's `press_key_for_app` (`crates/macos/src/system/key_dispatch.rs`) first walks `AXMenuBar` for a menu item whose `AXMenuItemCmdChar`/`AXMenuItemCmdModifiers` match the combo and presses it via `AXPress` — a semantic delivery needing no focus steal — while Windows exposes no queryable global menu-bar/accelerator surface, so §2.9's `press_key_for_app` is synthesis-only. **(b) No headless delivery to a background target:** macOS's headless arm (`allow_focus_steal` false) delivers via a pid-targeted event regardless of foreground, while Windows cannot inject to a non-foreground window without stealing focus, so a headless `press --app` whose target is not already frontmost fails closed on Windows where macOS succeeds. Decide here — with both adapters in review — whether the contract normalizes (for example by defining `press --app` as semantic-where-available with a documented per-platform focus floor) or ratifies the divergence explicitly, on the same identical-JSON-is-a-product-promise standard the `type` decision above uses and the same terms: a normalization lands as its own sub-phase PR before this gate merges, a ratified divergence is recorded here and needs no successor. **A third arm surfaced at §2.11's dogfood: the non-interactive *caller*, distinct from the non-foreground *target*.** `press escape --app <image>` issued from a background job failed to dismiss an open native menu in 4 of 4 reproductions while the identical command from the interactive foreground console dismissed it every time. The envelope is honest — `SendInput` cannot verify delivery, so `delivered_unverified` is the correct disposition rather than a false success — but the reach limit is undocumented, and an agent driving Windows from a service, a scheduled task or a CI job is exactly the caller that hits it. Settle it alongside the two arms above. **Evidence:** §2.9 scope (`press_key_for_app` synthesis-only, headless-non-foreground fails closed), macOS `key_dispatch.rs`'s `try_menu_bar_shortcut` and its pid-targeted `synthesize_key`, §2.8 KTD2 (`SendInput` foreground-queue, no per-pid targeting), §2.11 dogfood F3
- **Settle the Windows `launch` identifier contract (system-dirs-only bare names).** §2.9's `launch_app` accepts an absolute path or a bare name resolvable only under `System32` / the Windows directory (A21-1); it deliberately does not follow `CreateProcessW`'s full module search order and does not read `App Paths` / Start Menu. macOS resolves installed apps by display name or bundle id through `NSWorkspace`. §2.14 owns the by-name/AUMID path once a shell binding is taken (A21-8 validated the binding in standalone scratch with `expand_2_9: false`). Decide here whether the portable `launch` contract normalizes (for example by requiring §2.14's path before claiming display-name parity) or ratifies that Windows identifiers are path-or-system-image while macOS identifiers are display-name-or-bundle-id. **Evidence:** A21-1, A21-8, §2.9/`launch_path.rs`, §2.14 scope
- **Settle whether `wait --event`'s baseline advances, because today a disappearance is invisible for anything that appeared after the wait started.** `wait_for_event` seeds a baseline from the first successful capture and diffs every later poll against that same fixed capture, never advancing it (`crates/core/src/commands/wait_event.rs`). An entity that both appears and disappears inside one wait is therefore absent from the baseline and absent from the current capture, so `diff_signals` sees nothing. Measured on Windows at §2.11's dogfood: a window opened and closed inside one wait produced no `window-closed` in 4 of 4 discriminating trials while a pre-existing window produced one in 4 of 4, and an application launched 2s into a 14s wait and terminated 3s later produced `TIMEOUT` rather than `app_terminated`. The gap generalises to every disappearance event — `window-closed`, `app-terminated`, `surface-dismissed`. **This is core, not adapter:** the fixed baseline is platform-neutral and macOS behaves identically, so it is not a Windows defect and could not be fixed from a Windows sub-phase. Decide here whether the contract keeps the fixed baseline (a wait answers "what changed since I started"), advances it per poll (catching transient lifecycles at the cost of redefining every event), or documents the limit explicitly — on the same identical-JSON-is-a-product-promise standard the entries above use, and the same terms: a normalization is a shipped-behaviour change landing as its own sub-phase PR before this gate merges. **Evidence:** §2.11 dogfood F1, `docs/dogfood-reports/2026-08-15-001-feat-windows-2-11-signals-wait-parity-dogfood.md`
- **Settle the `--app` resolution error envelopes, which name a recovery the command cannot perform.** Two findings from §2.11's dogfood, both raised in `crates/core/src/app_lookup.rs` and therefore shared with macOS. `APP_NOT_FOUND` carries no `suggestion` at all, so a caller who passes a display name where Windows wants an image name gets no hint toward the accepted form — the failure mode the `--app` stem-matching entry below already anticipates. `AMBIGUOUS_TARGET`'s `suggestion` points at refs and snapshots, concepts `wait` does not have, and while its `details.candidates` carries a disambiguating `pid` and `process_instance` per candidate, no `wait` flag accepts either — the error is precise and the recovery path is absent. Decide here whether the envelopes gain accurate guidance, whether `wait` gains a pid selector, or whether the divergence is ratified. **Evidence:** §2.11 dogfood F4, F5
- **Bring the `docs/phases.md` hunk index current, or retire the check.** `probes/windows/13-ledger-check.ps1` requires a bijection between this document's hunks and backing ledger rows, measured as `git diff -U0 main -- docs/phases.md`. Under the platform delivery model `main` is an entire platform phase behind the integration branch, so the measured count grows with every merged sub-phase while the index only gains the rows whichever sub-phase was paying attention wrote — 62 indexed against 104 measured at §2.11, a shortfall of 42 accumulated across sub-phases that merged without updating it. §2.11 reported the shortfall rather than enforcing it, because enforcing it would make one sub-phase answer for every earlier sub-phase's doc edits; the half of the invariant that carries its value still fails the build (every indexed hunk names a ledger row that exists, and every `CONTRADICTS` row is backed by a hunk). This gate reviews the assembled branch and owns docs sync, so it is where the index is either brought current or the completeness half is retired with its reason recorded
- **Settle whether Windows detects an application's renderer, so `launch`'s Chromium guidance is cross-platform.** v0.8.1's `launch --cdp` is platform-neutral and verified working on Windows — core pushes `--remote-debugging-port` / `--remote-debugging-address=127.0.0.1` into the launch args and probes the loopback endpoint itself, so a Windows Electron target returns a real `data.cdp` with a parseable `websocket_url`. What does **not** cross is `LaunchResult.renderer`: macOS detects Chromium from the bundle (`crates/macos/src/system/renderer_kind.rs`) and Windows reports nothing, so the field is absent on Windows and the *unprompted* nudge core derives from it — launching a Chromium app **without** `--cdp` and being told to close and relaunch with it (`crates/core/src/commands/launch.rs`'s `launch_suggestion`) — never fires there. An agent that has not been told to ask for `--cdp` therefore walks a dense Electron tree on Windows where macOS would have redirected it. The detector is available rather than missing: §2.4 already ships Chromium recognition by window class (`crates/windows/src/tree/chromium.rs`, `Chrome_WidgetWin_1`), so the open question is placement and evidence, not feasibility — a window-class read needs a window, while macOS reads the bundle before one exists. Decide here whether Windows implements `renderer` detection (and on what signal, given a launch may report no window at all), or whether the divergence is ratified and the guidance documented as macOS-only, on the same identical-JSON-is-a-product-promise standard the entries above use and the same terms: a normalization lands as its own sub-phase PR before this gate merges, a ratified divergence is recorded here and needs no successor. **Evidence:** verified on this branch at the v0.8.1 merge — `launch --cdp` against an Electron target on Windows returned `cdp.port`, `cdp.websocket_url` and `product: Chrome/142.0.7444.265`, while the same target launched without `--cdp` returned neither `renderer` nor `suggestion`.
- **Settle steady-state windowless `close-app`.** On Windows, `list_apps` reports only window-owning processes, so a steady-state windowless process resolves `APP_NOT_FOUND` in core before the adapter close path runs; macOS can close windowless / menu-bar-only apps. A race that empties the top-level window set after core resolution maps to the class-(b) envelope `ACTION_FAILED`/`not_delivered` (never a silent `TerminateProcess` under a graceful envelope). Decide here whether the contract normalizes or ratifies the divergence. **Evidence:** §2.9/`close.rs` windowless graceful fallback, `list_apps_live` window-owning inventory
- **Settle launcher-style child-pid window attach.** A21-1 measured a helper whose visible window belongs to a child pid (`branch: launcher_style_child_pid_window`). Under `attach_if_running`, Windows matches by image name on the ToolHelp snapshot and then waits for an exact window at that pid — so a launcher-style product surfaces as attach ambiguity or `WINDOW_NOT_FOUND` rather than attaching the child's window. Decide here whether the contract normalizes (for example by walking child processes for the first accessible window) or ratifies the pid-exact attach rule. **Evidence:** A21-1
- **Record class-(b) Windows-only lifecycle envelopes** (conditions with no macOS pair — asserted against the envelope contract, not macOS equality): windowless graceful-close escalation → `ACTION_FAILED`/`not_delivered`; UIPI activation budget exhaustion on a strictly-higher-integrity target → `PERM_DENIED`/`not_delivered`; `CreateProcessW` invalid name → `INVALID_ARGS`/`not_delivered`. Protected-process close refusal is shared across platforms and ships as `INVALID_ARGS`/`not_delivered` via `close_app.rs` (`invalid_input_with_suggestion`), not `PERM_DENIED` (dogfood J2). Decide here only if any of these pairs should be renamed; otherwise record them as the Windows lifecycle envelope set. **Evidence:** §2.9 envelope-parity suite, dogfood J2, `crates/core/src/commands/close_app.rs`
- **Settle Explorer / multi-instance shell attach.** Dogfood J2 observed product `launch explorer.exe` (attach-default) return `AMBIGUOUS_TARGET`/`not_delivered` when multiple shell `explorer.exe` rows exist; `--no-attach` returns already-running `ACTION_FAILED`; folder args are ignored on the attach path. Decide here whether shell multi-instance attach needs a disambiguation rule (for example by command line / window title) or whether `AMBIGUOUS_TARGET` is the ratified contract for multi-row image matches. **Evidence:** 2026-08-08-001 dogfood J2
- **Settle whether the mutation-path delivery classifier promotes into `agent-desktop-core` beside the resolver-payload item.** §2.7 ships Windows `actions/mutation.rs` mirroring macOS `ax_mutation::classify`: both adapters pin the same `ErrorCode` / `DeliveryDisposition` / `RetryDisposition` pairings, but each classifies a platform-native failure space (`AXError` vs HRESULT/`UiaFailure` sentinel) and the outcome contract is deliberately per-adapter for Phase 2. Decide here — with both adapters in review — whether a shared mutation-outcome type or pairing table belongs in core, or whether the mirrored pairings stay adapter-local with the wire shapes as the only shared contract. A promotion that changes macOS to consume a core type lands as its own sub-phase PR before this gate merges; ratifying the per-adapter split is recorded here and needs no successor. **Evidence:** §2.7 scope (mutation classifier), A19-2 failure taxonomy, 2.7 dogfood envelope parity
- **Stop handing `AdapterError::stale_ref` a sentence where it expects a ref id.** `stale_ref` (`crates/core/src/adapter_error.rs:73`) takes a **ref id** and formats `"{ref_id} not found in current RefMap"`. Fourteen call sites hand it a whole sentence instead, so the message an agent reads and acts on comes out as `"Saved target has no process instance identity not found in current RefMap"` — ungrammatical, and wrong about the cause in every one of them: the ref was found and read, and it was the live evidence, the process generation, or the input geometry that refused it, never a missing RefMap entry. Ten sites are on macOS — `tree/resolve_errors.rs:4`, `tree/resolve.rs:52`/`:59`/`:62`, `tree/query/mod.rs:144`/`:149`/`:152`, `tree/renderer_probe.rs:9`, `actions/post_state.rs:115`, `actions/physical_click.rs:75` — and **four are in core itself**, so they are already reached on every platform: `ref_action.rs:60`, `headed_focus.rs:53`, `renderer_accessibility.rs:58`, `snapshot_ref.rs:125`. **What closes it:** each of the fourteen builds its error directly — `AdapterError::new(ErrorCode::StaleRef, message)` with the snapshot-refresh suggestion and the not-delivered disposition — exactly as Windows's `stale_evidence_error` (`crates/windows/src/tree/resolve_match.rs`) already does, leaving `stale_ref` to the two callers that genuinely pass a ref id (`crates/core/src/refs_store.rs:80`, `crates/core/src/commands/helpers.rs:317`) and a test that pins the constructor's message against a caller passing anything but an id. **Why here:** ten of the fourteen are in the macOS crate, the GA line for the whole platform phase, and the error-payload promotion above already opens `resolve_errors.rs` and `resolve.rs` — the same family of defect in the same two files, so this is one edit under one review rather than two. The four core sites travel with them rather than shipping a message set that reads one way in core and another in the adapters. **Evidence:** found while sub-phase 2.5 built the Windows resolver against the same constructor and rejected it for exactly this reason (`resolve_match.rs`'s recorded rationale)
- Merge `feat/windows-adapter` → `main` as one release-noted `feat!`

**Key APIs:** none — verification and merge only

**Depends on:** 2.0 through 2.14, including 2.12.1 — all of them merged; no Windows sub-phase may lag past this gate

**Exit criteria:** every item in the Cross-cutting sub-phase DoD holds for the whole branch; both cross-platform contract decisions above are settled in this document rather than left to the next platform to inherit; no call site on any platform passes `AdapterError::stale_ref` anything but a ref id, pinned by a test; `main` gains Windows support in one commit.

**Est. PR size:** ~500 LOC on top of a large verification effort. Two code changes are certain to land here: the error-payload promotion (core constructors plus both adapters and a core test) and the fourteen `stale_ref` call sites, which are small but spread across core and macOS. Either contract decision above may add a normalization change on one adapter. Everything else is review, live e2e, perf baseline, audits and the merge.

### Minimum OS Requirements

- Windows 10 1809+ is the API floor for the baseline UIA adapter, app/window operations, clipboard, and legacy screenshot fallback. Servicing reality (2026): mainstream Windows 10 reached end of support 2025-10-14 (consumer ESU through 2027-10-12); the serviced 1809-vintage targets are Windows 10 Enterprise LTSC 2019 and Windows Server 2019 (extended support to 2029-01-09), so Windows 11 and Server 2019+ are the practical release targets
- Windows 10 1903+ for `Windows.Graphics.Capture` per-window modern screenshot (cursor-capture toggle requires 19041+; border removal via `IsBorderRequired` requires 20348+)
- Newer Windows 10/11 builds may expose richer Quick Settings / notification / shell UIA trees; commands report `PLATFORM_NOT_SUPPORTED` or degrade to the documented fallback when a shell surface is absent
- UIA COM interfaces are available before Windows 10, but Phase 2 does not support pre-1809 as a release target
- Session 0, Server Core, secure desktop, locked desktop, and other-user sessions are explicitly unsupported for observation/action/capture

### New Dependencies

| Crate | Version | Scope | Purpose |
|-------|---------|-------|---------|
| `uiautomation` | 0.25 | Windows | UIA client wrapper, tree walker, patterns. Current stable 0.25.0 (2026-05-05); a `"0.24"` requirement will not auto-resolve to 0.25.x under 0.x semver, so the bump is explicit |
| `windows` | 0.62.2 | Windows | Raw Win32 / WinRT bindings for SendInput, clipboard, direct `Windows.Graphics.Capture`, D3D11 frame pool, WIC. Still current (2025-10-06); `uiautomation 0.25.0` pins `windows ^0.62.2` |
| `screencapturekit` | 1.5 (crates.io) | macOS | Published crates.io canonical crate — the doom-fish fork is the maintained successor, NOT a git-SHA pin |
| `objc2` | 0.6 | macOS (new for P2-O13 / O17) | Safe bridging to `SCScreenshotManager`, `CGPreflightScreenCaptureAccess`, and AppKit/Foundation calls scoped to screenshot/permissions code |

The Windows pins above were re-verified against crates.io on 2026-07-25 (sub-phase 2.0 evidence; no RUSTSEC advisories apply); 2.10's capture/clipboard work adds `windows` / `windows-sys` features in place rather than a new crate. The macOS pins (`screencapturekit`, `objc2`) still carry the 2026-04 recording. Re-verify any pin against crates.io and the repository's supply-chain policy at the opening sub-phase of the consuming platform before adding it to `Cargo.toml`.

Added as target-gated dependencies in the owning platform crates. The binary crate only depends on the platform crate for the current target. Feature set below matches shipped `crates/windows/Cargo.toml` after 2.10:
```toml
# src/Cargo.toml
[target.'cfg(target_os = "windows")'.dependencies]
agent-desktop-windows = { path = "crates/windows" }

[target.'cfg(target_os = "macos")'.dependencies]
agent-desktop-macos = { path = "crates/macos" }

# crates/windows/Cargo.toml
[target.'cfg(target_os = "windows")'.dependencies]
uiautomation = { version = "0.25", default-features = false, features = [
  "control",
  "input",
] }
windows = { version = "0.62.2", features = [
  "Foundation",
  "Graphics_Capture",
  "Graphics_DirectX_Direct3D11",
  "Win32_Graphics_Direct3D",
  "Win32_Graphics_Direct3D11",
  "Win32_Graphics_Dxgi_Common",
  "Win32_Graphics_Gdi",
  "Win32_Graphics_Imaging",
  "Win32_System_Com",
  "Win32_System_Com_StructuredStorage",
  "Win32_System_WinRT_Direct3D11",
  "Win32_System_WinRT_Graphics_Capture",
  "Win32_UI_Accessibility",
] }
windows-sys = { version = "0.61", features = [
  "Win32_Foundation",
  "Win32_System_Com",
  "Win32_UI_HiDpi",
  "Win32_Storage_FileSystem",
  "Win32_Security",
  "Win32_Security_Authorization",
  "Win32_System_ApplicationInstallationAndServicing",
  "Win32_System_LibraryLoader",
  "Win32_System_Threading",
  "Win32_System_Diagnostics_ToolHelp",
  "Win32_System_DataExchange",
  "Win32_System_Memory",
  "Win32_Graphics_Gdi",
  "Win32_Graphics_Dwm",
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_UI_WindowsAndMessaging",
] }

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
- [ ] Windows permissions section: UIA works without special permissions for most apps; UAC elevation may be required for elevated processes; pre-Chromium-138 apps may need `--force-renderer-accessibility` (Chromium 138+ auto-enables UIA)
- [ ] "From source" section updated with Windows build requirements (Rust + MSVC)

---

## Phase 3 — Linux Adapter

**Status: Planned** — delivered as sub-phases 3.0–3.15 into the `feat/linux-adapter` integration branch, mirroring the Windows sub-phase template ([Platform Delivery Model](#platform-delivery-model--sub-phases-and-integration-branches)) numbered slot for numbered slot, with AT-SPI2/D-Bus specifics substituted for UIA. Windows's §2.12.1 has no Linux counterpart in the mirror: it answers a Win32 handle-table hazard, and Linux gains an equivalent only if the same recycled-identifier hazard is measured on X11 or Wayland. Phase 3 begins only after Phase 2 (2.0–2.15) has merged to `main`.

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

### Sub-phase decomposition (3.0–3.15, mirrors Phase 2's numbered slots)

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

### 3.14 — Shell Surfaces & Notifications

**Goal:** Cover the Linux-only shell surface, notification, and tray scope — the DE-specific half of parity that has no single canonical implementation across GNOME/KDE/other DEs.

**Scope:** notification management and system tray, folded in below. Ships inside Phase 3 before the 3.15 integration merge, under the same no-convenience-deferral rule as Windows 2.14.

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

**Depends on:** 3.0 through 3.14 — all of them merged; no Linux sub-phase may lag past this gate

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
| `desktop_list_displays` | `list-displays` | Array of displays with `scale` (effective DPI / 96, core's field name) |
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

### Core-Owned Rules Must Have a Production Caller (enforced across all phases)

A rule this document presents as core-owned is canonical only if a production path calls it. A `pub` function in `agent-desktop-core` that has tests but no caller, shadowed by a platform crate's reimplementation, is worse than no rule at all: it reads as the definition while the copy the walk actually runs drifts from it silently, because tightening core's version leaves core's own test green and every snapshot keeps being graded by the copy. The workspace has produced this shape twice.

- **Accessible-name computation (closed).** `crates/core/src/accname.rs` shipped exported and tested with no production caller, while `crates/macos/src/tree/query/evidence_fields.rs` ranked the sources differently and was what reached every snapshot (Phase 1.6 U11). Sub-phase 2.3 reconciled them into one shared implementation both adapters call, and that reconciliation is one of the three reasons its shipped size exceeded its estimate — the cost of finding this shape late, not the cost of the fix.
- **Evidence completeness (open).** `required_complete` (`crates/macos/src/tree/query/node_evidence.rs`) restates `LocatorEvidence::satisfies` (`crates/core/src/live_locator/locator_evidence.rs`) clause for clause — the same eight requirement-gated `is_unknown`/`is_complete` tests, in the same order, differing only in that one reads `self` and the other a parameter. Core's `satisfies` has **no production call site anywhere in the workspace**, only its own unit test, while the macOS copy is the definition the walk actually runs (`crates/macos/src/tree/query/node_read.rs`'s `evidence_complete`). Add a field to `EvidenceRequirements` or change how an `Unknown` slot is judged, and core's test goes green while the macOS walk keeps grading by the old rule and reports nodes complete that core's own definition rejects; nothing anywhere fails. **What closes it:** delete `required_complete`, call `evidence.satisfies(requirements)` at the macOS call site, and keep a test that executes core's rule through an adapter path so it can never go callerless again. No core visibility change is needed — `satisfies` is already `pub` on a `pub` struct re-exported from `crates/core/src/lib.rs`, and the macOS file already imports `EvidenceRequirements` from core. **It is independent of the Windows branch and of every platform phase:** it touches the macOS crate and no other, needs no Windows or Linux leg, and can land as a standalone macOS PR at any time — the verification it needs is macOS e2e, macOS review and a macOS perf baseline, which any macOS-side PR runs. **Evidence:** `git log -S "fn satisfies" -- crates/` shows one origin (`3f32272`) and no later move, so the copy was authored alongside the original rather than left behind by a migration; carried unowned through the 2.3, 2.4 and 2.5 plans' follow-up lists.

**Enforced on every PR:** no platform crate may add a further copy of a core-owned rule. Windows consumes `EvidenceRequirements` already (`crates/windows/src/tree/element_properties.rs`, `properties.rs`) but has no requirements-driven completeness gate — `essential_live_evidence_complete` (`crates/windows/src/tree/live_read.rs`) is the narrower five-slot live-read rule paired with macOS's `post_state.rs`, a different rule serving a different path. The first sub-phase on any platform that needs requirements-gated completeness calls core's rule rather than reimplementing it locally, however demonstrated that pattern looks in the codebase, and Phase 3's Linux adapter is held to the same rule.

### CI Matrix Evolution

| Phase | CI Runners / Jobs |
|-------|-----------|
| Phase 1 | `macos-latest` (tests + CLI build) + `ubuntu-latest` (`fmt` job) |
| Phase 1.5 | Same as Phase 1 on PRs; release workflow fans out to `macos-latest` × 2 darwin arches + `ubuntu-22.04` + `ubuntu-22.04-arm` + `windows-latest` for the FFI matrix |
| Phase 1.6 | `ci.yml`: `fmt` (ubuntu-latest), `msrv` (ubuntu-latest, Rust 1.89.0), `platform-check` (matrix Linux/Windows/macOS, `cargo check` only), `test` (macos-latest, full suite), `ffi-python-smoke`, `ffi-header-drift`, `ffi-panic-guard`, `ffi-passthrough` (ubuntu-latest). Outside `ci.yml`: `native-e2e.yml` (self-hosted macOS, workflow_dispatch), `codeql.yml`, `supply-chain.yml` |
| v0.6.0 (current) | Real `test-windows` (`windows-latest`) and `test-linux` (`ubuntu-latest`) lanes execute `cargo test -p agent-desktop-core -p agent-desktop-{windows,linux} --lib` on every PR, alongside the macOS `test` job — core's platform-conditional code is now executed, not merely type-checked, on all three OSes. Hot-path performance baseline remains a per-PR Definition-of-Done review step (macOS: `scripts/perf-baseline-compare.sh`; Windows: probe corpus cost methodology, A15-13 / A18-7), not a blocking job |
| Phase 2 | The Windows test lane already exists as of v0.6.0; sub-phase 2.1 extends it to the adapter surface (clippy over `agent-desktop-windows`, binary-crate tests, size check); the self-hosted interactive Windows runner is registered at 2.12, the sub-phase whose UIA/shell integration lane needs it |
| Phase 3 | The Linux test lane already exists as of v0.6.0; sub-phase 3.1 extends it to the adapter surface; an interactive Ubuntu GNOME runner is added for AT-SPI2/shell integration tests at 3.12 |
| Phase 4 | macOS + Windows + Ubuntu (+ MCP protocol tests) |
| Phase 5 | macOS + Windows + Ubuntu (+ daemon tests, package build verification) |

Every runner runs the tests for the packages that build on that OS (`cargo test --workspace` on macOS; the Windows and Linux lanes scope to `agent-desktop-core` plus their own platform crate, since `agent-desktop-macos` does not compile off macOS — and `--lib` alone never covers the `agent-desktop` binary crate, which has no lib target). The other three gates are macOS-only today: `cargo clippy --all-targets -- -D warnings`, the `cargo tree -p agent-desktop-core` isolation check and the <15MB binary-size cap all run in the `test` job alone, while `test-windows` and `test-linux` each run a single `--lib` test invocation. Extending those three to the Windows and Linux lanes is sub-phase 2.1's and 3.1's work. Every Phase 2/3 sub-phase additionally runs a hot-path performance baseline — macOS via `scripts/perf-baseline-compare.sh`, Windows via the probe corpus cost methodology (A15-13 / A18-7) — see the [Cross-cutting sub-phase DoD](#cross-cutting-sub-phase-dod).

### Dependency Introduction Schedule

| Dependency | Introduced In | Purpose |
|------------|---------------|---------|
| `clap` 4.6, `serde`/`serde_json` 1.x, `thiserror` 2.0, `tracing` 0.1+, `base64` 0.22+ | Phase 1 | Core: CLI, JSON, errors, logging, encoding |
| `tracing-subscriber` 0.3, `rustc-hash` 2.1, `smallvec` 1.13 | Phase 1 | Log formatter, fast hashing, small vectors in hot paths |
| `accessibility-sys` 0.2.0, `core-foundation` 0.10.1, `core-foundation-sys` 0.8.7, `core-graphics` 0.25.0 | Phase 1 | macOS AX API FFI |
| `cbindgen` maintainer tool, `libc` 0.2+ | Phase 1.5 | Explicit C header regeneration + macOS `pthread_main_np` for FFI main-thread guard |
| *(no new external crates — a contract-hardening pass over existing dependencies)* | Phase 1.6 | — |
| `uiautomation` 0.25 | Phase 2 | Windows UIA wrapper |
| `windows` 0.62.2 | Phase 2 | Win32 / WinRT bindings including direct WGC / D3D11 / WIC (`uiautomation 0.25` pins `^0.62.2`) |
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
| `windows-capture` | 2.10 (KTD2) | Mandated diff-audit against the once-pinned 2.0.0 line found a video-recording library (deps include `rayon`; crates.io 2.0.1 supersedes 2.0.0) whose `windows` feature set includes `Win32_UI_Shell` — the manifest surface §2.9 declined as a reviewed decision. Modern capture ships as direct `Windows.Graphics.Capture` through the existing `windows` crate instead. |

### Platform API Quick Reference

| Capability | macOS | Windows | Linux |
|------------|-------|---------|-------|
| Tree root | `AXUIElementCreateApp(pid)` | `IUIAutomation.ElementFromHandle()` | `atspi Accessible` on bus |
| Children | `kAXChildrenAttribute` | `TreeWalker.GetFirstChild` | `GetChildren` D-Bus |
| Stable ID | `AXIdentifier` / `AXDOMIdentifier` (shipped, U5) | UIA `AutomationId` (Phase 2, sub-phase 2.3) | AT-SPI2 `accessible-id` (Phase 3, sub-phase 3.3) |
| Click | `AXPress` | `InvokePattern.Invoke()` | `Action.DoAction(0)` |
| Set text | `AXValue = val` | `ValuePattern.SetValue()` | `Text.InsertText` |
| Keyboard | `CGEventCreateKeyboard` | `SendInput` | `xdotool` / `ydotool` |
| Clipboard | `NSPasteboard`, typed `ClipboardContent` (shipped, U18) | Win32 Clipboard API, typed (shipped in 2.10; save/restore + lock hermeticity, A22-5) | `wl-clipboard` / `xclip`, typed (Phase 3) |
| Screenshot | `ScreenshotBackend` over secure `screencapture` path today; ScreenCaptureKit planned (P2-O13) | Direct `Windows.Graphics.Capture` with silent `BitBlt` / `PrintWindow` Legacy fallback (shipped in 2.10; A22-1) | `PipeWire` / `XGetImage` (P3-O11) |
| Permissions | `AXIsProcessTrusted()` | COM security / UAC | Bus availability |
| Notifications | Notification Center AX tree (`com.apple.notificationcenterui`) | UIA tree of Action Center / Toast Manager | D-Bus `org.freedesktop.Notifications` + daemon-specific history |
| System tray | `SystemUIServer` AX tree + `ControlCenter` AX tree | UIA tree of `Shell_TrayWnd` + overflow window | D-Bus `StatusNotifierWatcher` + XEmbed fallback |

---

## Risk Register

| ID | Risk | Likelihood | Impact | Mitigation |
|----|------|------------|--------|------------|
| R1 | macOS TCC friction deters adoption | High | High | Clear first-run guidance. Detect before any op. One-command setup: `permissions --request`. |
| R2 | Electron/Chromium a11y tree gaps on legacy builds | Medium | Medium | Chromium 138+ (Aug 2025) auto-enables native UIA for UIA clients. Detect Chromium windows; re-read past a settle before judging a tree thin, since a first read lands on the pre-activation shell; print `--force-renderer-accessibility` guidance for pre-138/pinned builds or trees still thin afterwards. |
| R3 | Custom-rendered UIs invisible to a11y | Medium | High | Phase 5 stretch: vision fallback. Short-term: document limitation in README and skills. |
| R4 | Wayland a11y gaps | Medium | Medium | Focus on GNOME (best AT-SPI2 support). Prefer AT-SPI actions over coordinate input. Document gaps. |
| R5 | Rust a11y crate maintenance stalls | Low | High | Pin versions, maintain patches. `atspi` backed by the Odilia project. Fork-ready. |
| R6 | MCP spec changes break compat | Low | Medium | Pin `rmcp` version. Monitor spec under Linux Foundation governance. |
| R7 | Tree traversal too slow (>5s) | Medium | Medium | Depth limiting via `--max-depth`. Focused-window-only. Cached subtrees in Phase 5 daemon. Progressive skeleton traversal (`--skeleton` + `--root`) reduces token consumption 78-96% for dense apps. |
| R8 | Ref instability confuses agents | Medium | High | Clear docs: refs are snapshot-scoped and snapshot-qualified (`@<snapshot_id>:e<n>`). `STALE_REF` error with recovery hint. Progressive skeleton traversal with scoped invalidation provides a stable drill-down workflow. Stable `native_id` evidence (shipped on macOS; Windows/Linux land it in their own vocabulary sub-phase) reduces stale-resolution failures on Electron and localized apps. |
| R9 | Headless operation requirement | High | Critical | Phase 1 introduced `ActionRequest`/`InteractionPolicy`, default no focus steal/cursor movement, and explicit physical/headed policy paths; Phase 1.6 added default-on auto-wait and the occlusion gate on top. Phase 2/3 preserve the same contract for Windows/Linux. |
| R10 | Command registry link-GC | Medium | High | Research Topic B confirmed `inventory`/`linkme` are unreliable across linkers for cdylib consumers. Resolved by pure `build.rs` filesystem enumeration — zero linker magic, once that registry migration (P2-O16) lands. |
| R11 | Skeleton traversal cross-platform | Low | High | Core is already platform-agnostic (`crates/core/src/snapshot_ref.rs`); Windows needs ~50 LOC glue (raw-view walk + `IsControlElement` filter + `FindAll(TreeScope_Children, TrueCondition)` + fresh `UICacheRequest` per drill-down). Research Topic 4 confirmed `ElementFromHandle(hwnd)` is headless-safe. |
| R12 | RDP / session-isolation blocks Windows dev and CI | Medium | High | UIA requires an interactive session — an RDP disconnect can drop the console session to a non-interactive state. Document the `tscon` console-reattach workaround for the self-hosted runner (sub-phase 2.12, where that runner is registered); mirrors the macOS exclusive-desktop gate (`AGENT_DESKTOP_E2E_EXCLUSIVE=1` + `interaction_lock.py`) that already serializes native e2e runs. |
| R13 | UIA event handler MTA lifecycle leaks | Medium | Medium | `RemoveAutomationEventHandler` races the final in-flight callback dispatch on the MTA worker thread if torn down naively. Use the post-remove-barrier pattern (`Arc<Handler>` outlives the final callback) documented in the Windows Engineering Invariants — apply it from the first sub-phase that registers a handler (`watch`, once P2-O11 lands), not retrofitted after a leak is observed. The barrier is not where the cost is: 2.0 measured removal under 296 in-flight events at 72 ms, but removal on an *idle* stream after window open/close churn while handlers were registered at 86 s — 63 ms without that churn, superlinear, and not avoided by making callbacks cheap. Bound and budget handler removal, and avoid holding handlers across window open/close churn. |
| R14 | Merge-train discipline: integration branch drifts from `main` | Medium | Medium | Seventeen sub-phases landing serially into `feat/windows-adapter` (then `feat/linux-adapter`) is a long-lived branch by construction. Mitigate with a rebase cadence (rebase onto `main` at the start of each sub-phase, not just before the final merge) and treat each sub-phase's own review as a checkpoint rather than deferring all review to the 2.15/3.15 hardening pass. |
