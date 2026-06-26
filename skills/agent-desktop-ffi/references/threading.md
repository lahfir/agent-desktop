# Threading

## macOS: main-thread rule

Every adapter-touching entrypoint **must be invoked on the process's
main thread**. macOS accessibility and Cocoa APIs require this.

Entrypoints subject to the main-thread guard:

- `ad_snapshot`, `ad_execute_by_ref`, `ad_wait`
- `ad_get_tree`, `ad_find`, `ad_get`, `ad_is`, `ad_resolve_element`
- `ad_execute_action`, `ad_execute_action_with_policy`,
  `ad_execute_ref_action_with_policy`
- `ad_screenshot`
- Clipboard: `ad_get_clipboard`, `ad_set_clipboard`, `ad_clear_clipboard`
- Input: `ad_mouse_event`, `ad_drag`
- App / window: `ad_launch_app`, `ad_close_app`, `ad_focus_window`,
  `ad_window_op`
- Listing: `ad_list_apps`, `ad_list_windows`, `ad_list_surfaces`
- Notifications: `ad_list_notifications`, `ad_dismiss_notification`,
  `ad_dismiss_all_notifications`, `ad_notification_action`

The check runs at **runtime, in every build profile** — worker-thread
calls return `AD_RESULT_ERR_INTERNAL` with a `'static` diagnostic
`"agent_desktop FFI entry called off the main thread (macOS requires
main-thread AX/Cocoa calls)"`. No build-config difference; no silent
UB window in release builds.

On non-macOS targets the check is a compile-time `true` and has zero
runtime cost.

## Operations safe off the main thread

These functions carry no runtime main-thread guard:

- `ad_adapter_create` / `ad_adapter_create_with_session` / `ad_adapter_destroy`
- `ad_init`, `ad_abi_version`
- `ad_version` (no adapter; pure serialization)
- `ad_status` (reads permission report and ref-store metadata only; no AX tree query)
- `ad_check_permissions` (pure process-wide AX trust query)
- `ad_set_log_callback`
- `ad_last_error_code`, `ad_last_error_message`, `ad_last_error_suggestion`,
  `ad_last_error_platform_detail`, `ad_last_error_details`
- List accessors: `ad_app_list_count` / `_get` / `_free`,
  `ad_window_list_count` / `_get` / `_free`,
  `ad_surface_list_count` / `_get` / `_free`,
  `ad_notification_list_count` / `_get` / `_free`
- `ad_image_buffer_data` / `_size` / `_width` / `_height` / `_format` / `_free`
- `ad_release_window_fields`
- `ad_free_handle` (invokes `CFRelease` which is thread-safe — but
  still prefer calling from the thread that produced the handle)
- `ad_free_tree`, `ad_free_action_result`, `ad_free_string`

## Log callback threading

`ad_set_log_callback(cb)` is safe to call from any thread (no AX
involvement). However, the installed callback may be invoked from threads
other than the registering thread — tracing events can originate from any
thread that calls an `ad_*` function.

A callback unregistered via `NULL` may still receive one or more
invocations from a concurrent thread for a brief window after this call
returns. The callback (and any data it captures) must remain valid for the
process lifetime, or the caller must quiesce all active adapter calls
before unregistering.

## Python consumers

CPython's GIL serializes calls but does not pin them to the main thread.
If you're calling from anything other than the main interpreter thread
you will silently corrupt state on macOS.

Two patterns work:

1. **Restrict FFI calls to the main thread.** Use `asyncio` with the
   default event loop pinned to main, or a synchronous entrypoint only.
2. **Marshal across threads yourself.** Use a queue; have a dedicated
   main-thread worker that dequeues and invokes the FFI.

## AXIsProcessTrusted inheritance

`ad_check_permissions` calls macOS's `AXIsProcessTrusted()`, which
returns the trust status of the **hosting executable** — i.e., the
`python3` / `node` / `swift` process, not `agent-desktop` itself.

Consequence: granting accessibility permission to one Python script's
Python interpreter grants it to every Python script that dlopens
`libagent_desktop_ffi.dylib`. Document this prominently for your
consumers; consider requiring opt-in permission prompts in host code
rather than relying on macOS's binary-level grant.

## Thread-ownership of handles

`ad_resolve_element` and `ad_find` return an opaque `AdNativeHandle`
that wraps a platform pointer. The handle is **single-owner,
single-thread** by FFI contract:

- Create it on thread A → free it on thread A with `ad_free_handle`.
  Transferring the handle to thread B is undefined behavior.
- Use it in FFI calls only from the same thread that produced it.

## Last-error is thread-local

Every thread has its own last-error slot. Thread A's failure does not
set thread B's last-error; `ad_last_error_*` accessors always see the
calling thread's state.

## ad_wait lifecycle

`ad_wait` blocks the calling thread for up to `timeout_ms` milliseconds
while it holds a live reference into the adapter's allocation. Do not
call `ad_adapter_destroy` on the same handle from another thread while
`ad_wait` is running — that is a use-after-free. Ensure the wait has
returned before destroying the adapter.
