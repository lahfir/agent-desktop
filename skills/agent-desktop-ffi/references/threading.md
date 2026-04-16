# Threading

## macOS: main-thread rule

Every adapter-touching entrypoint (`ad_get_tree`, `ad_find`, `ad_get`,
`ad_is`, `ad_resolve_element`, `ad_execute_action`, `ad_screenshot`,
clipboard get/set/clear, mouse, drag, launch, close, focus, window-op,
list-apps/windows/surfaces, notification list/dismiss/action)
**must be invoked on the process's main thread**. macOS accessibility
and Cocoa APIs require this.

The check runs at **runtime, in every build profile** — worker-thread
calls return `AD_RESULT_ERR_INTERNAL` with a `'static` diagnostic
`"agent_desktop FFI entry called off the main thread (macOS requires
main-thread AX/Cocoa calls)"`. No build-config difference; no silent
UB window in release builds.

On non-macOS targets the check is a compile-time `true` and has zero
runtime cost.

Operations that are **safe off-main-thread** (no runtime guard):

- `ad_adapter_create` / `ad_adapter_destroy`
- `ad_last_error_{code,message,suggestion,platform_detail}`
- `ad_check_permissions` (pure process-wide query)
- `ad_app_list_count` / `_get` / `_free`
- `ad_window_list_count` / `_get` / `_free`
- `ad_surface_list_count` / `_get` / `_free`
- `ad_notification_list_count` / `_get` / `_free`
- `ad_image_buffer_data` / `_size` / `_width` / `_height` / `_format` / `_free`
- `ad_release_window_fields`
- `ad_free_handle` (invokes `CFRelease` which is thread-safe) — but
  still prefer calling from the thread that produced the handle.
- `ad_free_tree`, `ad_free_action_result`, `ad_free_string`

## Python consumers

CPython's GIL serializes calls but does not pin them to the main
thread. If you're calling from anything other than the main interpreter
thread you will silently corrupt state on macOS.

Two patterns work:

1. **Restrict FFI calls to the main thread.** Use `asyncio` with the
   default event loop pinned to main, or a synchronous entrypoint only.
2. **Marshal across threads yourself.** Use a queue; have a dedicated
   main-thread worker that dequeues and invokes the FFI.

## AXIsProcessTrusted inheritance

⚠ **Privilege-escalation vector.** `ad_check_permissions` calls macOS's
`AXIsProcessTrusted()`, which returns the trust status of the **hosting
executable** — i.e., the `python3` / `node` / `swift` process, not
`agent-desktop` itself.

Consequence: granting accessibility permission to one Python script's
Python interpreter grants it to every Python script that dlopens
`libagent_desktop_ffi.dylib`. Document this prominently for your
consumers; consider requiring opt-in permission prompts in host code
rather than relying on macOS's binary-level grant.

## Thread-ownership of handles

`ad_resolve_element` returns an opaque `AdNativeHandle` that wraps a
platform pointer. The handle is **single-owner, single-thread** by FFI
contract:

- Create it on thread A → free it on thread A with `ad_free_handle`.
  Transferring the handle to thread B is undefined behavior.
- Use it in FFI calls only from the same thread that produced it.

## Last-error is thread-local

Every thread has its own last-error slot. Thread A's failure does not
set thread B's last-error; `ad_last_error_*` accessors always see the
calling thread's state.
