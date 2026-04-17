# Error handling

The FFI uses an errno-style last-error pattern. Every `AdResult`-returning
function returns `AD_RESULT_OK` (= 0) on success or a negative error
code on failure. When a failure occurs, thread-local last-error state is
populated; read it with the `ad_last_error_*` accessors.

## Minimal pattern

```c
AdResult rc = ad_launch_app(adapter, "com.apple.finder", 5000, &win);
if (rc != AD_RESULT_OK) {
    const char *msg = ad_last_error_message();
    const char *sug = ad_last_error_suggestion();   // may be NULL
    fprintf(stderr, "launch_app failed (%d): %s\n", (int)rc, msg ? msg : "(no message)");
    if (sug) fprintf(stderr, "  suggestion: %s\n", sug);
    // no need to release the struct — out-param was zero-initialized
    return -1;
}
// ...use win...
ad_release_window_fields(&win);
```

## Lifetime contract

The pointer returned by `ad_last_error_message()`,
`ad_last_error_suggestion()`, and `ad_last_error_platform_detail()`
remains valid across any number of subsequent **successful** FFI calls.
Only the next **failing** call rotates the slot.

Consequence: you can cache the pointer right after a failure and keep
reading it until the next failure — equivalent to POSIX `errno` /
`strerror`.

```c
AdResult rc = ad_some_call(...);
const char *msg = ad_last_error_message();   // snapshot

ad_check_permissions(adapter);                // success
ad_check_permissions(adapter);                // success
printf("%s\n", msg);                          // still valid
```

Failure-path calls rotate: if a subsequent call fails, the prior
pointer may dangle. Read it before the next potentially-failing call.

## Error codes

| Name                                 | i32   | Meaning                               |
|--------------------------------------|-------|---------------------------------------|
| `AD_RESULT_OK`                       |   0   | Success                               |
| `AD_RESULT_ERR_PERM_DENIED`          |  -1   | Accessibility / input permission missing |
| `AD_RESULT_ERR_ELEMENT_NOT_FOUND`    |  -2   | Ref resolve / find miss               |
| `AD_RESULT_ERR_APP_NOT_FOUND`        |  -3   | Bundle/PID lookup miss                |
| `AD_RESULT_ERR_ACTION_FAILED`        |  -4   | Action dispatched but rejected        |
| `AD_RESULT_ERR_ACTION_NOT_SUPPORTED` |  -5   | Platform cannot perform this action   |
| `AD_RESULT_ERR_STALE_REF`            |  -6   | Handle pre-dates a DOM change         |
| `AD_RESULT_ERR_WINDOW_NOT_FOUND`     |  -7   | Window filter matched nothing         |
| `AD_RESULT_ERR_PLATFORM_NOT_SUPPORTED`| -8  | API unavailable on this OS            |
| `AD_RESULT_ERR_TIMEOUT`              |  -9   | Wait exceeded deadline                |
| `AD_RESULT_ERR_INVALID_ARGS`         | -10   | Null pointer, bad enum, invalid UTF-8 |
| `AD_RESULT_ERR_NOTIFICATION_NOT_FOUND`| -11  | Notification index out of range       |
| `AD_RESULT_ERR_INTERNAL`             | -12   | Rust panic caught at FFI boundary     |

## Enum validation

Every `#[repr(i32)]` enum field is validated at the C boundary. An
out-of-range discriminant returns `AD_RESULT_ERR_INVALID_ARGS` with
diagnostic last-error text. This prevents the consumer from
accidentally triggering undefined behavior by stuffing an arbitrary
`int32_t` into an enum slot.

## Panic safety

Every `extern "C"` entrypoint wraps its body in `catch_unwind`. A
Rust panic inside the FFI surfaces as `AD_RESULT_ERR_INTERNAL` with
message `"rust panic in FFI boundary"`. No `SIGABRT`, no host crash.

The cdylib must be built under the `release-ffi` profile for this
guarantee to hold in optimized builds — the workspace `release` profile
uses `panic = "abort"` (for CLI binary-size reasons).
