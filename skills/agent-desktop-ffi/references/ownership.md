# Pointer ownership

Every `*mut T` / `*const T` returned by the FFI comes with a matching
free function. Always call it; the allocator the FFI uses is Rust's
`Box::from_raw` / `CString::from_raw`, which cannot be freed with C's
`free()`.

## Allocation / release table

### Command-backed JSON strings

These entrypoints write an owned, NUL-terminated JSON envelope into
`*out`; free with `ad_free_string`. See error-handling.md for the
dual-failure mode (command-level errors write `ok:false` JSON into
`*out`; infrastructure errors leave `*out` null with no allocation).

| Allocates                                                                         | Frees with              |
|-----------------------------------------------------------------------------------|-------------------------|
| `ad_version(&out)`                                                                | `ad_free_string(out)`   |
| `ad_status(adapter, &out)`                                                        | `ad_free_string(out)`   |
| `ad_snapshot(adapter, app, surface, max_depth, interactive_only, compact, &out)` | `ad_free_string(out)`   |
| `ad_execute_by_ref(adapter, ref_id, snapshot_id, action, policy, &out)`          | `ad_free_string(out)`   |
| `ad_wait(adapter, args, &out)`                                                    | `ad_free_string(out)`   |

### Adapter lifecycle

| Allocates                                            | Frees with                              |
|------------------------------------------------------|-----------------------------------------|
| `ad_adapter_create()`                                | `ad_adapter_destroy(adapter)`           |
| `ad_adapter_create_with_session(session)`            | `ad_adapter_destroy(adapter)`           |

### Opaque list handles

| Allocates                                                    | Frees with                              |
|--------------------------------------------------------------|-----------------------------------------|
| `ad_list_apps(adapter, &list)`                               | `ad_app_list_free(list)`                |
| `ad_list_windows(adapter, app, focused, &list)`              | `ad_window_list_free(list)`             |
| `ad_list_surfaces(adapter, pid, &list)`                      | `ad_surface_list_free(list)`            |
| `ad_list_notifications(adapter, filter, &list)`              | `ad_notification_list_free(list)`       |
| `ad_dismiss_all_notifications(adapter, f, &ok, &fail)`       | `ad_notification_list_free` on each, or `ad_dismiss_all_notifications_free(ok, fail)` |

### App / window lifecycle

| Allocates                                                 | Frees with                                                          |
|-----------------------------------------------------------|---------------------------------------------------------------------|
| `ad_launch_app(adapter, id, timeout, &out)`               | `ad_release_window_fields(&out)` — frees interior strings only; the `AdWindowInfo` struct lives on the caller's stack |

### Raw tree and element access

| Allocates                                                                              | Frees with                              |
|----------------------------------------------------------------------------------------|-----------------------------------------|
| `ad_get_tree(adapter, win, opts, &out)`                                                | `ad_free_tree(&out)`                    |
| `ad_resolve_element(adapter, entry, &handle)`                                          | `ad_free_handle(adapter, &handle)` — zeroes `handle.ptr` so a follow-up call is a no-op |
| `ad_find(adapter, win, query, &handle)`                                                | same as `ad_resolve_element`            |

### Action results

| Allocates                                                                              | Frees with                   |
|----------------------------------------------------------------------------------------|------------------------------|
| `ad_execute_action(adapter, handle, action, &out)`                                     | `ad_free_action_result(&out)` |
| `ad_execute_action_with_policy(adapter, handle, action, policy, &out)`                 | `ad_free_action_result(&out)` |
| `ad_execute_ref_action_with_policy(adapter, entry, action, policy, &out)`              | `ad_free_action_result(&out)` |
| `ad_notification_action(adapter, idx, expected_app, expected_title, name, &out)` — pass the `app_name`/`title` from `ad_list_notifications` (either may be null) so NC reorder between list and press returns `ERR_NOTIFICATION_NOT_FOUND` instead of pressing a different notification | `ad_free_action_result(&out)` |

### Clipboard and image buffers

| Allocates                                   | Frees with                                                          |
|---------------------------------------------|---------------------------------------------------------------------|
| `ad_get_clipboard(adapter, &text)`          | `ad_free_string(text)`                                              |
| `ad_get(adapter, handle, property, &text)`  | `ad_free_string(text)` — text may be null when the property is absent; `ad_free_string(NULL)` is a no-op |
| `ad_screenshot(adapter, target, &buf)`      | `ad_image_buffer_free(buf)` (buf is opaque; read via `ad_image_buffer_{data,size,width,height,format}`) |

## Rules

- Every free function is **null-tolerant**. `ad_free_tree(NULL)`,
  `ad_free_handle(adapter, NULL)`, `ad_free_string(NULL)`, etc. are
  no-ops. List accessors (`ad_*_list_count`, `_get`) also accept null
  and return `0` / `NULL` respectively.
- **Double-free of list handles and `AdImageBuffer` is undefined.** The
  opaque wrappers are allocated by `Box::into_raw`; the second call
  would invoke `Box::from_raw` on a freed allocation. Always set the
  pointer to `NULL` after freeing.
- **`ad_free_handle` is safe to double-call** — it zeroes `handle.ptr`
  after the platform release, so a follow-up call sees `NULL` and
  returns `AD_RESULT_OK` without re-entering `CFRelease`.
- **Adapters must outlive their handles.** Free every handle with the
  same adapter that produced it before calling `ad_adapter_destroy`.
  Destroying the adapter first and later freeing its handles is
  undefined behavior.
- Pointers inside a struct (`.id`, `.title`, `.app_name`, each
  `AdNotificationInfo.body`, etc.) are freed by the struct's owning
  free function (list_free / release_fields) — do not call
  `ad_free_string()` on them individually.
- `AdActionResult` owns `action`, `ref_id`, `post_state`,
  `post_state.states`, `steps`, and each `steps[i].label` /
  `steps[i].outcome`; free all of them only through
  `ad_free_action_result(&out)`. Treat the returned counts as
  read-only metadata; release the unmodified result struct.
- Ownership does **not** transfer back to Rust after you free. Keep a
  local `NULL` to prevent accidental reuse.

## Out-param zeroing

Every fallible FFI function zeroes its out-param **before** any guard
(pointer validation, main-thread check, UTF-8 validation). On error,
calling the paired free function is safe: all pointers inside are
guaranteed null, all counts zero, so the free is a no-op rather than
a double-free on a previous caller's allocation.

In particular:

- `ad_get_clipboard` writes `*out = NULL` before the adapter call —
  no stale buffer visible on error.
- `ad_launch_app` writes `*out = zeroed AdWindowInfo` before the
  platform call — `ad_release_window_fields(&out)` on the zero-init
  struct is a no-op.
- `ad_screenshot` writes `*out = NULL` before allocating the image
  buffer — no stale pointer when the screenshot fails.
- `ad_snapshot`, `ad_execute_by_ref`, `ad_wait`, `ad_status`,
  `ad_version` write `*out = NULL` before any guard, so on
  infrastructure failures no allocation is made and `ad_free_string(NULL)`
  is a safe no-op.
- `ad_*_list` and `ad_resolve_element` / `ad_find` all apply the same
  pattern to their handle / list out-params.
