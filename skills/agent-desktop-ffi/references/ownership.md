# Pointer ownership

Every `*mut T` / `*const T` returned by the FFI comes with a matching
free function. Always call it; the allocator the FFI uses is Rust's
`Box::from_raw` / `CString::from_raw`, which cannot be freed with C's
`free()`.

## Allocation / release table

| Allocates                                               | Frees with                             |
|---------------------------------------------------------|-----------------------------------------|
| `ad_adapter_create()`                                   | `ad_adapter_destroy(adapter)`           |
| `ad_list_apps(adapter, &list)`                          | `ad_app_list_free(list)`                |
| `ad_list_windows(adapter, app, focused, &list)`         | `ad_window_list_free(list)`             |
| `ad_list_surfaces(adapter, pid, &list)`                 | `ad_surface_list_free(list)`            |
| `ad_list_notifications(adapter, filter, &list)`         | `ad_notification_list_free(list)`       |
| `ad_dismiss_all_notifications(adapter, f, &ok, &fail)`  | `ad_notification_list_free` on each (or `ad_dismiss_all_notifications_free(ok, fail)`) |
| `ad_launch_app(adapter, id, timeout, &out)`             | `ad_release_window_fields(&out)` (free interior strings; struct itself lives on caller's stack) |
| `ad_get_tree(adapter, win, opts, &out)`                 | `ad_free_tree(&out)`                    |
| `ad_resolve_element(adapter, entry, &handle)`           | `ad_free_handle(adapter, &handle)` — `*mut AdNativeHandle`; the call zeroes `handle.ptr` on success so a follow-up call is a no-op |
| `ad_find(adapter, win, query, &handle)`                 | same as `ad_resolve_element`            |
| `ad_execute_action(adapter, handle, action, &out)`      | `ad_free_action_result(&out)`           |
| `ad_notification_action(adapter, idx, expected_app, expected_title, name, &out)` | `ad_free_action_result(&out)` — pass the `app_name`/`title` you observed in `ad_list_notifications` (either may be null) so NC reorder between list and press is caught with `ERR_NOTIFICATION_NOT_FOUND` instead of pressing a different notification |
| `ad_screenshot(adapter, target, &buf)`                  | `ad_image_buffer_free(buf)` (buf is opaque; read via `ad_image_buffer_{data,size,width,height,format}`) |
| `ad_get_clipboard(adapter, &text)`                      | `ad_free_string(text)`                  |
| `ad_get(adapter, handle, property, &text)`              | `ad_free_string(text)` (text may be null on "property absent"; `ad_free_string(NULL)` is a no-op) |

## Rules

- Every free function is **null-tolerant**. `ad_free_tree(NULL)`,
  `ad_free_handle(adapter, NULL)`, `ad_free_string(NULL)`, etc. are
  no-ops. List accessors (`ad_*_list_count`, `_get`) also accept null
  and return `0` / `NULL` respectively.
- **Double-free of list handles and `AdImageBuffer` is undefined.** The
  opaque wrappers are allocated by `Box::into_raw`; the second call
  would invoke `Box::from_raw` on a freed allocation. Always set the
  pointer to `NULL` after freeing.
- **`ad_free_handle` is safe to double-call** — it zeroes
  `handle.ptr` after the platform release, so a follow-up call sees
  `NULL` and returns `AD_RESULT_OK` without re-entering `CFRelease`.
- Pointers inside a struct (`.id`, `.title`, `.app_name`, each
  `AdNotificationInfo.body`, etc.) are freed by the struct's owning
  free function (list_free / release_fields) — do not
  `ad_free_string()` them individually.
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
- `ad_*_list` and `ad_resolve_element` / `ad_find` all apply the same
  pattern to their handle / list out-params.
