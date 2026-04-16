# Pointer ownership

Every `*mut T` / `*const T` returned by the FFI comes with a matching
free function. Always call it; the allocator the FFI uses is Rust's
`Box::from_raw` / `CString::from_raw`, which cannot be freed with C's
`free()`.

## Allocation / release table

| Allocates                              | Frees with                       |
|----------------------------------------|----------------------------------|
| `ad_adapter_create()`                  | `ad_adapter_destroy(adapter)`    |
| `ad_list_apps(... &apps, &count)`      | `ad_free_apps(apps, count)`      |
| `ad_list_windows(... &wins, &count)`   | `ad_free_windows(wins, count)`   |
| `ad_launch_app(... &out)`              | `ad_free_window(&out)`           |
| `ad_list_surfaces(... &sfs, &count)`   | `ad_free_surfaces(sfs, count)`   |
| `ad_get_tree(... &out)`                | `ad_free_tree(&out)`             |
| `ad_resolve_element(... &handle)`      | `ad_free_handle(adapter, &handle)` |
| `ad_execute_action(... &out)`          | `ad_free_action_result(&out)`    |
| `ad_screenshot(... &img)`              | `ad_free_image(&img)`            |
| `ad_get_clipboard(... &text)`          | `ad_free_string(text)`           |

## Rules

- Every free function is **null-tolerant**. `ad_free_tree(NULL)`,
  `ad_free_handle(adapter, NULL)`, etc. are no-ops.
- Double-free is **undefined behavior**. Set the pointer to `NULL`
  after freeing.
- Pointers inside a struct (`.id`, `.title`, `.app_name`) are freed by
  the struct's free function — do not `ad_free_string()` them
  individually.
- Ownership does **not** transfer back to Rust after you free. Keep a
  local `NULL` to prevent accidental reuse.

## Out-param zeroing

Every fallible FFI function zeroes its out-param at entry, before any
fallible work. On error, calling the paired free function is safe: all
pointers inside are guaranteed null, all counts zero, so the free is a
no-op rather than a double-free on a previous caller's allocation.
