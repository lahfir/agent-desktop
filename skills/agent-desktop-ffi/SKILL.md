---
name: agent-desktop-ffi
version: 0.1.11
tags: ffi, c-bindings, cdylib, python, swift, node, go, rust-ffi
requirements:
  - agent-desktop-ffi
description: >
  C-ABI bindings over agent-desktop's PlatformAdapter. Consumers
  (Python ctypes, Swift, Node ffi-napi, Go cgo, C++, Ruby fiddle)
  link libagent_desktop_ffi.{dylib,so,dll} and call `ad_*` functions
  directly instead of spawning the CLI binary per call.
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

Four reference topics, loaded as needed:

- [ownership.md](references/ownership.md) — who allocates / who frees,
  for every `*mut T` the FFI hands back to the caller.
- [error-handling.md](references/error-handling.md) — errno-style
  last-error contract, enum validation, panic boundary.
- [threading.md](references/threading.md) — macOS main-thread rule,
  AXIsProcessTrusted inheritance when Python/Node dlopens the cdylib,
  and the single-owner handle invariant.
- [build-and-link.md](references/build-and-link.md) — minimum working
  example for Python ctypes and a C program that links the dylib.

## ⚠ Core constraints before you integrate

- **Main thread only (macOS).** Call every adapter-touching entrypoint
  (`ad_get_tree`, `ad_resolve_element`, `ad_execute_action`,
  `ad_screenshot`, clipboard, launch/close, window ops, observation,
  notifications, etc.) from the process's main thread. The FFI enforces
  this at runtime in **every build profile** — a worker-thread call
  returns `AD_RESULT_ERR_INTERNAL` with a diagnostic last-error. On
  non-macOS platforms the check is a compile-time true; there is no
  runtime cost.

- **Release profile.** `cargo build --release` produces
  `panic = "abort"` — any Rust panic inside an `extern "C"` fn will
  `SIGABRT` the host. Use `--profile release-ffi` to get the correct
  `panic = "unwind"` profile. CI enforces this.

- **Last-error lifetime.** Pointers returned by `ad_last_error_*`
  remain valid across any number of subsequent *successful* FFI calls
  on the same thread. Only the next failing call rotates them. Cache
  the pointer once, read it as many times as you need.

- **Handle release.** Every `ad_resolve_element` result must be
  released with `ad_free_handle(adapter, handle)` on the same adapter
  that produced it. On macOS this balances the internal `CFRetain`;
  on Windows/Linux the call is a no-op but safe to issue.

- **Enum discriminants.** Every `#[repr(i32)]` enum field is validated
  at the C boundary — invalid discriminants return
  `AD_RESULT_ERR_INVALID_ARGS` instead of undefined behavior.

- **ABI is unstable before 1.0.** The header lists the exact current
  shapes. Anything added or reordered in a later patch is a breaking
  change; pin the version of libagent_desktop_ffi you link against.

- **`ad_get_tree` returns a raw adapter tree, not the CLI snapshot.**
  Ref IDs are always null, no skeleton/drill-down pipeline is wired
  through, and `interactive_only` / `compact` follow adapter
  semantics which may diverge slightly from the CLI's post-processed
  shape. Use `ad_find` + `ad_get` / `ad_is` for point lookups, or
  invoke the CLI if you need CLI-parity JSON snapshots.
