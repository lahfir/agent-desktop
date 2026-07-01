---
name: agent-desktop-ffi
version: 0.4.1
tags: ffi, c-bindings, cdylib, python, swift, node, go, rust-ffi
requirements:
  - agent-desktop-ffi
description: >
  C-ABI bindings over agent-desktop's PlatformAdapter. Consumers
  (Python ctypes, Swift, Node ffi-napi, Go cgo, C++, Ruby fiddle)
  link libagent_desktop_ffi.{dylib,so,dll} and call `ad_*` functions
  directly instead of spawning the CLI binary per call. The canonical
  observe-act workflow is: ad_init → ad_adapter_create[_with_session]
  → ad_snapshot → parse @e refs → ad_execute_by_ref → ad_free_string
  → ad_adapter_destroy.
---

# agent-desktop-ffi

Direct C-ABI access to every PlatformAdapter operation. Build the
cdylib with the workspace's `release-ffi` profile:

```sh
cargo build --profile release-ffi -p agent-desktop-ffi
```

The output is `target/release-ffi/libagent_desktop_ffi.dylib`
(`.so` on Linux, `.dll` on Windows) plus a committed C header at
`crates/ffi/include/agent_desktop.h`.

A Python ctypes smoke harness lives at `tests/ffi-python/smoke.py` and
serves as a worked end-to-end example covering the ABI handshake, struct
size validation, `ad_version`, and the snapshot pipeline leg. See
`tests/ffi-python/README.md` for usage.

Four reference topics, loaded as needed:

- [ownership.md](references/ownership.md) — who allocates / who frees,
  for every `*mut T` the FFI hands back to the caller.
- [error-handling.md](references/error-handling.md) — errno-style
  last-error contract, enum validation, panic boundary.
- [threading.md](references/threading.md) — macOS main-thread rule,
  AXIsProcessTrusted inheritance when Python/Node dlopens the cdylib,
  and the single-owner handle invariant.
- [build-and-link.md](references/build-and-link.md) — ABI handshake,
  struct size validation, minimal C and Python examples, observe-act
  workflow, and prebuilt archive locations.

## Observe-act workflow (canonical path)

```
ad_init(AD_ABI_VERSION_MAJOR)                    // verify header ↔ dylib match
adapter = ad_adapter_create_with_session("s1")   // or ad_adapter_create()
rc = ad_snapshot(adapter, "Finder", 0, 10, false, false, &json_out)
// parse json_out: locate @e refs in data.tree, note data.snapshot_id
ad_free_string(json_out)
// build action:
AdAction act = {0}; act.kind = AD_ACTION_KIND_CLICK;
rc = ad_execute_by_ref(adapter, "@e5", snapshot_id, &act, 0, &result_out)
ad_free_string(result_out)
ad_adapter_destroy(adapter)
```

`ad_snapshot` returns a `{version, ok, command, data}` JSON envelope
identical to the CLI output. The `data.tree` field contains `@e`-prefixed
ref IDs for interactive elements. Pass a ref ID and the `snapshot_id`
to `ad_execute_by_ref` to drive the CLI-semantics ref-action pipeline
(RefStore load → strict resolution → actionability preflight → dispatch).

## Core constraints

- **ABI handshake.** Call `ad_init(AD_ABI_VERSION_MAJOR)` once after loading the
  dylib. A mismatch between the compiled-in constant and the loaded dylib returns
  `AD_RESULT_ERR_INVALID_ARGS` — abort rather than proceed. You can also read the
  raw dylib major via `ad_abi_version()` for diagnostic display. New `ad_*` symbols
  and new error codes are additive (no bump required); removed or layout-changed
  symbols increment the major.

- **Session adapters.** `ad_adapter_create_with_session("session-id")` associates
  the adapter with a session namespace for refmap persistence — the same as CLI
  `--session <id>`. Passing the same session ID across adapter lifetimes lets
  `ad_execute_by_ref` with `snapshot_id=NULL` target the latest snapshot from
  that session. Session IDs: 1–64 chars, ASCII alphanumeric / `-` / `_`.
  Invalid IDs return null (check `ad_last_error_*`).

- **Structured session trace (no ABI change).** File-based JSONL tracing activates
  only when the session has a manifest with `trace: on` from `session start`
  (CLI) or equivalent on-disk setup. `ad_adapter_create_with_session` alone does
  **not** create trace files. When tracing is active, `command_context()`-backed
  commands append to one segment per OS process under
  `~/.agent-desktop/sessions/<id>/trace/<pid>-<procTs>.jsonl`. A long-lived host
  reuses the same segment filename for all calls in that process. For unstructured
  diagnostics regardless of session manifest, use `ad_set_log_callback` (below).

- **Main thread only (macOS).** Call every adapter-touching entrypoint
  (`ad_snapshot`, `ad_execute_by_ref`, `ad_wait`, `ad_get_tree`, `ad_find`,
  `ad_get`, `ad_is`, `ad_resolve_element`, `ad_execute_action`,
  `ad_execute_action_with_policy`, `ad_execute_ref_action_with_policy`,
  `ad_screenshot`, clipboard get/set/clear, mouse, drag, launch, close, focus,
  window-op, list-apps/windows/surfaces, notification list/dismiss/action)
  from the process's main thread. macOS accessibility and Cocoa APIs require
  this. The FFI enforces this at runtime in every build profile — a worker-thread
  call returns `AD_RESULT_ERR_INTERNAL` with a diagnostic last-error. On
  non-macOS platforms the check is a compile-time true; there is no runtime cost.

- **Release profile.** `cargo build --release` produces `panic = "abort"` —
  any Rust panic inside an `extern "C"` fn will `SIGABRT` the host. Use
  `--profile release-ffi` to get the correct `panic = "unwind"` profile. CI
  enforces this.

- **Last-error lifetime.** Pointers returned by `ad_last_error_*` remain valid
  across any number of subsequent *successful* FFI calls on the same thread.
  Only the next failing call rotates them. Cache the pointer once, read it as
  many times as you need.

- **ad_last_error_details.** A fourth accessor, `ad_last_error_details()`,
  returns a borrowed JSON string carrying structured details (e.g. the
  actionability check report on `ACTION_FAILED`, candidate summaries on
  `AMBIGUOUS_TARGET`). The details may contain element names, values, and window
  titles from the user's screen — treat as sensitive diagnostics and avoid routing
  to shared log surfaces.

- **Handle release.** Every `ad_resolve_element` / `ad_find` result must be
  released with `ad_free_handle(adapter, &handle)` on the same adapter that
  produced it, before that adapter is destroyed. On macOS this balances the
  internal `CFRetain`; on Windows/Linux the call is a no-op but safe to issue.
  `ad_free_handle` zeroes `handle.ptr` so a follow-up call is a safe no-op.

- **Primary ref-action path.** `ad_execute_by_ref` is the recommended entrypoint
  for the observe-act loop: it loads the RefStore, looks up the ref in the refmap
  (STALE_REF on miss), runs strict element re-identification (STALE_REF / AMBIGUOUS_TARGET),
  runs the live actionability preflight, then dispatches. TypeText and PressKey
  default to `focus_fallback` policy (matching CLI `type`/`press-key`); all other
  actions default to `headless`. Pass `AD_POLICY_KIND_HEADED` (2) to opt in to
  cursor-based fallbacks.

- **Low-level action paths.** `ad_execute_action` (headless, no preflight) and
  `ad_execute_action_with_policy` are raw escape hatches for callers holding a live
  `AdNativeHandle` from `ad_resolve_element` / `ad_find`. Use them when you need
  to bypass the ref-action pipeline. `ad_execute_ref_action_with_policy` accepts a
  pre-resolved `AdRefEntry` instead of a ref string — useful when you have
  serialized an entry outside the RefStore pipeline.

- **Ref-action preflight.** `ad_execute_by_ref` and `ad_execute_ref_action_with_policy`
  both resolve the element strictly and run the live actionability preflight
  (visible, stable, enabled, supported action, policy, editable) before dispatching
  — a disabled or unsupported target fails before any platform call. On
  `AD_RESULT_ERR_ACTION_FAILED`, the structured check report is available as JSON
  via `ad_last_error_details()`.

- **Action result steps.** `AdActionResult.steps` mirrors the CLI `steps` array
  for activation-chain actions. Each entry has `label` and `outcome` strings and
  is owned by the result; release with `ad_free_action_result(&out)`.

- **Tracing / log callback.** Two tracing surfaces coexist:

  1. **Structured file trace** — same JSONL contract as CLI `--trace`, gated by a
     `trace: on` session manifest. Segments include `event`, `ts_ms`, `seq`, and
     redacted fields. Requires `session start` (or equivalent manifest on disk)
     before creating the adapter; plain session-id adapters write nothing to disk.

  2. **`ad_set_log_callback(cb)`** — installs a `tracing` subscriber layer that
     delivers events as JSON to your callback. `cb` receives an int32_t level
     (1=ERROR … 5=TRACE) and a `const char *msg` valid only for the duration of
     the call. Pass `NULL` to unregister. The layer is installed on the first
     non-null call; if a foreign global subscriber already owns the process at
     that point, the install fails with `AD_RESULT_ERR_INTERNAL` and no events are
     ever delivered. Sensitive field values (password, token, text, …) are
     replaced with `{"redacted":true}` before formatting. A panicking callback is
     caught and silently discarded. The callback may fire from threads other than
     the registering thread, and may still fire briefly after a `NULL` unregister
     — keep the callback and any data it captures valid for the process lifetime.

- **Wait.** `ad_wait(adapter, args, &out)` runs the full CLI `wait` command
  (element-appear, window-appear, text-appear, menu-open/close, notification,
  element predicates). Zero-initialize `AdWaitArgs`, set the fields you need, and
  validate the struct size against `AD_WAIT_ARGS_SIZE` / `ad_wait_args_size()` before
  calling. The output is a `{version, ok, command, data}` JSON envelope freed with
  `ad_free_string`. `ad_wait` blocks the calling thread up to `timeout_ms` ms —
  ensure the adapter is not destroyed from another thread while it is running.

- **Text input privacy.** On macOS, focus-fallback or headed text insertion may
  briefly use the clipboard for non-ASCII text. For sensitive text, prefer
  `AD_ACTION_KIND_SET_VALUE` with `AD_POLICY_KIND_HEADLESS` when the target
  supports settable values.

- **Enum discriminants.** Every `#[repr(i32)]` enum field is validated at the C
  boundary — invalid discriminants return `AD_RESULT_ERR_INVALID_ARGS` instead of
  undefined behavior.

- **ABI stability.** The major version in `AD_ABI_VERSION_MAJOR` increments on any
  breaking change (removed symbol, incompatible layout). Additive changes (new
  symbols, new error codes) do not bump it. Before 1.0, pin the exact version of
  libagent_desktop_ffi you link against.

- **`ad_get_tree` vs `ad_snapshot`.** `ad_get_tree` returns a raw flat BFS tree
  without `@e` refs, no refmap persistence, and no JSON envelope — use it for
  custom traversal or UI inspection. For observe-act agents that drive actions via
  `ad_execute_by_ref`, always start with `ad_snapshot`.
