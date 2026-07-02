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
bash tests/e2e/run.sh                          # E2E: real binary vs fixture app, verify by observation (needs --release + AX permission)
```

Run the binary: `./target/release/agent-desktop snapshot --app Finder -i`

The E2E harness drives the release binary against a real SwiftUI/AppKit fixture and asserts every effect by independent observation (never the command's own `ok:true`), covering every ref action in **both** headless and `--headed` mode. See `tests/e2e/README.md`.

## Pre-commit Hook

The repo ships a pre-commit hook at `.githooks/pre-commit` that runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --lib --workspace` against staged Rust changes. Wire it up once after cloning:

```bash
git config core.hooksPath .githooks
```

Bypass for an emergency commit with `git commit --no-verify` or `SKIP_PRECOMMIT=1 git commit ...`.

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
  - `perf:` — performance improvement with no behavior change
  - `chore:` — maintenance tasks, dependencies
  - `ci:` — CI/CD changes
  - `test:` — adding or fixing tests
- Format: `type: concise imperative description` (lowercase type, no capital after colon)
- Focus on "why" not "what"
- Examples: `feat: add scroll-to command`, `fix: prevent stale ref on window resize`, `ci: add binary size check`
- **Pre-1.0 versioning policy** (release-please `bump-minor-pre-major` + `bump-patch-for-minor-pre-major`): while the version is 0.x, a `BREAKING CHANGE` cuts a **minor** (0.2 → 0.3) and a `feat:` cuts a **patch**. Do not expect a major release before 1.0.

## Core Principle

agent-desktop is NOT an AI agent. It is a tool that AI agents invoke. It outputs structured JSON with ref-based element identifiers. The observation-action loop lives in the calling agent.

## Architecture

### Workspace Layout

```
agent-desktop/
├── Cargo.toml              # workspace: members, shared deps
├── CONCEPTS.md             # shared domain vocabulary for refs, snapshots, sessions, actionability, and related concepts
├── rust-toolchain.toml     # pinned Rust version
├── clippy.toml             # project-wide lint config
├── crates/
│   ├── core/               # agent-desktop-core (platform-agnostic)
│   │   └── src/
│   │       ├── ref_alloc.rs      # Shared ref helpers (INTERACTIVE_ROLES, is_collapsible)
│   │       ├── snapshot_ref.rs   # Ref-rooted drill-down (run_from_ref)
│   │       └── commands/         # one file per command
│   ├── macos/              # agent-desktop-macos (Phase 1)
│   ├── windows/            # agent-desktop-windows (stub → Phase 2)
│   ├── linux/              # agent-desktop-linux (stub → Phase 2)
│   └── ffi/                # agent-desktop-ffi (cdylib + committed C ABI header)
├── src/                    # agent-desktop binary (entry point)
│   ├── main.rs             # entry point, permission check, JSON envelope
│   ├── batch/              # batch JSON → typed Commands
│   ├── cli/                # clap derive enum, help text, CLI contract tests
│   ├── cli_args/           # command argument structs by domain
│   ├── command_policy/     # permission/ref/side-effect policy
│   ├── dispatch/           # command dispatcher, parse helpers, notifications
│   └── tests/              # binary-level conformance tests
├── docs/
│   └── solutions/          # documented solutions to past problems (bugs, best practices, workflow patterns), organized by category with YAML frontmatter (module, tags, problem_type); relevant when implementing or debugging in documented areas
└── tests/
    ├── fixtures/           # golden JSON snapshots
    └── integration/        # macOS CI integration tests
```

### Dependency Inversion (Non-Negotiable)

- `agent-desktop-core` defines the `PlatformAdapter` trait and all shared types
- Platform crates (`macos`, `windows`, `linux`) implement the trait
- **Core NEVER imports platform crates.** Platform crates NEVER import each other.
- Two legitimate wiring points bring platform → core together:
  1. The binary crate (`src/`) — CLI consumers
  2. The FFI crate (`crates/ffi/`) — cdylib consumers (Python, Swift, Go, Node, C++)
- CI enforces core isolation: `cargo tree -p agent-desktop-core` must contain zero platform crate names

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
pub fn dispatch(
    cmd: Commands,
    adapter: &dyn PlatformAdapter,
    permission_report: &PermissionReport,
) -> Result<serde_json::Value, AppError> {
    match cmd {
        Commands::Snapshot(args) => commands::snapshot::execute(args, adapter),
        Commands::Click(args) => commands::click::execute(args, adapter),
        // one arm per command
    }
}
```

Batch is not a second dispatcher. `src/batch/mod.rs` deserializes JSON entries into the same typed `Commands` enum, runs the same `CommandPolicy` preflight, and calls the same `dispatch()` path as CLI.

### Additive Phase Model

- **Phase 1:** Foundation + macOS MVP (56 commands, core engine, macOS adapter)
- **Phase 2:** Windows + Linux adapters, 10+ new commands — core untouched
- **Phase 3:** MCP server mode via `--mcp` flag — wraps existing commands
- **Phase 4:** Daemon, sessions, enterprise quality gates

Phases 2–4 add adapters, transports, and production readiness work. Nothing in core is rebuilt.

## Coding Standards

### File Rules

- **400 LOC hard limit per file.** If approaching 400, split by responsibility. No exceptions. _Exception: files bearing an `@generated` marker that are produced by `build.rs` codegen and validated by a CI drift gate are exempt — the limit applies to the hand-written templates and the build script itself, not the generated output. Do not hand-edit these files; fix the generator instead._
- **No inline comments.** Code must be self-documenting through naming. Only Rust doc-comments (`///`) on public items when the name alone is insufficient.
- **One struct/enum per file** for domain types. `node.rs` defines `AccessibilityNode`. `action.rs` defines `Action`.
- **One command per file.** Each CLI command lives in its own file under `commands/`. Filename matches the command name. _This rule scopes to hand-written command files; a single `@generated` wrapper file that consolidates multiple command entrypoints is not a violation._
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
ACTION_NOT_SUPPORTED, STALE_REF, AMBIGUOUS_TARGET, WINDOW_NOT_FOUND,
PLATFORM_NOT_SUPPORTED, TIMEOUT, INVALID_ARGS, NOTIFICATION_NOT_FOUND,
SNAPSHOT_NOT_FOUND, POLICY_DENIED, INTERNAL
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
│   ├── capabilities.rs # AX-supported actions and settable attributes
│   ├── builder.rs      # build_subtree, tree traversal
│   ├── roles.rs        # Role mapping
│   ├── resolve.rs      # Element re-identification
│   └── surfaces.rs     # Surface detection
├── actions/            # Interacting with elements
│   ├── mod.rs          # re-exports
│   ├── dispatch.rs     # perform_action match arms
│   ├── activate.rs     # Smart AX-first activation chain
│   ├── extras.rs       # select_value helpers
│   ├── scroll.rs       # scroll semantics and gated physical fallback
│   └── type_text.rs    # headless text insertion and physical typing
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
3. Add the CLI subcommand variant to `src/cli/mod.rs` and arguments under `src/cli_args/`
4. Add a match arm in `dispatch()` in the binary crate
5. If new `Action` variant needed, add to `crates/core/src/action.rs`
6. If new adapter method needed, add to `PlatformAdapter` trait with a default returning `Err(AdapterError::not_supported())`

No existing files are modified beyond the registration points. Enforce via code review.

## JSON Output Contract

Every command produces a response envelope:

```json
{
  "version": "2.0",
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
  "version": "2.0",
  "ok": false,
  "command": "click",
  "error": {
    "code": "STALE_REF",
    "message": "Element could not be resolved from the requested snapshot",
    "suggestion": "Run 'snapshot' to refresh, then retry with updated ref"
  }
}
```

The `error` object may also carry an optional `details` object (e.g. the actionability report on an actionability failure, candidate summaries on `AMBIGUOUS_TARGET`, or the last observed state on a `wait` `TIMEOUT`).

### Serialization Rules

- Omit null/None fields (`#[serde(skip_serializing_if = "Option::is_none")]`)
- Omit empty arrays (`#[serde(skip_serializing_if = "Vec::is_empty")]`)
- Omit bounds in compact mode
- `ref_count` and `tree` go inside `data`, not as top-level siblings

## Ref System

- Refs allocated in depth-first document order: `@e1`, `@e2`, etc.
- An element receives a ref when it is **addressable for an action**: its role is interactive (`button`, `textfield`, `checkbox`, `link`, `menuitem`, `tab`, `slider`, `combobox`, `treeitem`, `cell`, `radiobutton`, `switch`, `colorwell`, `menubutton`, `incrementor`, `dockitem`), **or** it advertises an available action regardless of role. Container roles such as `scrollarea` (Scroll) and `disclosure` (Expand/Collapse/Click) are not interactive by role but are genuinely actionable, so they are ref-able — `scroll` / `expand` / `collapse` need a ref to target them
- A bare `SetFocus` affordance does not qualify on its own (focusability is not a primary action), so inert focusable containers stay ref-less
- Static text and non-actionable groups/containers do NOT get refs (they remain in tree for context)
- Refs are deterministic within a snapshot but NOT stable across snapshots if UI changed
- Snapshot refs are stored by snapshot ID under `~/.agent-desktop/snapshots/{snapshot_id}/refmap.json`, with a `latest_snapshot_id` pointer for commands that omit `--snapshot`
- `~/.agent-desktop/last_refmap.json` is written only as a latest-snapshot inspection artifact; command code must use `RefStore`
- Action commands use strict re-identification from platform-neutral `RefEntry` evidence: pid, role, path/source surface, role-conditional stable text identity, and bounds hash. Mutable control values are volatile and must not be treated as stable text identity. Return `STALE_REF` on mismatch and `AMBIGUOUS_TARGET` when multiple plausible live candidates remain.
- Progressive traversal: `--skeleton` clamps depth to 3, annotates truncated containers with `children_count`. Named/described containers at boundary receive refs as drill-down targets
- Drill-down: `--root @ref` starts from a previously-discovered ref with scoped invalidation (only that ref's subtree refs are replaced on re-drill)
- RefMap size check: write-side guard prevents >1MB refmap files
- **Sessions:** `session start` creates a manifest-gated session under `~/.agent-desktop/sessions/<id>/`, sets `current_session`, and (by default) enables automatic trace segments. Bare `--session <id>` without a manifest scopes only the snapshot namespace — no surprise trace files
- **Trace:** manifest `trace: on` writes per-process JSONL segments under `<session>/trace/<pid>-<procTs>.jsonl`; `--trace <path>` overrides to one file; activation resolves `--session` > `AGENT_DESKTOP_SESSION` > `current_session`

## PlatformAdapter Trait

Core defines `PlatformAdapter`; platform crates implement it. Methods default to
`not_supported()`, so an adapter only implements what it supports. Read the
current signatures in `crates/core/src/adapter.rs` — notably strict resolution
(`resolve_element_strict*` → STALE_REF on 0, AMBIGUOUS_TARGET on 2+), live reads
for the actionability preflight (`get_live_*`), and `is_protected_process`
(keeps platform-specific process names out of core).

## macOS Adapter Gotchas

- **Ancestor-path set, not a global visited set** — macOS reuses
  `AXUIElementRef` pointers across sibling branches, so a global visited set
  would prune real subtrees.
- **`AXElement` memory safety** — inner field is `pub(crate)` (prevents
  double-free via raw pointer extraction); `Clone` must `CFRetain`, `Drop` must
  `CFRelease`.
- **Batch attribute reads** — use `AXUIElementCopyMultipleAttributeValues`
  (3-5x faster than per-attribute fetches).

## Testing

- Unit tests use an in-memory `MockAdapter`; golden fixtures in `tests/fixtures/`
  regression-test serialization.
- macOS CI integration tests drive real apps (Finder, TextEdit, System Settings).
- `tests/e2e/run.sh` drives the release binary against the SwiftUI fixture and
  verifies every effect by independent observation in both headless and
  `--headed` mode (see `tests/e2e/README.md`).

## CI Requirements

- GitHub Actions macOS runner executes full test suite on every PR
- `cargo tree -p agent-desktop-core` must not contain platform crate names
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --workspace`
- Binary size check: fail if release binary exceeds 15MB

## Commands

56 commands spanning App/Window, Observation, Interaction, Scroll, Keyboard,
Mouse, Notifications (macOS), Clipboard, Wait, System (including `session`), and
Batch. The full surface and per-command reference live in `skills/agent-desktop/`.
All 56 are implemented on macOS (Phase 1); Windows/Linux (Phase 2/3) target the
same surface. Adding a command: see the Extensibility Pattern above.

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
