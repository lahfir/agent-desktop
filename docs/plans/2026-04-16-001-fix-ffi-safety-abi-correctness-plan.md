---
title: "fix: resolve PR #22 FFI safety and ABI correctness findings"
type: fix
status: active
date: 2026-04-16
reviewed: 2026-04-16
origin: https://github.com/lahfir/agent-desktop/pull/22#pullrequestreview-4102879703
---

> **Review status.** This plan was reviewed on 2026-04-16 by five reviewer personas (coherence, feasibility, security-lens, scope-guardian, adversarial). A P0 finding — Cargo rejecting per-package `panic` override — was verified experimentally (spike-compile) and the plan was restructured:
> - **Unit 1** now uses a dedicated `[profile.release-ffi]` inheriting from release (custom profile verified: `cargo run --profile release-ffi --example panic_spike` catches panic and returns cleanly).
> - **Unit 5** split into 5a/5b/5c; **Unit 6** split into 6a/6b/6c; **Unit 8** split into 8a/8b; **Unit 13** split into 13a/13b/13c. Each atomic-commit-sized.
> - **Unit 10** (action constructor helpers) dropped — out of review scope, surface expansion, consumer wrappers make helpers moot.
> - **New Unit 6d**: `ad_abi_version()` + compile-time `AD_ABI_VERSION` constant to prevent silent heap corruption from header/dylib mismatch throughout pre-1.0.
> - **New Unit 11b**: minimal C-side test harness exercising struct memcpy, enum fuzzing, interior-NUL, double-free, list-handle-after-free (Rust-only tests cannot exercise these bug classes).
> - **Unit 11** main-thread check elevated to unconditional release-mode enforcement (`pthread_main_np()` → `ErrInternal`); Send+Sync on `NativeHandle` removed and replaced with FFI-layer thread-ownership tracking.
> - **Unit 7** makes `AdActionResult` opaque (matches list-handle pattern) to close memcpy-copy double-free vector.
> - **Security elevation**: `AXIsProcessTrusted` inheritance when Python/Node dlopens the cdylib is documented as a privilege-escalation vector, not a threading note. Header + skill carry a P0 security callout.
> - **Unit 4** gains an Alternatives Considered section (BFS vs DFS-additive); BFS pruning parity is property-tested against current DFS output.
> - **cbindgen opaque_types** list added to Unit 5 (cbindgen silently skips non-repr(C) structs otherwise).
> - **`publish = false`** guard on `crates/ffi/Cargo.toml` added as Unit 0 (prevents accidental cargo-publish until distribution plan lands).
> - **`AdRefEntry` partial-freeze** moved to Deferred Tasks with explicit rationale and user sign-off checkpoint.
> - **tracing output** in cdylib mode: Unit 7 redacts AX string content (log field name + NUL position only) to prevent host-log data leakage until `ad_set_log_callback` lands in Phase 2.
> - **New Phase 0 / Unit R (user request, 2026-04-16)**: refactor the entire `crates/ffi/` crate to the project's modular file layout **before any safety work**. Split `types.rs` (28 packed types) into `types/` subdirectory with one struct/enum per file; mirror the platform-crate folder pattern (`convert/`, `tree/`, `actions/`, `apps/`, `windows/`, `input/`, `screenshot/`, `surfaces/` each with submodules); strip every inline `//` comment repo-wide in the crate; replace wildcard `pub use types::*` with explicit re-exports; remove the `.unwrap()` in `error.rs:56`; eliminate unjustified `#[allow(dead_code)]` annotations; keep every file under the 400 LOC hard limit. `///` doc-comments retained only where the item name isn't self-documenting. All subsequent units operate on the post-refactor paths.

# FFI Safety and ABI Correctness

## Overview

PR #22 introduced `crates/ffi/` — a new `cdylib + cbindgen` C FFI layer exposing `PlatformAdapter` over the C ABI so Python / Swift / Go / Node / C++ consumers can drive agent-desktop without forking a subprocess per call.

The PR review identified 24 findings (4 blockers, 11 important, 5 smaller, 4 nits) that must be resolved before the first `agent_desktop.h` ships. Once the header is distributed, every one of these fixes becomes a breaking change for downstream consumers.

This plan sequences the fixes so safety primitives (panic boundary, enum validation, error lifetime) land first and are reused across later units, and so every ABI-shape change (opaque list handles, `surface` field, `focused_only`, `ad_free_handle`, `AdRefEntry` completion) lands together and before publication.

**Target branch:** `feat/c-bindings` (PR #22 is currently open).

## Problem Frame

The reviewer ([lahfir](https://github.com/lahfir), CHANGES_REQUESTED on PR #22) identified that the FFI crate as submitted is architecturally correct but contains multiple soundness bugs that are either undefined behavior, heap-corruption primitives, or silent data loss:

- Flat-tree child ranges walk into grandchildren (API contract violation, silent wrong data)
- Error-string pointers dangle after any subsequent successful FFI call (use-after-free)
- `#[repr(i32)]` enum fields are read from C-supplied structs with no range validation (instant UB on invalid discriminant)
- Workspace release profile is `panic = "abort"`, making every panic inside an `extern "C"` fn immediately `SIGABRT` the host Python / Swift / Node process

Additionally, several ABI-shape omissions (no `surface` field on `AdTreeOptions`, no `focused_only` on `ad_list_windows`, no `ad_free_handle`, no notification FFI, no observation primitives) would require breaking changes after publication.

## Requirements Trace

Numbered against the 24 findings in the origin review for traceability:

- **R1.** (Blocker #1) Direct-child iteration over `AdNode.child_start..child_start+child_count` must return only direct children, not descendants.
- **R2.** (Blocker #2) `ad_last_error_{code,message,suggestion,platform_detail}` pointers must remain valid across subsequent successful FFI calls; only the next *failing* call invalidates them.
- **R3.** (Blocker #3) Every `#[repr(i32)]` enum field read from a C-supplied struct must be validated via range check before conversion; invalid discriminants return `AD_RESULT_ERR_INVALID_ARGS`.
- **R4.** (Blocker #4) Every `extern "C"` function wraps its body in `std::panic::catch_unwind`; the cdylib's release profile is overridden to `panic = "unwind"` per-package.
- **R5.** (Important #5) Every successful `ad_resolve_element` (and future `ad_find`) has a matching `ad_free_handle` that `CFRelease`s on macOS; core trait gains `release_handle(&NativeHandle)` method with platform impls.
- **R6.** (Important #6) Every FFI function that writes an out-param zeroes it at entry, before any fallible work.
- **R7.** (Important #7) Debug builds assert `pthread_main_np()` at every macOS-sensitive entry point; the header loudly documents the main-thread requirement.
- **R8.** (Important #8) List-returning FFI functions return opaque handles (`AdWindowList`, `AdAppList`, `AdSurfaceList`, `AdNotificationList`) with `_count` / `_get` / `_free` accessors; raw `ptr + count` APIs are replaced entirely. `AdImageBuffer` tracks its allocation length internally.
- **R9.** (Important #9) `AdTreeOptions` carries a `surface` field covering all seven `SnapshotSurface` variants.
- **R10.** (Important #10) `ad_list_windows` accepts `focused_only: bool` or a dedicated `ad_focused_window` fn exists.
- **R11.** (Important #11) FFI exposes `ad_list_notifications`, `ad_dismiss_notification`, `ad_dismiss_all_notifications`, `ad_notification_action`, `ad_find`, `ad_get`, `ad_is`. Remaining gaps (`wait_*`, `batch`, `press_key_for_app`, `get_live_value`) are explicit non-goals for this plan.
- **R12.** (Important #12) `skills/agent-desktop-ffi/SKILL.md` ships with the FFI, covering memory ownership rules, error pattern, thread-safety, and a build / link example.
- **R13.** (Important #13) `ad_window_to_core` validates `id` and `title` non-null; null/invalid-UTF-8 returns `AD_RESULT_ERR_INVALID_ARGS`.
- **R14.** (Important #14) `build.rs` fails loudly on cbindgen failure; cbindgen version is pinned exactly; CI regenerates the header and diffs against the committed copy.
- **R15.** (Important #15) Interior NUL bytes in mandatory string fields are replaced (U+FFFD or `?`) instead of returning null pointers; optional fields retain null-on-NUL.
- **R16.** (Smaller) Remove wildcard `pub use types::*` in `lib.rs`; use explicit re-exports.
- **R17.** (Smaller) Strip inline comments in `tree.rs`; keep only `///` doc-comments on public items.
- **R18.** (Smaller) Every C-visible optional string or sentinel field documents nullability and meaning in the generated header via `///` doc-comments on the Rust types.
- **R19.** (Smaller) Remove all `.unwrap()` / `.expect()` from non-test code in `crates/ffi/`.
- **R20.** (Smaller) Consolidate or remove misleading `#[allow(dead_code)]` annotations; document cdylib false-positive rationale at file scope where still needed.
- **R21.** (Smaller) Re-evaluate `unsafe impl Send + Sync for NativeHandle` now that FFI materializes the cross-thread risk; either enforce single-thread discipline or remove the impl.
- **R22.** (Smaller) `AdImageBuffer.data_len` is not read back from the C-mutable struct at free time.
- **R23.** (Smaller) Compile-time assertion that `ErrorCode::VARIANTS == AdResult::VARIANTS - 1` prevents silent misses when core adds an error variant.
- **R24.** (Nit) `CLAUDE.md` reflects that `crates/ffi/` is now the second platform → core wiring point.
- **R25.** (Plan-added, user request) The FFI crate is restructured to match the project's modular standards before any fixes land: one domain type per file, mirrored subfolder layout (tree/ actions/ input/ system/ equivalents), explicit per-item re-exports, zero inline `//` commentary, `///` doc-comments only where the name is insufficient, every file under 400 LOC, zero `unwrap`/`expect` in non-test code, and no unjustified `#[allow(dead_code)]`. This is the precondition for every later unit.

## Scope Boundaries

- **Phase 1 macOS only.** Windows and Linux adapter stubs exist in `crates/ffi/Cargo.toml`'s `[target.'cfg(...)']` deps but are compile-time only. Main-thread enforcement is macOS-specific (`pthread_main_np`); other platforms get a no-op shim.
- **No ABI stability guarantees.** The header carries a loud comment that the ABI is unstable until v1.0. No `SONAME` versioning, no `install_name` tricks — filename versioning is deferred to the 1.0 release plan.
- **Pre-1.0 remains pre-1.0.** Workspace version stays `0.1.x`. Adding the FFI artifact to the release pipeline is explicitly deferred (see below).

### Deferred to Separate Tasks

- **`ad_wait_*`, `ad_batch`, `ad_press_key_for_app`, `ad_get_live_value` FFI exposure** — follow-up PR after this one lands. Each needs its own API design (wait mode enum, batch JSON schema) and none block the v0 publication.
- **Release pipeline cdylib artifacts** — `libagent_desktop_ffi.{dylib,so,dll}` and `agent_desktop.h` are not currently produced or uploaded by `.github/workflows/release.yml`; adding them is a separate distribution plan (touches `release-please-config.json`, `.github/workflows/release.yml`, `npm/` postinstall, and needs cross-platform build matrix expansion).
- **`docs/solutions/` capture of FFI learnings** — runs after this plan merges via `ce:compound`. Ten candidate topics already identified; writing them is out of scope here.
- **`AdRefEntry` expansion** — core's `RefEntry` has `value`, `states`, `bounds`, `available_actions`, `source_app` fields that the FFI currently omits. They are not required by today's resolve path (resolver only needs `pid` + `role` + `name` + optional `bounds_hash`), so the minimal-but-sufficient shape is what this plan freezes. Expansion is a separate ABI addition if future resolver versions need richer context.
- **Log callback registration** (`ad_set_log_callback`) — foreign-exception propagation hazard documented in flow analysis; deferred to Phase 2 when a callback ABI is designed end-to-end.
- **`ad_version()` / `ad_abi_version()` exports** — useful for dlopen'd consumers but non-blocking for pre-1.0 fixes; bundle with the 1.0 release plan.
- **Cross-language integration tests** — Python ctypes, Go cgo, Swift bridging, Node ffi-napi, glibc-vs-musl matrices. Each is valuable but each is its own multi-day scaffold. This plan adds Rust-side coverage for every finding; cross-language E2E is a separate QA track.

## Context & Research

### Relevant Code and Patterns

FFI crate (all in `crates/ffi/src/`):
- `lib.rs` — `AdAdapter { inner: Box<dyn PlatformAdapter> }` + `build_adapter()` parallel to `src/main.rs:136-154`. Current wildcard `pub use types::*`.
- `types.rs` — 28 `#[repr(C)]` structs + `#[repr(i32)]` enums. `AdTreeOptions` missing `surface`. `AdNativeHandle { ptr }` has no destructor path.
- `error.rs` — thread-local `StoredError` with paired `CString`s. `set_last_error`, `clear_last_error`, C exports. Line 56 has `.unwrap()` on an infallible `CString::new`.
- `convert.rs` — `string_to_c` (returns null on NUL), `c_to_str` (unbound lifetime declaration), `free_*_fields` helpers. File-scoped `#![allow(dead_code)]`.
- `tree.rs` — DFS flatten with broken child-range addressing. Recursive walker at `flatten_recursive` (line 26-67). `ad_get_tree` hardcodes `SnapshotSurface::Window` at line 158.
- `actions.rs` — enum converters, `ad_resolve_element` (leaks `CFRetain`ed element), `ad_execute_action` (no out-param zeroing), `ad_free_action_result`.
- `apps.rs` / `windows.rs` / `surfaces.rs` — `Box::from_raw(slice_from_raw_parts_mut(ptr, count))` with caller-supplied count → heap corruption primitive. `ad_free_window` and `ad_free_windows` naming collision.
- `windows.rs::ad_window_to_core` (line 10-28) — silent wrong-window match via `.unwrap_or("")` coercion of null title.
- `input.rs` — clipboard + mouse + drag. Current `ad_free_string` is only safe for clipboard-returned strings.
- `screenshot.rs` — `AdImageBuffer` C-mutable `data_len` field used at free time → heap corruption if consumer mutates.
- `build.rs` — `.expect()` on cbindgen config, `.ok()` swallows generation errors, writes into `include/agent_desktop.h` on every build.
- `cbindgen.toml` — `style = "both"`, `prefix_with_name = true`, `ScreamingSnakeCase`.
- `include/agent_desktop.h` — 461 LOC, committed; missing nullability / lifetime docs.

Core crate:
- `crates/core/src/adapter.rs:59-92` — `NativeHandle` definition. `unsafe impl Send + Sync` with "Phase 4 rework" comment. No `Drop`.
- `crates/core/src/adapter.rs:15-25` — `SnapshotSurface` (7 variants).
- `crates/core/src/adapter.rs:10-13` — `WindowFilter { focused_only, app }`.
- `crates/core/src/adapter.rs:115-247` — `PlatformAdapter` trait (28 methods). `find_element` / `batch` are NOT trait methods — live in `crates/core/src/commands/{find,batch}.rs` as tree walkers + dispatchers.
- `crates/core/src/notification.rs` — `NotificationInfo { index, app_name, title, body, actions }`, `NotificationFilter { app, text, limit }`.
- `crates/core/src/error.rs` — `ErrorCode` (12 variants, declaration order matches `AdResult = -1..=-12`).
- `crates/core/src/refs.rs` — `RefEntry { pid, role, name, bounds_hash, value, states, bounds, available_actions, source_app }`. Current FFI omits the last five.

macOS crate:
- `crates/macos/src/tree/element.rs:24-39` — `AXElement` with `Drop`+`Clone` doing `CFRelease`/`CFRetain`. Pattern for `NativeHandle` `release_handle`.
- `crates/macos/src/tree/resolve.rs:88` — `CFRetain(el.0)` before wrapping in `NativeHandle::from_ptr`. Defines retain ownership: caller must release exactly once.
- `crates/macos/src/adapter.rs:38-52` — `get_tree` wires each `SnapshotSurface` to a `crates/macos/src/tree/surfaces.rs` function.
- `crates/macos/src/adapter.rs:131-161` — `get_live_value` / `get_element_bounds` use `ManuallyDrop<AXElement>` to borrow without double-release. Pattern to follow.

Binary crate reference:
- `src/main.rs:136-154` — `build_adapter()` that FFI's `lib.rs` parallels; single source of truth pattern the CLAUDE.md "only binary wires platform → core" rule assumes (now violated by FFI).
- `src/batch.rs` parses batch commands through the typed CLI command path.

CI / release:
- `.github/workflows/ci.yml` — Dependency isolation check only guards `agent-desktop-core`. No cdylib build. No header-drift check.
- `.github/workflows/release.yml` — ships CLI tarballs + npm; no cdylib artifact slot.
- `Cargo.toml` release profile — `panic = "abort"`, `lto = true`, `opt-level = "z"`. The `panic = "abort"` makes `catch_unwind` a no-op in release builds unless overridden per-package.

### Institutional Learnings

None. `docs/solutions/` does not yet exist in this repo. This fix cycle is the first time C-ABI concerns touch the codebase and should seed that directory via `ce:compound` after merge.

### External References

Not consulted (the origin review is external-quality grounding and cites every external pattern it recommends: `catch_unwind` + `panic=unwind` per-package override, errno-style thread-local error lifetimes, `pthread_main_np()` assertions, `Box::into_raw`/`Box::from_raw` ownership discipline, cbindgen version pinning for drift-free output).

## Key Technical Decisions

- **Panic strategy: dedicated `[profile.release-ffi]` inheriting from release.** Cargo rejects per-package `panic` overrides (`error: panic may not be specified in a package profile` — verified 2026-04-16). The workaround is a custom profile: `[profile.release-ffi] inherits = "release" panic = "unwind"`. The cdylib is built via `cargo build --profile release-ffi -p agent-desktop-ffi`; the CLI still builds via `cargo build --release` under the unchanged `panic = "abort"` workspace profile. Spike-verified: a panic inside an `extern "C"` fn wrapped in `catch_unwind` returns cleanly under release-ffi (exit 0). CI + release workflow must build the cdylib with the explicit `--profile release-ffi` flag. Size measured: 468 KB cdylib, 1.1 MB CLI — both well under limits. Alternative considered and rejected: drop workspace `panic = "abort"` entirely — the CLI would gain unwind metadata for no user-visible benefit, and the release binary budget is tight (15 MB gate).
- **Error lifetime: keep last-error intact on success paths.** Remove `clear_last_error()` calls from every success path. The last-error slot only rotates when a *new* error is set. Matches the C `errno` contract.
- **Enum validation: `TryFrom<i32>` generated by declarative macro.** A single `try_from_c_enum!(AdActionKind, 0..=20)`-style macro per enum generates the `TryFrom<i32>` impl. Each extern fn reads the field as `i32`, calls `try_into()`, returns `ErrInvalidArgs` on `Err`.
- **Panic boundary: macro, not closure wrapper.** A `#[ffi_entrypoint]` macro (or `ffi_try!` declarative macro) wraps every extern fn body in `catch_unwind` and translates unwinds to `AD_RESULT_ERR_INTERNAL`. Using a macro keeps each function's prose unchanged; a closure wrapper forces every fn to be rewritten.
- **Tree layout: BFS with contiguous sibling runs.** The flat array is emitted level-by-level (breadth-first), so `nodes[child_start..child_start+child_count]` enumerates direct children exactly. Iterative traversal with `VecDeque` (not recursion) also resolves the review's nit about stack depth on thin-stack consumers (JNI, Python ctypes).
- **Opaque list handles.** `AdWindowList`, `AdAppList`, `AdSurfaceList`, `AdNotificationList` each wrap a `Box<[T]>` inside a forward-declared struct. `_count(list)`, `_get(list, i)`, `_free(list)` accessors. Caller never sees the backing pointer or length; count mismatches become impossible by construction.
- **`ad_free_handle` + core trait addition.** `PlatformAdapter` gains `release_handle(&self, handle: &NativeHandle) -> Result<(), AdapterError>` with default `not_supported()` impl. macOS impl calls `CFRelease(ptr as CFTypeRef)`. FFI exports `ad_free_handle(adapter, handle)`. Explicit release (not `Drop`) because `AdNativeHandle { ptr }` is a C struct the consumer owns — Rust has no lifecycle hook into it.
- **Pre-freeze `AdTreeOptions.surface` + `ad_list_windows` focused_only.** Both ABI additions land together as "freeze the struct shape" work. Adding them after publication breaks every consumer compiled against the old layout.
- **Notifications: full CRUD on first release.** All four notification commands become FFI fns in this plan. The review's stance: "ship notifications in this PR" — followed.
- **`find` / `get` / `is`: FFI-side implementation, no core trait additions.** `find` is implemented as an iterative walk over the flat tree inside the FFI. `get` reuses `get_live_value` + `get_element_bounds`. `is` is a state-bitset check on the resolved element. No platform-layer additions needed. Matches existing `commands/find.rs` approach.
- **Main-thread check: macOS `pthread_main_np()` debug_assert.** No-op on other platforms (Windows UIA and Linux AT-SPI don't require main thread). Loud `///` comment in header + skill doc explains the macOS constraint to consumers.
- **Header drift detection: CI regen + `git diff --exit-code`.** `build.rs` continues to copy into `include/agent_desktop.h`. CI runs a fresh `cargo build -p agent-desktop-ffi` and fails if the working tree has any diff afterward. Cbindgen pinned exactly (`cbindgen = "= 0.27.0"`) to eliminate formatting drift across CI images.
- **`NativeHandle` `Send + Sync` stance.** Keep the `unsafe impl` but narrow its justification comment: "Phase 1: handles are single-owner and single-thread by FFI contract. Consumers violating this invoke undefined behavior." Debug assertion in `release_handle` verifies current-thread equals creation-thread.
- **Interior NUL policy.** Mandatory fields (role, name on notifications, app_name, etc.) use lossy replace — NUL byte → U+FFFD UTF-8 sequence. Optional fields (value, description, hint) keep the current null-on-NUL behavior because the header will explicitly document nullability.

## Open Questions

### Resolved During Planning

- **Macro vs runtime wrapper for `catch_unwind`?** Declarative macro `ffi_try!` wrapping the fn body. Chosen over proc-macro attribute to keep build times sane and debugging straightforward.
- **Expand `AdRefEntry` to match `RefEntry`?** No. Minimal-but-sufficient set freezes today's actual resolver input (`pid`, `role`, `name`, `bounds_hash`); the other fields are resolver output metadata the FFI doesn't need yet.
- **Add `ad_focused_window()` or `focused_only` flag?** Flag on `ad_list_windows`. One function, one API surface, no duplicate paths.
- **`AdAction` ergonomics: tagged union vs constructor helpers?** Keep the current variant-struct shape (simpler ABI, cbindgen-friendly) but add `ad_action_click()`, `ad_action_type_text(text)`, `ad_action_scroll(dir, amount)`, `ad_action_press(combo)`, `ad_action_drag(from, to, duration_ms)` constructor helpers that zero-init unused fields. Python / Go consumers no longer have to construct the full union manually. Rust caller-side tests verify each helper produces the expected `AdAction` layout.
- **Generated header: commit or gitignore?** Keep committed. CI regens + diffs. Alternative (OUT_DIR only + separate xtask) adds tooling for marginal benefit.

### Deferred to Implementation

- **Exact macro shape for `ffi_try!`** — final form depends on whether `std::panic::catch_unwind` needs `AssertUnwindSafe` wrappers around captured `adapter: *const AdAdapter` pointers. Work out during Unit 1.
- **Whether `release_handle` needs to tolerate null pointers** — decide when writing macOS impl; likely yes (matches `ad_adapter_destroy` null-tolerance).
- **Whether the compile-time variant-count assertion (R23) needs `variant_count` nightly feature or a manual match-arm count.** Prefer stable-only approach (manual match that compiles to a constant); decide in Unit 12.
- **BFS traversal: recursive build-list-then-emit vs single-pass queue** — both produce correct output; pick based on which is clearer. Benchmark if 10k-node Electron tree builds non-trivially slower.
- **Whether `ad_free_handle` is adapter-scoped or global** — `adapter.release_handle(handle)` means consumer must pass the adapter that created the handle. Mildly awkward but avoids a thread-local registry. Lock in during Unit 7.

## High-Level Technical Design

> *This illustrates the intended approach and is directional guidance for review, not implementation specification. The implementing agent should treat it as context, not code to reproduce.*

**Panic boundary macro shape (Unit 1):**

```
macro_rules! ffi_try { ($body:block) => { {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)) {
        Ok(result_code) => result_code,
        Err(_) => {
            error::set_last_error_static("rust panic in FFI boundary");
            AdResult::ErrInternal
        }
    }
}}}

// Usage per extern fn:
#[no_mangle]
pub unsafe extern "C" fn ad_foo(...) -> AdResult { ffi_try!({
    // existing body unchanged, must end in AdResult
})}
```

**BFS flat-tree layout (Unit 4):**

`root → [a → [a1, a2], b]` becomes:

| index | role | parent_index | child_start | child_count |
|---|---|---|---|---|
| 0 | root | -1 | 1 | 2 |
| 1 | a    |  0 | 3 | 2 |
| 2 | b    |  0 | 5 | 0 |
| 3 | a1   |  1 | 5 | 0 |
| 4 | a2   |  1 | 5 | 0 |

`root.children = nodes[1..3] = [a, b]` ✅
`a.children = nodes[3..5] = [a1, a2]` ✅

Siblings are always contiguous. Child ranges always address *direct* children. The same layout holds for any tree shape.

**Opaque list handle shape (Unit 5):**

```
// Rust side (not exposed to header):
pub struct AdWindowList {
    items: Box<[AdWindowInfo]>,
}

// Header (cbindgen emits forward declaration):
typedef struct AdWindowList AdWindowList;

AdResult ad_list_windows(adapter, app_filter, focused_only, AdWindowList** out);
uint32_t ad_window_list_count(const AdWindowList* list);
const AdWindowInfo* ad_window_list_get(const AdWindowList* list, uint32_t index);
void ad_window_list_free(AdWindowList* list);
```

No caller-supplied `count` → count mismatches impossible. Same shape for `AdAppList`, `AdSurfaceList`, `AdNotificationList`.

## Output Structure

New files introduced by this plan, repo-relative:

    crates/ffi/src/
    ├── ffi_try.rs           # Unit 1: panic boundary macro + static error helper
    ├── enum_validation.rs   # Unit 2: TryFrom<i32> macro + per-enum impls
    ├── notifications.rs     # Unit 8: notification FFI surface
    ├── observation.rs       # Unit 9: ad_find / ad_get / ad_is
    └── action_helpers.rs    # Unit 10: ad_action_click / _type_text / etc.

    crates/ffi/tests/
    └── layout_contract.rs   # Cross-unit: asserts C struct layouts (size/align/offsets)

    crates/core/src/
    (adapter.rs modified for release_handle trait method)

    crates/macos/src/
    (tree/element.rs or new adapter method for release_handle impl)

    skills/agent-desktop-ffi/
    ├── SKILL.md             # Unit 14
    └── references/
        ├── ownership.md     # Who allocates / who frees for every pointer type
        ├── error-handling.md
        └── threading.md

    .github/workflows/
    (ci.yml modified for header-drift check)

This tree is a scope declaration. Implementers may reshape if a better layout emerges — the per-unit `**Files:**` sections remain authoritative.

## Implementation Units

### Phase 0 — Refactor to modular CLAUDE.md-compliant structure

Precondition for every later unit. Lands in a single reviewable PR (or a small stack if size dictates) that **changes no behavior** — only moves, splits, strips commentary, and tightens pub boundaries. Passes `cargo clippy --all-targets -- -D warnings`, `cargo test --lib -p agent-desktop-ffi`, and the cbindgen-generated header diff should be byte-identical or differ only in ordering (no new / removed / renamed symbols).

- [ ] **Unit R: Modular refactor + CLAUDE.md alignment**

**Goal:** Bring `crates/ffi/` to the same subfolder discipline the platform crates use. One domain type per file. No inline comments. Explicit pub boundaries. Zero `unwrap`. No dead-code annotations unless justified. Every file under 400 LOC with headroom for the fixes that follow.

**Requirements:** R25 (+ pre-satisfies the file-level mechanics of R16, R17, R19, R20; those R-rows stay in their later units for the non-mechanical portions — explicit re-exports of *new* types from Unit 5+, inline comments introduced by the BFS rewrite, `unwrap`s introduced during fixes.)

**Dependencies:** None. Must land first.

**Files — target structure after refactor:**

```
crates/ffi/src/
├── lib.rs                       # mod declarations + explicit re-exports only (<80 LOC)
├── adapter.rs                   # AdAdapter wrapper + build_adapter + adapter create/destroy/check_permissions
├── error.rs                     # AdResult + StoredError + last-error surface (no unwrap, CStr fallback)
├── types/
│   ├── mod.rs                   # re-exports only (one pub use per type, no wildcard)
│   ├── point.rs                 # AdPoint
│   ├── rect.rs                  # AdRect
│   ├── native_handle.rs         # AdNativeHandle
│   ├── ref_entry.rs             # AdRefEntry
│   ├── action_kind.rs           # AdActionKind
│   ├── direction.rs             # AdDirection
│   ├── modifier.rs              # AdModifier
│   ├── key_combo.rs             # AdKeyCombo
│   ├── scroll_params.rs         # AdScrollParams
│   ├── drag_params.rs           # AdDragParams
│   ├── action.rs                # AdAction
│   ├── element_state.rs         # AdElementState
│   ├── action_result.rs         # AdActionResult
│   ├── app_info.rs              # AdAppInfo
│   ├── window_info.rs           # AdWindowInfo
│   ├── window_op_kind.rs        # AdWindowOpKind
│   ├── window_op.rs             # AdWindowOp
│   ├── surface_info.rs          # AdSurfaceInfo
│   ├── mouse_button.rs          # AdMouseButton
│   ├── mouse_event_kind.rs      # AdMouseEventKind
│   ├── mouse_event.rs           # AdMouseEvent
│   ├── image_format.rs          # AdImageFormat
│   ├── image_buffer.rs          # AdImageBuffer
│   ├── screenshot_kind.rs       # AdScreenshotKind
│   ├── screenshot_target.rs     # AdScreenshotTarget
│   ├── node.rs                  # AdNode
│   ├── node_tree.rs             # AdNodeTree
│   └── tree_options.rs          # AdTreeOptions
├── convert/
│   ├── mod.rs                   # re-exports
│   ├── string.rs                # c-string helpers (string_to_c, opt_string_to_c, c_to_str, free_c_string)
│   ├── rect.rs                  # rect_to_c
│   ├── window.rs                # window_info_to_c, free_window_info_fields
│   ├── app.rs                   # app_info_to_c, free_app_info_fields
│   └── surface.rs               # surface_info_to_c, free_surface_info_fields
├── tree/
│   ├── mod.rs                   # re-exports
│   ├── flatten.rs               # AccessibilityNode → AdNodeTree
│   ├── get.rs                   # ad_get_tree
│   └── free.rs                  # ad_free_tree
├── actions/
│   ├── mod.rs                   # re-exports
│   ├── conversion.rs            # direction_from_c, key_combo_from_c, action_from_c
│   ├── resolve.rs               # ad_resolve_element
│   ├── execute.rs               # ad_execute_action
│   ├── result.rs                # action_result_to_c, ad_free_action_result
│   └── native_handle.rs         # (Unit 6c) ad_free_handle — placeholder file added empty in Unit R so Unit 6c is a pure insert
├── apps/
│   ├── mod.rs
│   ├── list.rs                  # ad_list_apps + ad_free_apps (pre-Unit 5a; Unit 5a replaces with opaque list)
│   ├── launch.rs                # ad_launch_app
│   └── close.rs                 # ad_close_app
├── windows/
│   ├── mod.rs
│   ├── to_core.rs               # ad_window_to_core helper
│   ├── list.rs                  # ad_list_windows + ad_free_windows (pre-Unit 5b)
│   ├── focus.rs                 # ad_focus_window
│   ├── free_one.rs              # ad_free_window (single-struct — will be renamed to ad_release_window_fields in Unit 5b)
│   └── op.rs                    # ad_window_op
├── input/
│   ├── mod.rs
│   ├── clipboard.rs             # ad_get_clipboard, ad_set_clipboard, ad_clear_clipboard, ad_free_string
│   ├── mouse.rs                 # ad_mouse_event, mouse_button_from_c
│   └── drag.rs                  # ad_drag
├── screenshot/
│   ├── mod.rs
│   ├── capture.rs               # ad_screenshot
│   └── free.rs                  # ad_free_image
└── surfaces/
    ├── mod.rs
    ├── list.rs                  # ad_list_surfaces
    └── free.rs                  # ad_free_surfaces
```

**Migration map (what goes where):**

| Current file | New home |
|---|---|
| `crates/ffi/src/lib.rs` (adapter logic) | `crates/ffi/src/adapter.rs` |
| `crates/ffi/src/lib.rs` (re-exports) | `crates/ffi/src/lib.rs` (kept; wildcards removed, explicit list added) |
| `crates/ffi/src/types.rs` (28 items) | `crates/ffi/src/types/*.rs` (one per struct/enum) |
| `crates/ffi/src/convert.rs` | `crates/ffi/src/convert/{string,rect,window,app,surface}.rs` |
| `crates/ffi/src/tree.rs` | `crates/ffi/src/tree/{flatten,get,free}.rs` |
| `crates/ffi/src/actions.rs` | `crates/ffi/src/actions/{conversion,resolve,execute,result}.rs` |
| `crates/ffi/src/apps.rs` | `crates/ffi/src/apps/{list,launch,close}.rs` |
| `crates/ffi/src/windows.rs` | `crates/ffi/src/windows/{to_core,list,focus,free_one,op}.rs` |
| `crates/ffi/src/input.rs` | `crates/ffi/src/input/{clipboard,mouse,drag}.rs` |
| `crates/ffi/src/screenshot.rs` | `crates/ffi/src/screenshot/{capture,free}.rs` |
| `crates/ffi/src/surfaces.rs` | `crates/ffi/src/surfaces/{list,free}.rs` |

**Approach:**
- Refactor in one atomic commit per subtree (types → convert → tree → actions → apps → windows → input → screenshot → surfaces → adapter). Small, reviewable steps; each commit compiles + passes clippy.
- **No behavior change.** Every FFI symbol keeps the same signature and same module path as seen by C consumers (cbindgen treats Rust module paths as invisible; the generated header stays identical). Run `cargo build -p agent-desktop-ffi` after each sub-commit and `diff include/agent_desktop.h` — must be empty (or ordering-only if cbindgen's topological sort changes).
- **Inline comments stripped.** Every `//` that isn't a `///` doc-comment on a public item is deleted. A `///` doc-comment is retained only when the name alone doesn't communicate intent — e.g., `/// # Safety` blocks on `unsafe extern "C" fn`, lifetime documentation on the `ad_last_error_*` errno-style semantics. Descriptive rehash-of-code comments are slop and go.
- **Explicit re-exports.** `lib.rs` line `pub use types::*` is replaced with:
  ```rust
  pub use types::action::AdAction;
  pub use types::action_kind::AdActionKind;
  pub use types::action_result::AdActionResult;
  // ...one per exported type
  ```
  `types/mod.rs` does the same: `pub mod point; pub mod rect; ...` (each submodule is `pub(crate)` internally; the type name is re-exported via explicit `pub use`).
- **Zero `unwrap` / `expect`** in non-test code. `error.rs:56` `.unwrap()` on the infallible-today `CString::new("(message contained null byte)")` becomes a `'static CStr` literal: `static NUL_BYTE_FALLBACK: &CStr = c"(message contained null byte)";` and `set_last_error` falls back to it via a `MessageSource::Static` variant (or equivalent — the exact shape stays the implementer's call). `build.rs` `.expect()` calls are replaced with graceful handling that still fails the build loudly (see Unit 12 for the full safety pass; Unit R only removes the `.unwrap()`s, doesn't rewrite `build.rs` end-to-end).
- **`#[allow(dead_code)]` audit.** Every annotation in `crates/ffi/src/` is checked against actual reachability from a `#[no_mangle]` export. Reachable functions lose the annotation. Remaining annotations (cdylib false positives on `pub(crate)` helpers called across module boundaries that linker visibility hides from rustc) are consolidated to a single `#![allow(dead_code)]` at the file head with a one-line `// cdylib false-positive: see docs/solutions/cdylib-dead-code.md` explanation (or a `///` doc-comment at crate level if one exists). No sprayed per-item annotations.
- **`c_to_str` unbound lifetime** is fixed in this unit (pre-existing footgun): signature becomes `fn c_to_str(ptr: *const c_char) -> Option<&'_ str>` with an elided-but-bounded lifetime tied to `ptr`, or returns `Option<String>` (always cloned). Pick the cloned-return variant for simplicity; it's only called by short-lived `to_owned()` paths today.
- **Naming normalization.** `ad_free_window` (singular, for the struct from `ad_launch_app`) stays named `ad_free_window` in this unit but the docstring clarifies it's interior-fields-only. Unit 5b renames it to `ad_release_window_fields`. This unit doesn't rename to keep the no-behavior-change invariant intact.
- **cbindgen.toml** left untouched. The header regeneration after refactor must diff to empty (or ordering-only). If cbindgen's output changes because of module-path-based symbol ordering, commit the re-ordered header as part of Unit R's final commit and note it in the PR body.
- **`#[macro_use]` placement** for later `ffi_try!` macro (Unit 2) is left to that unit. Unit R doesn't introduce new macros.

**Patterns to follow:**
- `crates/macos/src/` layout — the direct template for the ffi refactor. Note the `tree/`, `actions/`, `input/`, `system/` subfolders mirror what ffi/ now becomes.
- `crates/macos/src/lib.rs` — mod declarations + explicit re-exports only; no behavior.
- `crates/macos/src/adapter.rs` — single responsibility (PlatformAdapter impl); same shape for `crates/ffi/src/adapter.rs` (AdAdapter wrapper + lifecycle FFI exports).
- `crates/core/src/commands/mod.rs` — one command per file pattern.

**What "slop" means here (the cleanup list):**
- Inline `//` comments that narrate what the next line does (`// free the boxed slice`, `// convert to C`, etc.) — delete.
- Redundant module-level `#[allow(dead_code)]` when items are reachable — delete.
- `TODO`/`FIXME`/`XXX` markers without ticket references — delete or replace with `tracing::warn!` if the note is truly operational.
- Wildcard imports / re-exports — replace with explicit lists.
- Duplicated helper logic between modules (e.g., null-check + early-return boilerplate) — consolidate into a single helper in `convert/string.rs` or `error.rs`.
- `let _ = ...;` for deliberate result-drops — replace with `drop(...)` to be explicit about intent; prefer not silently discarding `Result`s at all where `?`-propagation works.
- Self-documenting obvious code wrapped in comments — delete.
- Multi-line `//!` or `///` docstrings on implementation details that don't cross the FFI boundary — condense to a single sentence or remove.

**Test scenarios:**
- Happy path: `cargo build -p agent-desktop-ffi` compiles clean after refactor
- Happy path: `cargo test --lib -p agent-desktop-ffi` — all 26 existing tests still pass unchanged
- Happy path: `cargo clippy --all-targets -p agent-desktop-ffi -- -D warnings` passes
- Happy path: generated `include/agent_desktop.h` byte-identical (or ordering-only diff) pre- and post-refactor. `git diff include/agent_desktop.h | grep -E '^[+-](typedef|struct|enum|AD_|ad_)'` returns zero lines (or only lines where order changed but no additions/removals)
- Verification: `grep -rn '^//[^/!]' crates/ffi/src/` returns only `// SAFETY:` comments inside `unsafe` blocks (and no `// descriptive-text-about-next-line` comments)
- Verification: `grep -rn '\.unwrap()\|\.expect(' crates/ffi/src/ --include='*.rs' | grep -v '#\[cfg(test)\]'` returns zero hits outside `#[cfg(test)]` blocks
- Verification: `grep -rn 'pub use .*::\*' crates/ffi/src/` returns zero hits
- Verification: `wc -l crates/ffi/src/**/*.rs` — every file under 400 LOC (and most should be under 150, with headroom for Units 1-13 additions)

**Verification:** Refactor commit(s) land on `feat/c-bindings` before any Unit 1+ work begins. The PR description notes the refactor is behavior-neutral. A reviewer who diffs the generated header sees no ABI change.

---

### Phase 1 — Safety Foundation

- [ ] **Unit 0: Publish guard + custom FFI release profile**

**Goal:** Cargo.toml shape-changes land before Unit 1's macro work. Prevent accidental `cargo publish` of the cdylib until the distribution plan lands. Make `release-ffi` profile the canonical build mode for the cdylib.

**Requirements:** R4 (profile prerequisite), scope-guardian P1 (publish guard)

**Dependencies:** None.

**Files:**
- Modify: `Cargo.toml` (add `[profile.release-ffi] inherits = "release" panic = "unwind"`)
- Modify: `crates/ffi/Cargo.toml` (add `[package] publish = false` — prevents accidental cargo-publish; removed when distribution plan lands)
- Modify: `crates/ffi/Cargo.toml` (add `libc = "0.2"` under `[target.'cfg(target_os = "macos")'.dependencies]` for `pthread_main_np` used by Unit 11)

**Approach:**
- Custom profile; CLI release profile unchanged.
- `publish = false` in FFI crate prevents `cargo publish -p agent-desktop-ffi` until the distribution plan explicitly removes it.
- Pre-verified: spike compile of an extern fn with `catch_unwind` under `release-ffi` caught the panic and returned 0; under the `release` profile (panic=abort) the same code aborts.

**Patterns to follow:**
- Existing `[profile.release]` block in `Cargo.toml` lines 22-27.

**Test scenarios:**
- Happy path: `cargo build --profile release-ffi -p agent-desktop-ffi` compiles clean
- Happy path: `cargo build --release` still compiles the CLI under `panic = "abort"` (both profiles coexist)
- Error path: `cargo publish -p agent-desktop-ffi --dry-run` refuses (publish=false)

**Verification:** Both profiles build without errors; `cargo publish --dry-run` on ffi crate returns an error citing publish=false.

---

- [ ] **Unit 1: Panic-unwind cdylib boundary**

**Goal:** Every `extern "C"` fn catches Rust panics and returns `AD_RESULT_ERR_INTERNAL` instead of aborting the host process.

**Requirements:** R4

**Dependencies:** Unit 0 (custom profile must exist).

**Files:**
- Create: `crates/ffi/src/ffi_try.rs` (macro + static last-error helper for panic path)
- Modify: `crates/ffi/src/lib.rs` (declare `mod ffi_try`, re-export `ffi_try!`)
- Modify: `crates/ffi/src/error.rs` (add `set_last_error_static(&'static CStr)` that writes a pointer to a `'static CStr` — never allocates, safe for panic paths)
- Modify: each of the 8 module files containing `#[no_mangle] pub extern "C" fn` exports — wrap every extern body in `ffi_try!`: `lib.rs` (adapter create/destroy, check_permissions), `tree.rs` (get_tree, free_tree), `actions.rs` (resolve_element, execute_action, free_action_result, free_handle), `apps.rs` (list_apps, launch_app, close_app + new opaque list accessors), `windows.rs` (list_windows, focus_window, window_op + new opaque list accessors), `input.rs` (clipboard, mouse, drag, free_string), `screenshot.rs` (screenshot, free_image), `surfaces.rs` (list_surfaces + new opaque list accessors)
- Test: `crates/ffi/src/ffi_try.rs` (unit tests) + `crates/ffi/tests/panic_boundary.rs` (integration; reuses the spike example)
- Promote: `crates/ffi/examples/panic_spike.rs` (from Unit 0 verification) stays as a permanent regression example showing `catch_unwind` working under `release-ffi`

**Approach:**
- Declarative macro `ffi_try!` wraps a block in `catch_unwind(AssertUnwindSafe(|| { $body }))`.
- On panic: set last-error to a `'static CStr` `"rust panic in FFI boundary"` via pointer-to-static (never allocates — allocation inside the panic handler could double-panic). Payload is discarded intentionally; no panic-sourced data reaches the FFI error surface.
- Custom profile `release-ffi` is the only way `catch_unwind` actually catches in optimized builds; workspace profile stays `abort` for the CLI binary.
- Two-phase guard: `ad_adapter_destroy` and all `ad_free_*` fns use the macro too; panicking inside `Drop` during destroy must not escape.
- Transitive-dep audit: verify no `crates/ffi` dependency registers a callback invoked from foreign-C frames (CF run loops, ObjC exception bridging via `objc2`, Unix signal handlers). If any such path exists, `catch_unwind`-at-entry is insufficient and the plan must add inner catch_unwind around the callback. Document the audit result in Unit 1's verification.

**Patterns to follow:**
- No existing catch_unwind pattern in workspace — this unit establishes it.
- Mirror the `#[no_mangle] pub unsafe extern "C" fn` signature pattern that every current FFI export uses; the macro sits inside the fn body, not decorating the signature.

**Test scenarios:**
- Happy path: `ffi_try!` returning `AdResult::Ok` propagates correctly
- Happy path: `ffi_try!` returning `AdResult::ErrInvalidArgs` propagates correctly with last-error already set by fn body
- Error path: `ffi_try!` body calls `panic!("synthetic panic")` → return value is `AD_RESULT_ERR_INTERNAL`, `ad_last_error_message()` returns `"rust panic in FFI boundary"`
- Error path: `ffi_try!` body triggers arithmetic overflow in debug → same translation
- Error path: two sequential panicking calls do not leak or double-set last-error
- Integration: release build with the per-package profile override actually catches panic (not aborts); integration test uses `std::process::Command` to run a test binary that panics inside an extern fn, asserts exit code 0 (caught) not 134/SIGABRT
- Integration: debug build also catches (workspace default `panic = "unwind"` in debug)

**Verification:** Cargo `release` profile show-config for `agent-desktop-ffi` reports `panic = "unwind"`; panic test binary exits with `AD_RESULT_ERR_INTERNAL` in both debug and release.

---

- [ ] **Unit 2: Enum validation at C boundary**

**Goal:** No invalid `#[repr(i32)]` discriminant from C ever reaches Rust enum code; all out-of-range values return `AD_RESULT_ERR_INVALID_ARGS`.

**Requirements:** R3

**Dependencies:** Unit 1 (panic boundary already catches any miss, but explicit validation is safer and diagnoses better).

**Files:**
- Create: `crates/ffi/src/enum_validation.rs` (declarative macro generating `TryFrom<i32>` + variant-range check for every `#[repr(i32)]` C enum)
- Modify: `crates/ffi/src/types.rs` (apply macro invocations to `AdActionKind`, `AdDirection`, `AdModifier`, `AdMouseButton`, `AdMouseEventKind`, `AdWindowOpKind`, `AdScreenshotKind`, `AdImageFormat` — 8 enums)
- Modify: `crates/ffi/src/actions.rs`, `windows.rs`, `input.rs`, `screenshot.rs`, `tree.rs` (all FFI fns that read enum fields from C structs)
- Test: `crates/ffi/src/enum_validation.rs` (unit tests)

**Approach:**
- Every enum-typed field on a C struct is read as `i32`, then `try_into()` or macro-generated `AdActionKind::from_c(raw: i32)` returning `Option<AdActionKind>`.
- On `None`: `error::set_last_error(AdapterError::new(ErrorCode::InvalidArgs, "unknown X enum value: N"))`, return `AD_RESULT_ERR_INVALID_ARGS`.
- Preserve the existing exhaustive `match` arms in each fn body — they run only after validation succeeds.

**Patterns to follow:**
- Existing `mouse_button_from_c` / `direction_from_c` in `crates/ffi/src/{input,actions}.rs` — these are the functions to reshape (they currently trust the enum, fix is to validate first).

**Test scenarios:**
- Happy path: valid discriminant for each of the 8 enums round-trips correctly
- Edge case: first valid variant (0) and last valid variant (enum's max) accepted
- Error path: discriminant = -1 returns `ErrInvalidArgs` with diagnostic last-error
- Error path: discriminant = 999 returns `ErrInvalidArgs`
- Error path: discriminant = `i32::MAX` returns `ErrInvalidArgs`
- Integration: calling `ad_execute_action` with `AdAction.kind = 999` returns `ErrInvalidArgs`, doesn't reach the match arms, doesn't UB

**Verification:** Fuzz test (in-Rust, not C) that iterates `-100..=100` through each enum slot and asserts no panic, no UB marker (Miri-clean), correct return code.

---

- [ ] **Unit 3: Errno-style last-error lifetime**

**Goal:** `ad_last_error_{code,message,suggestion,platform_detail}` pointers remain valid across every subsequent successful FFI call; only the next *failing* call overwrites.

**Requirements:** R2

**Dependencies:** Unit 1 (panic-path sets static last-error; that code must be aware of the new lifetime contract).

**Files:**
- Modify: `crates/ffi/src/error.rs` (remove `clear_last_error` from success paths by no longer providing it; consolidate to `set_last_error` replacement on failure)
- Modify: every FFI fn in `crates/ffi/src/{apps,windows,tree,actions,input,screenshot,surfaces,lib}.rs` that currently calls `clear_last_error()` on success — delete those calls
- Modify: `crates/ffi/include/agent_desktop.h` (via rustdoc on `ad_last_error_*` functions — document lifetime semantics)
- Test: `crates/ffi/src/error.rs` + new integration test `crates/ffi/tests/error_lifetime.rs`

**Approach:**
- Current behavior: every successful FFI call drops the thread-local `StoredError`, invalidating pointers consumer may still hold → UAF.
- New behavior: successful calls leave the last-error slot untouched. Consumers read last-error after a failure and can keep reading the same pointers across any number of successful calls until a new failure occurs.
- Matches POSIX `errno` semantics exactly.
- `clear_last_error` stays as a private helper usable only when explicitly starting a new error chain (in practice: only called by `ad_adapter_create` to start fresh).

**Patterns to follow:**
- POSIX errno (`<errno.h>`) — the canonical model. Every library exposing errno-like last-error follows this contract.

**Test scenarios:**
- Happy path: failing call sets last-error; pointer returned by `ad_last_error_message()` is readable
- Happy path: after ten successful calls following a failure, `ad_last_error_message()` returns the same non-dangling string
- Happy path: a second failing call overwrites the previous last-error; the old pointer is no longer guaranteed valid
- Edge case: thread-local isolation — Thread A's last-error is not visible to Thread B (existing behavior, confirm preserved)
- Edge case: panic path (from Unit 1) sets last-error to static string; subsequent successful calls don't invalidate the static pointer
- Integration: C harness that reproduces the UAF pattern (`ad_foo` fails → read message → call `ad_check_permissions` succeeds → use cached message pointer) — must not segfault or read garbage

**Verification:** Integration test simulates the UAF pattern from the review and passes under Miri / ASan.

### Phase 2 — Tree Correctness

- [ ] **Unit 4: BFS flat-tree layout with iterative traversal**

**Goal:** `AdNode.child_start..child_start + child_count` over `AdNodeTree.nodes` returns direct children only — no grandchildren leak, no siblings skipped. Handles trees of arbitrary depth without Rust stack pressure from consumers on thin stacks (JNI ~256KB, Python ctypes worker threads).

**Requirements:** R1

**Dependencies:** None (tree.rs is self-contained; depends only on established panic boundary from Unit 1 and the `try_into` enum pattern from Unit 2 if surface enum is involved — but tree.rs doesn't consume a surface enum today).

**Files:**
- Modify: `crates/ffi/src/tree.rs` (rewrite `flatten_tree` + `flatten_recursive` as iterative BFS; update `AdNode` field semantics to BFS layout)
- Modify: rename test `test_flatten_depth_first_order` → `test_flatten_breadth_first_layout`; update to iterate actual child ranges (`nodes[child_start..child_start+child_count]`) and assert direct-child roles (not just field values); add multi-level nested test and wide-shallow test
- (Inline-comment strip in rewritten tree.rs is handled in Unit 13 alongside the other doc-cleanup work — avoids duplication)
- Test: `crates/ffi/src/tree.rs` (unit tests)

**Approach:**
- Single pass: walk the source tree with a `VecDeque<(AccessibilityNode, parent_index)>`. Emit each node; defer children; after all nodes from the current level are emitted, process the next level.
- Each emission records `(own_index, parent_index)`; at end of each parent's child-emission batch, patch the parent's `child_start` and `child_count`.
- Pre-count nodes in a first pass for `Vec::with_capacity` — avoids the 14-realloc issue on a 10k-node tree. Alternative: single-pass with capacity hint (`count_nodes_for_capacity`) to avoid two passes.
- No recursion → no FFI-layer depth guard needed; we inherit `ABSOLUTE_MAX_DEPTH=50` from the macOS adapter.

**Patterns to follow:**
- Standard BFS queue pattern. No workspace precedent — this unit establishes it.

**Test scenarios:**
- Happy path: single root with no children → single-node tree, child_count=0
- Happy path: root with three flat children → root.children returns all three, each child has child_count=0
- Happy path: review's failing case `root → [a → [a1, a2], b]` → iterating root's children yields `[a, b]` (NOT `[a, a1]`), iterating a's children yields `[a1, a2]`, iterating b's children yields `[]`
- Happy path: deeply nested tree (10 levels, one child per level) — child ranges walk correctly at every level
- Happy path: wide tree (one root with 1000 direct children) — all 1000 accessible, no reallocation observed (tracked via capacity test)
- Edge case: empty tree via `AccessibilityNode` with zero children produces single-node array, `ad_free_tree` handles the zero-child root correctly
- Edge case: tree emitted at full `max_depth` pruning boundary — remaining children should have `child_count = 0` (or the current pruning semantics preserved), not dangling ranges

**Verification:** All existing tree tests pass. New test `test_direct_child_iteration_returns_only_direct_children` iterates `nodes[child_start..child_start+child_count]` at every level and asserts exact role sequence.

### Phase 3 — ABI-Locked Shape Changes

- [ ] **Unit 5: Opaque list handles + image buffer length encapsulation**

**Goal:** Replace every `(*mut T, count)` list-returning API with an opaque handle. Caller never sees the backing allocation. `AdImageBuffer` tracks allocation length internally, not in a C-mutable field.

**Requirements:** R8, R22

**Dependencies:** Unit 1 (panic boundary wraps new fns), Unit 2 (if any enum validation applies).

**Files:**
- Modify: `crates/ffi/src/types.rs` (add forward-declared `AdWindowList`, `AdAppList`, `AdSurfaceList`, `AdNotificationList` — private structs with `Box<[T]>` inner; make `AdImageBuffer.data_len` private by replacing with an opaque wrapper struct)
- Rewrite: `crates/ffi/src/apps.rs` (`ad_list_apps` returns `AdAppList*`; new `ad_app_list_count`, `ad_app_list_get`, `ad_app_list_free`; remove `ad_free_apps`)
- Rewrite: `crates/ffi/src/windows.rs` (same pattern for `AdWindowList`; also see Unit 6 for `focused_only` addition)
- Rewrite: `crates/ffi/src/surfaces.rs` (same pattern for `AdSurfaceList`)
- Rewrite: `crates/ffi/src/screenshot.rs` (`AdImageBuffer` now opaque or embeds a private length field not read back from C)
- Rename: existing `ad_free_window` → `ad_release_window_fields` (interior-string free for the single `AdWindowInfo` from `ad_launch_app`) to disambiguate from the list case
- Test: `crates/ffi/src/{apps,windows,surfaces,screenshot}.rs` unit tests + new `crates/ffi/tests/opaque_lists.rs`

**Approach:**
- `AdWindowList`, etc. are forward-declared in the header via cbindgen's opaque-struct support. Rust defines them as `pub struct AdWindowList { items: Box<[AdWindowInfo]> }` — no `#[repr(C)]`, intentionally no layout guarantee.
- Accessors: `_count(list)` returns `items.len() as u32`; `_get(list, i)` returns `items.get(i as usize).map_or(null(), |w| w as *const _)`; `_free(list)` drops the `Box`.
- `ad_app_list_get` returning null on out-of-bounds is explicitly documented; caller must check.
- `AdImageBuffer`: keep the struct for cbindgen compat but store the allocation length in a shadow thread-local map keyed on `data` pointer, OR wrap in a private `Box` allocated structure where `ad_screenshot` returns `AdImageBuffer*` (opaque) with accessors `_data()`, `_size()`, `_width()`, `_height()`, `_format()`. Second approach is cleaner; use it.
- Rename breaks existing API: `ad_free_window` naming collision with `ad_free_windows` caused the review's heap-corruption hazard. Rename to `ad_release_window_fields` and document that it's for the single struct from `ad_launch_app`.

**Patterns to follow:**
- Rust `Box::into_raw` / `Box::from_raw` for ownership transfer across the FFI boundary.
- Cbindgen opaque struct declaration: the Rust struct carries no `#[repr(C)]`; cbindgen emits a forward `typedef struct AdWindowList AdWindowList;`.

**Test scenarios:**
- Happy path: `ad_list_apps` returns non-null list; `ad_app_list_count` matches expected; iterating via `_get` returns each app
- Happy path: empty list from `ad_list_apps` (no apps running in test env) — list is non-null, count is 0, `_get(0)` returns null
- Happy path: `ad_list_windows(adapter, app_filter, focused_only=false, &list)` populates list; out-of-bounds `_get(count)` returns null
- Edge case: `_free(null)` is a no-op (matches review's current free-fn tolerance pattern)
- Edge case: double-free under Miri produces expected error (or documented caller responsibility — pick; I recommend double-free is UB and the skill doc says so)
- Error path: caller-supplied adapter is null → `ErrInvalidArgs`
- Integration: assert that struct layout of `AdWindowList` is NOT exposed in the header — grep the generated `agent_desktop.h` for `AdWindowList` should find only `typedef struct AdWindowList AdWindowList;` and function signatures, no field declarations

**Verification:** Header review shows only opaque typedefs for the four list types; no count-mismatch API path exists.

---

- [ ] **Unit 6: ABI surface completion — surface / focused_only / RefEntry / handle**

**Goal:** Every ABI addition that would otherwise be a breaking change post-publication lands in this unit. After this, the C struct layouts are frozen.

**Requirements:** R9, R10, R5

**Dependencies:** Unit 2 (surface enum validation), Unit 5 (focused_only lands on the reshaped `ad_list_windows`).

**Files:**
- Modify: `crates/ffi/src/types.rs`:
  - Add `AdSnapshotSurface` enum (`Window=0, Focused, Menu, Menubar, Sheet, Popover, Alert`)
  - Add `surface: AdSnapshotSurface` field to `AdTreeOptions` (default=0=Window preserves parity with the current hardcoded `SnapshotSurface::Window` at `crates/ffi/src/tree.rs:158`; since the header is unpublished, this is a pre-freeze shape correction, not an ABI-compatible addition)
- Modify: `crates/ffi/src/tree.rs` (`ad_get_tree` maps surface enum variants to `SnapshotSurface` core variants; no longer hardcodes Window)
- Modify: `crates/ffi/src/windows.rs` (`ad_list_windows` signature adds `focused_only: bool` parameter; `WindowFilter` construction uses the new value)
- Modify: `crates/core/src/adapter.rs` (add trait method `release_handle(&self, handle: &NativeHandle) -> Result<(), AdapterError>` with `not_supported()` default)
- Modify: `crates/macos/src/adapter.rs` (impl `release_handle`: `CFRelease(handle.as_raw() as CFTypeRef)`; handle nulls safely)
- Create or modify: `crates/ffi/src/actions.rs` — add `ad_free_handle(adapter: *const AdAdapter, handle: *const AdNativeHandle) -> AdResult`
- Modify: `crates/ffi/include/agent_desktop.h` — regenerated with new signatures
- Test: `crates/ffi/src/{tree,windows,actions}.rs` unit tests; integration test `crates/ffi/tests/surfaces_and_focused.rs`

**Approach:**
- `AdSnapshotSurface`: straightforward `#[repr(i32)]` enum mapping 1:1 to `SnapshotSurface`.
- `focused_only` on `ad_list_windows`: second-last parameter (before `out`). Matches `WindowFilter::focused_only`.
- `release_handle` in core: the first handle-releasing trait method. Default `not_supported()` preserves backward compat with Windows/Linux stubs.
- macOS impl: carefully calls `CFRelease` (not `CFRetain(… - 1)`), follows `Drop` pattern in `crates/macos/src/tree/element.rs:24-39`.
- `ad_free_handle`: takes adapter + handle by pointer. Both nulls tolerated (matches `ad_adapter_destroy`). Calls adapter's `release_handle` under `ffi_try!`.
- Windows/Linux stubs at `crates/windows/src/lib.rs` and `crates/linux/src/lib.rs` require no modification — they inherit the `not_supported()` default for `release_handle`.

**Patterns to follow:**
- `AXElement::Drop` in `crates/macos/src/tree/element.rs` — reference for correct `CFRelease` discipline.
- `SnapshotSurface` match in `crates/macos/src/adapter.rs:38-52` — the mapping the FFI layer mirrors.
- Adapter default-impl pattern elsewhere in `crates/core/src/adapter.rs` — every new trait method ships with `not_supported()` default.

**Test scenarios:**
- Happy path: `ad_get_tree` with each of the seven `AdSnapshotSurface` variants succeeds against an app that exposes the corresponding surface
- Happy path: `ad_list_windows(adapter, null_filter, focused_only=true, &list)` returns at most one window (the focused one)
- Happy path: `ad_list_windows(adapter, null_filter, focused_only=false, &list)` returns all windows
- Happy path: `ad_resolve_element` → `ad_free_handle` → repeat 1000 times → no Core Foundation retain count leak (measured via Activity Monitor or `vm_stat` in the test harness; also verify via Leaks instrument in a separate validation run)
- Edge case: `ad_free_handle(null_adapter, _)` returns `ErrInvalidArgs`
- Edge case: `ad_free_handle(adapter, null_handle)` is a no-op returning `Ok`
- Edge case: surface enum out-of-range is caught by Unit 2's validation → `ErrInvalidArgs`
- Error path: platform with `not_supported()` default for `release_handle` returns `ActionNotSupported` — windows/linux stubs; macOS never hits this

**Verification:** Run a 10-minute soak test resolving + freeing handles in a loop; `leaks --atExit` reports zero `AXUIElement` retains accumulated from agent-desktop allocations. Generated header contains `AdSnapshotSurface` enum with all seven variants and `focused_only` parameter on `ad_list_windows`.

### Phase 4 — Input Hygiene

- [ ] **Unit 7: Out-param zeroing, string NUL handling, window validation**

**Goal:** Every fallible FFI fn zeroes out-params before any fallible work. Mandatory string fields never return null due to interior NUL. Window targeting by title never silently picks the wrong window.

**Requirements:** R6, R13, R15

**Dependencies:** Unit 1 (panic boundary), Unit 5 (new opaque list signatures).

**Files:**
- Modify: `crates/ffi/src/{actions,apps,screenshot,tree,windows}.rs` — audit every fn with out-params, add zeroing at entry (some already have it: `ad_get_tree`, `ad_list_windows` under the new list shape; missing in: `ad_launch_app`, `ad_execute_action`, `ad_resolve_element`, `ad_screenshot`)
- Modify: `crates/ffi/src/convert.rs` — create two helper functions: `string_to_c_lossy(&str) -> *mut c_char` (mandatory fields: replace interior NUL with U+FFFD before `CString::new`; infallible after replacement) and `opt_string_to_c(&Option<String>) -> *const c_char` (optional fields: returns null on `None` OR on interior NUL). Apply `string_to_c_lossy` to all mandatory string fields in `actions.rs`, `windows.rs`, `apps.rs`, `tree.rs`, `surfaces.rs`, `notifications.rs` (from Unit 8); apply `opt_string_to_c` to optional ones (description, hint, value, bundle_id, etc.)
- Modify: `crates/ffi/src/windows.rs::ad_window_to_core` — null-check `w.id` and `w.title`; return `AdapterError::new(InvalidArgs, "window id or title is null")` on null
- Test: `crates/ffi/src/{convert,windows}.rs` + new `crates/ffi/tests/nul_handling.rs`

**Approach:**
- Zeroing: `unsafe { *out = std::mem::zeroed(); }` immediately after pointer validation, before any call that could fail.
- NUL replacement: one pass replacing `\0` → U+FFFD bytes (3-byte UTF-8 sequence `0xEF 0xBF 0xBD`) in the string, then `CString::new` becomes infallible. Allocates a new `String` only when a NUL is present (fast path: no allocation if source has no NUL).
- `ad_window_to_core` validation: matches the defensive-validation pattern in `crates/ffi/src/apps.rs:70-77` (null-checked `c_to_str` with early return).

**Patterns to follow:**
- Existing `c_to_str` null-check in `apps.rs` `ad_launch_app` line 69-78.
- `ad_get_tree` current zeroing pattern at `tree.rs:40-42`.

**Test scenarios:**
- Happy path: `ad_execute_action` succeeds, out-param populated correctly
- Error path: `ad_execute_action` with invalid action kind → out-param is zeroed, caller calling `ad_free_action_result` on it is a no-op (inner pointers all null)
- Error path: `ad_resolve_element` with stale entry → `AdNativeHandle.ptr = null` (not garbage)
- Edge case: string with interior NUL at position 5 of 20 → mandatory field preserves first 5 chars, replaces NUL with U+FFFD, keeps remaining 14 chars
- Edge case: Chromium string like `"foo\0\0bar"` → mandatory field returns `"foo\u{FFFD}\u{FFFD}bar"`
- Edge case: optional field with interior NUL → returns null pointer (unchanged behavior)
- Error path: `ad_get_tree(adapter, window_with_null_title, opts, out)` → `ErrInvalidArgs`, no "match empty string" behavior
- Error path: `ad_launch_app` with non-UTF-8 id bytes → `ErrInvalidArgs`

**Verification:** Miri-clean on all tests; consumer stack-allocating a struct and passing it into a failing FFI fn can then call the paired `_free` safely.

### Phase 5 — Coverage Parity

- [ ] **Unit 8: Notification FFI surface**

**Goal:** All four notification commands exposed via the C ABI.

**Requirements:** R11 (notifications portion)

**Dependencies:** Unit 5 (opaque list shape — notifications list reuses it as `AdNotificationList`).

**Files:**
- Create: `crates/ffi/src/notifications.rs` (new module — ~250 LOC estimate)
- Modify: `crates/ffi/src/types.rs` (add `AdNotificationInfo`, `AdNotificationFilter`, opaque `AdNotificationList`)
- Modify: `crates/ffi/src/convert.rs` (add `notification_info_to_c`, `free_notification_info_fields`)
- Modify: `crates/ffi/src/lib.rs` (`mod notifications`)
- Test: `crates/ffi/src/notifications.rs` (unit tests)

**Approach:**
- Exports: `ad_list_notifications(adapter, filter, &list)`, `ad_dismiss_notification(adapter, index, app_filter)`, `ad_dismiss_all_notifications(adapter, app_filter, &dismissed_list, &failed_list)`, `ad_notification_action(adapter, index, action_name, &result)`.
- Index stability contract: documented in header — dismiss-by-index is valid only against the *most recent* list call; adapter re-queries internally.
- `dismiss_all` surfaces both dismissed and failed as separate opaque lists (`AdNotificationList` + `AdStringList` for failure reasons). Failed dismissals do not overwrite last-error if any succeeded; caller inspects `failed_list` instead.

**Patterns to follow:**
- `crates/core/src/commands/{list_notifications,dismiss_notification,dismiss_all_notifications,notification_action}.rs` — the command-layer logic; FFI is a thin wrapper.
- Opaque list pattern from Unit 5.

**Test scenarios:**
- Happy path: `ad_list_notifications(adapter, null_filter, &list)` returns notification list on a system with notifications present
- Happy path: `ad_dismiss_notification(adapter, 0, null)` dismisses the first notification; subsequent list returns one fewer
- Happy path: `ad_notification_action(adapter, 0, "Reply", &result)` triggers the Reply button
- Edge case: filter by app name `"Slack"` returns only Slack notifications
- Edge case: filter by text `"deploy"` returns only matching notifications
- Edge case: `dismiss_all` with partial failure — `dismissed_list` non-empty, `failed_list` non-empty
- Error path: `ad_dismiss_notification` with out-of-range index → `ErrNotificationNotFound` (error code -11, which already exists in `AdResult`)
- Error path: `ad_notification_action` with unknown action name → `ErrActionNotSupported`

**Verification:** FFI consumer can replicate every `agent-desktop list-notifications` / `dismiss-notification` / `notification-action` CLI flow.

---

- [ ] **Unit 9: Observation primitives — find / get / is**

**Goal:** Cheap single-element lookups without building a full snapshot tree.

**Requirements:** R11 (find/get/is portion)

**Dependencies:** Unit 4 (BFS tree — `ad_find` walks the tree), Unit 6 (`ad_free_handle` — `ad_find` returns a handle the caller must release).

**Files:**
- Create: `crates/ffi/src/observation.rs`
- Modify: `crates/ffi/src/types.rs` (add `AdFindQuery { role, name_substring, value_substring }`)
- Modify: `crates/ffi/src/lib.rs` (`mod observation`)
- Test: `crates/ffi/src/observation.rs` + `crates/ffi/tests/observation.rs`

**Approach:**
- `ad_find(adapter, app, query, &handle)`: walks `get_tree` result, finds first match, resolves to `NativeHandle` via `resolve_element`, returns `AdNativeHandle` in out-param. Caller must `ad_free_handle`.
- `ad_get(adapter, handle, property, &str_out)`: property is an enum or magic string (`"value"`, `"bounds"`, `"role"`, `"name"`). Dispatches to `get_live_value` / `get_element_bounds` / etc. Returns allocated string via `ad_free_string`.
- `ad_is(adapter, handle, property, &bool_out)`: boolean state check. `"focused"`, `"enabled"`, `"selected"`, `"checked"`, `"expanded"`. Reads from resolved element's state set.

**Patterns to follow:**
- `crates/core/src/commands/find.rs` — reference tree-walk logic (FFI re-implements rather than calling core's command layer to avoid a JSON round-trip).
- `crates/core/src/commands/get.rs` and `is.rs` — reference for property dispatch.

**Test scenarios:**
- Happy path: `ad_find(adapter, "Finder", { role: "button", name_substring: "OK" }, &h)` returns a handle to the first matching button
- Happy path: `ad_get(adapter, h, "value", &s)` returns the element's current value
- Happy path: `ad_is(adapter, h, "focused", &b)` returns true for the currently focused element
- Edge case: `ad_find` with no matches returns `ElementNotFound` and out-handle is zeroed
- Edge case: `ad_get` for unknown property returns `InvalidArgs`
- Error path: stale handle (UI repainted) returns `StaleRef` on `ad_get`
- Integration: full flow `ad_find → ad_get → ad_is → ad_free_handle` with no leaks (soak test, same as Unit 6)

**Verification:** FFI consumer can do `find → is(focused) → if yes then get(value)` without a single `ad_get_tree` call.

---

- [ ] **Unit 10: Action constructor helpers**

**Goal:** Consumers building `AdAction` from Python / Go / Swift no longer zero-init unused variant fields.

**Requirements:** R11 (ergonomics sub-point surfaced by flow analysis)

**Dependencies:** Unit 1 (every `extern "C"` constructor helper must go through the `ffi_try!` panic boundary — even pure-value constructors can panic on OOM).

**Files:**
- Create: `crates/ffi/src/action_helpers.rs`
- Modify: `crates/ffi/src/lib.rs` (`mod action_helpers`)
- Test: `crates/ffi/src/action_helpers.rs`

**Approach:**
- Per-variant constructor: `ad_action_click() -> AdAction`, `ad_action_double_click() -> AdAction`, `ad_action_type_text(text: *const c_char) -> AdAction`, `ad_action_scroll(dir: AdDirection, amount: u32) -> AdAction`, `ad_action_press(combo: *const AdKeyCombo) -> AdAction`, etc.
- Each returns a fully-zeroed `AdAction` with only the relevant variant-fields populated.
- `text`-carrying actions borrow the pointer; caller retains ownership of the backing string during `ad_execute_action`.
- No new `ad_free_action` helper — `AdAction` is a value type, dropped when it goes out of scope on the C caller's stack.

**Patterns to follow:**
- Existing `mouse_button_from_c` / `direction_from_c` — direct conversion helpers.
- `#[no_mangle] pub extern "C" fn` returning a value-type struct is unusual for FFI but widely supported; cbindgen handles it.

**Test scenarios:**
- Happy path: each helper produces an `AdAction` whose relevant fields are set and irrelevant variant fields are zeroed
- Happy path: `ad_action_type_text("hello")` produces an action that, passed to `ad_execute_action`, types "hello"
- Edge case: `ad_action_type_text(null)` returns an `AdAction` with kind `TypeText` and a null text pointer; `ad_execute_action` then returns `InvalidArgs` (no segfault)
- Integration: round-trip in Rust — construct via helper, pass to `action_from_c`, verify core `Action` matches expectation

**Verification:** Consumer can write `ad_execute_action(adapter, handle, &ad_action_click(), &result)` as a one-liner.

### Phase 6 — Production Readiness

- [ ] **Unit 11: macOS main-thread enforcement and NativeHandle review**

**Goal:** Debug builds assert that every macOS-sensitive FFI call happens on the main thread. The `NativeHandle` thread-safety justification is narrowed to match reality.

**Requirements:** R7, R21

**Dependencies:** Unit 1 (macro wrapper is the natural insertion point for per-fn assertion).

**Files:**
- Create: `crates/ffi/src/main_thread.rs` (shim: `debug_assert_main_thread()` — macOS calls `pthread_main_np()`, other platforms no-op)
- Create: the assertion helper lives in `crates/ffi/src/main_thread.rs` (new file, already listed above) and exports `ffi_macos_main_thread!` — a macro that composes `ffi_try!` with a prepended `debug_assert!(is_main_thread(), ...)`. The macro is applied only to macOS-sensitive fns: clipboard, window ops, actions, screenshot. Not applied to: `ad_adapter_create`, `ad_adapter_destroy`, `ad_last_error_*`, `ad_free_*`, `ad_action_*` constructors. The plain `ffi_try!` remains the default; `ffi_macos_main_thread!` is a composed variant, not a replacement.
- Modify: `crates/core/src/adapter.rs` — narrow the `unsafe impl Send + Sync for NativeHandle` comment to current-reality-accurate justification
- Modify: `crates/ffi/include/agent_desktop.h` — add prominent warning block about main-thread requirement (via `///` doc-comment on `ad_adapter_create` or a top-level `//!` header comment)
- Modify: `crates/ffi/src/lib.rs` — crate-level `//!` rustdoc covering thread-safety model
- Test: `crates/ffi/tests/main_thread.rs`

**Approach:**
- `pthread_main_np()` available via `libc::pthread_main_np()` (macOS only). Returns non-zero if current is main.
- `debug_assert!(is_main_thread(), "must be called on main thread");` — panics in debug, no-op in release. Panic → caught by Unit 1 boundary → returns `ErrInternal` with message "called off main thread". Safer than UB in release builds.
- Non-macOS: `fn is_main_thread() -> bool { true }` (Windows UIA and Linux AT-SPI have no main-thread restriction; the check is informational only).
- Retain `unsafe impl Send + Sync` on `NativeHandle` — the FFI contract makes the consumer responsible. Update the comment from "Phase 1 single-threaded CLI" to "FFI contract: single-owner, single-thread. Consumer synchronizes. Violations are UB."

**Patterns to follow:**
- `libc::pthread_main_np` — standard macOS idiom. No workspace precedent.

**Test scenarios:**
- Happy path: main-thread calls succeed in debug build
- Error path: spawned thread calls `ad_get_clipboard` in debug build → assertion fires → caught by panic boundary → returns `ErrInternal` with "must be called on main thread" last-error
- Edge case: release build does not perform the check (debug_assert is removed) — worker-thread call proceeds and may silently UB on macOS; documented loudly in the header
- Edge case: non-macOS builds skip the check entirely and succeed on any thread
- Integration: `ad_adapter_create` / `ad_adapter_destroy` / `ad_free_*` do NOT enforce the check (they're safe off-thread)

**Verification:** `pthread_main_np` call sites confined to debug-only path. Header has a "⚠ THREAD SAFETY" block near the adapter-creation functions.

---

- [ ] **Unit 12: Build hygiene — cbindgen pinning, header drift, dead_code, unwrap removal**

**Goal:** Build tooling fails loudly when cbindgen breaks. Generated header cannot silently drift. Zero `.unwrap()` / `.expect()` in non-test code. Compile-time assertion guards `ErrorCode`/`AdResult` parity.

**Requirements:** R14, R19, R20, R23

**Dependencies:** None.

**Files:**
- Modify: `crates/ffi/Cargo.toml` — `cbindgen = "= 0.27.0"` (exact pin; remove the caret)
- Modify: `crates/ffi/build.rs`:
  - Replace `.expect()` with graceful `eprintln!("cargo:warning=...")` fallbacks on env-var read failures
  - Replace `if let Ok(bindings) = ...` with `match` that panics loudly on cbindgen error (build-time only, not runtime)
  - Replace `.ok()` on `fs::copy` with `eprintln!` on failure
- Modify: `.github/workflows/ci.yml` — add step after `cargo build --release` that runs `cargo build -p agent-desktop-ffi` and then `git diff --exit-code crates/ffi/include/`
- Modify: `crates/ffi/src/error.rs`:
  - Remove the line-56 `.unwrap()` on `CString::new("(message contained null byte)")` — replace with static bytes
  - Add compile-time assertion: `const _: () = assert!(ErrorCode::VARIANTS == AdResult::non_ok_variant_count());` (use a manual const-fn match counter if `variant_count` is nightly-only)
- Modify: `crates/ffi/src/convert.rs` — remove file-scoped `#![allow(dead_code)]` OR keep with an explicit comment explaining the cdylib false-positive rationale
- Audit and remove: every `#[allow(dead_code)]` in `crates/ffi/src/` that annotates a fn reachable from a `#[no_mangle]` export; consolidate remaining to module-level where needed
- Test: build-script test harness + CI workflow dry-run

**Approach:**
- Exact cbindgen pin (`= 0.27.0`) eliminates formatting drift from patch-version changes. Bumping cbindgen becomes an explicit PR with a regenerated header.
- Header drift check: CI runs `cargo clean -p agent-desktop-ffi && cargo build -p agent-desktop-ffi` then `git diff --exit-code`. Any header change that wasn't committed fails the build.
- Compile-time variant assertion: compile-time check that every `ErrorCode` variant has a matching `AdResult` entry. Prevents silent misses when core adds an error.

**Patterns to follow:**
- CI `fail if cargo tree -p core contains platform crates` pattern in `.github/workflows/ci.yml:43-49` — similar shape for the new drift check.

**Test scenarios:**
- Happy path: `cargo build -p agent-desktop-ffi` succeeds; header is regenerated identically
- Error path: if someone modifies a type in `types.rs` without committing the regenerated header, CI fails with a clear diff
- Error path: if someone adds a new `ErrorCode::Foo` variant without adding `AdResult::ErrFoo`, `cargo build` fails with the const-assert message
- Error path: if `cbindgen.toml` is malformed or cbindgen itself errors, `cargo build` fails loudly (not silently emits a stale header)

**Verification:** Grep `unwrap\|expect` in `crates/ffi/src/*.rs` (non-test sections) returns zero hits. CI header-drift step exists and passes.

### Phase 7 — Documentation

- [ ] **Unit 13: Skill + rustdoc + CLAUDE.md + code cleanup**

**Goal:** Every agent / developer / consumer reading project docs sees accurate FFI guidance. Per-field nullability is documented in the generated header. Wildcard re-exports eliminated. Inline comments stripped.

**Requirements:** R12, R16, R17, R18, R24

**Dependencies:** All prior units (doc-writes reference the final API shape).

**Files:**
- Create: `skills/agent-desktop-ffi/SKILL.md` (frontmatter + overview)
- Create: `skills/agent-desktop-ffi/references/ownership.md` (who allocates / who frees table, per pointer type)
- Create: `skills/agent-desktop-ffi/references/error-handling.md` (errno-style contract, last-error lifetime, enum validation)
- Create: `skills/agent-desktop-ffi/references/threading.md` (macOS main-thread rule, Python permission attachment quirk, single-owner handle rule)
- Modify: `crates/ffi/src/lib.rs` — add `//!` crate-level rustdoc: purpose, ownership model, error pattern, thread safety, example build/link
- Modify: `crates/ffi/src/lib.rs` — replace `pub use types::*` with explicit re-exports (list every type the FFI actually exports)
- Modify: `crates/ffi/src/types.rs` — add `///` doc-comments on every field for nullability / lifetime / sentinel-value documentation (cbindgen propagates to header)
- Modify: `crates/ffi/src/tree.rs` — strip inline `//` comments on lines 43, 60, 215, 220, 231, 244, 251, 255, 260
- Modify: `CLAUDE.md` line 79 — update "binary crate is the only place that wires platform → core" to reflect FFI being the second legitimate wiring point
- Modify: `CLAUDE.md` workspace tree section — add `crates/ffi/` to the layout diagram
- Modify: `skills-lock.json` (auto-updated by clawhub, but verify the new skill is tracked)

**Approach:**
- `SKILL.md` frontmatter: `name: agent-desktop-ffi`, version matching `Cargo.toml` (0.1.x), tags `ffi, c-bindings, cdylib`, description covering the Python/Swift/Go/Node use case.
- Reference docs split by topic so an AI or human consumer can load the relevant file without reading everything.
- Ownership table: for every `ad_*` fn that allocates, list the matching `ad_free_*` / `ad_*_list_free` / `ad_release_*_fields` / `ad_free_handle` / `ad_free_string` / `ad_free_action_result` / `ad_free_image` / `ad_free_tree`.
- Threading doc: explicit warning about `AXIsProcessTrusted` attaching to the hosting executable (Python, not agent-desktop) — consumers will hit this on day one.

**Patterns to follow:**
- Existing `skills/agent-desktop/SKILL.md` frontmatter + reference structure.
- `docs/prd-addendum-skill-maintenance.md` mandates — every new platform / interface adds a skill directory.
- Rustdoc field-level docs in `crates/core/src/adapter.rs` — pattern for per-field documentation cbindgen propagates.

**Test scenarios:**
- Test expectation: none — this unit is pure documentation. Verification is via the review checklist below.

**Verification:**
- `skills/agent-desktop-ffi/SKILL.md` exists with valid frontmatter (clawhub sync dry-run passes)
- Generated `agent_desktop.h` contains `///` comments on every field for at least: `AdWindowInfo.bounds` (nullability sentinel `has_bounds`), `AdAppInfo.bundle_id` (optional, null if unset), `AdSurfaceInfo.title` / `.item_count`, `AdNode.{ref_id,name,value,description,hint}`, `AdActionResult.ref_id`
- `lib.rs` has no wildcard re-exports (grep `pub use types::\*` returns zero)
- `tree.rs` has no inline `//` comments (only `///` doc-comments)
- `CLAUDE.md` FFI mention at the workspace-tree section and the platform-wiring paragraph
- Clawhub sync picks up the new skill on the next release

## System-Wide Impact

- **Interaction graph:** The FFI crate is the second platform → core wiring point (after `src/`). CI dependency-isolation check currently only guards `agent-desktop-core`; this plan doesn't add a new gate for `agent-desktop-ffi` because its platform imports are intentional (parallel to the binary crate). CLAUDE.md updated to reflect the two-wiring-point reality.
- **Error propagation:** The errno-style last-error lifetime change (Unit 3) affects every FFI fn's success path — all `clear_last_error()` calls on success become no-ops. Downstream: consumers who were accidentally depending on errors being cleared on success must re-read the documentation (but no current consumer exists — PR unmerged).
- **State lifecycle risks:** `ad_free_handle` (Unit 6) is the first handle-releasing surface; forgetting to call it in an agent loop is a steady leak of Core Foundation retains on macOS. Skill doc must make this prominent. `NativeHandle` single-owner invariant is newly documented but was always assumed.
- **API surface parity:** Commands now exposed via FFI (notifications + find/get/is) reach parity with the documented "50 commands" count from the CLI. The gap list (wait/batch/press_key_for_app/get_live_value) is the only Phase 2 FFI coverage work.
- **Integration coverage:** Unit tests cover Rust-side correctness. The plan explicitly does not add Python/Swift/Go/Node integration tests — each is its own test scaffolding work (deferred). Cross-language layout parity IS implicitly tested by the `crates/ffi/tests/layout_contract.rs` test asserting struct size/align/offsets match known-good expectations.
- **Unchanged invariants:** `PlatformAdapter` trait remains the sole abstraction. Core never imports platform crates (CI still enforces). `ErrorCode` enum ordering is preserved (new variants, if any, go to the end). CLI command count (53) is unchanged — this plan doesn't add CLI commands, only FFI exports.

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| Per-package `panic = "unwind"` adds landing-pad size to the cdylib | Measure cdylib size before/after; accept if <500 KB additional. FFI cdylib is not under the 15 MB CLI budget. |
| BFS rewrite introduces a regression in tree output format | New test asserts direct-child iteration. Existing `test_flatten_depth_first_order` renamed and reworked. Golden-fixture tests (if any added later) snapshot the BFS format. |
| Core trait addition (`release_handle`) breaks Windows / Linux adapter stubs at compile | Default `not_supported()` impl means Win/Linux stubs compile without change. macOS impl lands in this plan. |
| `ad_free_handle` consumer forgetfulness causes CFRetain leak anyway | Skill doc's Flow B example makes `ad_free_handle` the mandatory final step. Future static analyzer for C consumers could detect missing frees; out of scope. |
| Header drift CI check produces false positives when developer machine's `cargo build` runs before CI | Documented in README (Development section): after modifying FFI types, commit the regenerated header immediately. |
| Cbindgen 0.27 gets yanked or has CVE | Exact-pin forces an explicit bump; worst case, upgrade PR touches `Cargo.toml` + regenerates header in one commit. Low likelihood. |
| The opaque list pattern locks out future zero-copy access | Accept: opaque handles are net better for safety now; zero-copy can be added non-breakingly by exposing a `_data_ptr()` accessor later. |
| Panic boundary's static-message approach loses dynamic context in a panic | Accept: the alternative (allocate inside panic handler) risks double-panic → abort. Static message names the boundary; full diagnostic via `ad_last_error_platform_detail()` on recoverable errors. |
| Main-thread `debug_assert` in release builds is a silent UB channel | Header's prominent warning + skill doc make the constraint loudly known. Consumers that violate get what they ask for. |
| NULL-replacement in mandatory fields masks upstream Electron AX bugs | Replacement is logged via `tracing` at `warn` level so test runs still surface occurrences. |

## Documentation / Operational Notes

- **CLAUDE.md update** is part of Unit 13. Lines to touch: the "binary crate is the only place that wires platform → core" claim and the workspace tree diagram that omits `crates/ffi/`.
- **`skills/agent-desktop-ffi/`** is a new skill directory; the release pipeline (`clawhub sync --root skills/ --all`) picks it up automatically on the next release, so no release-workflow edits needed.
- **No rollout gate.** The fixes all target an unmerged PR. Merge = ship (within the scope of the Phase 1 plan).
- **Post-merge follow-ups tracked by this plan:**
  1. Run `ce:compound` on each of the 15 review findings to seed `docs/solutions/`.
  2. Open a separate plan for `ad_wait`, `ad_batch`, `ad_press_key_for_app`, `ad_get_live_value` FFI exposure.
  3. Open a separate plan for cdylib artifact distribution (release pipeline, npm slot for native libraries, cross-platform build matrix).

## Sources & References

- **Origin review:** [PR #22 review 4102879703](https://github.com/lahfir/agent-desktop/pull/22#pullrequestreview-4102879703) — `CHANGES_REQUESTED` by [@lahfir](https://github.com/lahfir)
- **Origin branch:** `feat/c-bindings` (PR #22 by Jake Rosoman)
- **Project standards:** `CLAUDE.md` (LOC limits, no unwrap, explicit re-exports, conventional commits, skill maintenance)
- **Trait surface:** `crates/core/src/adapter.rs` (28 methods, `PlatformAdapter`, `NativeHandle`, `SnapshotSurface`, `WindowFilter`)
- **Retain discipline reference:** `crates/macos/src/tree/element.rs:24-39` (`AXElement` `Drop`+`Clone` with `CFRelease`/`CFRetain`)
- **Command-layer logic to thin-wrap:** `crates/core/src/commands/{list_notifications,dismiss_notification,dismiss_all_notifications,notification_action,find,get,is}.rs`
- **cbindgen config:** `crates/ffi/cbindgen.toml`
- **CI workflow:** `.github/workflows/ci.yml`
- **Release workflow:** `.github/workflows/release.yml`
- **Skill precedent:** `skills/agent-desktop/SKILL.md`
