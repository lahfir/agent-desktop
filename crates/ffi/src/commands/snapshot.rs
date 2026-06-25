use crate::AdAdapter;
use crate::commands::app_error_to_adapter;
use crate::convert::string::{decode_optional_filter, string_to_c};
use crate::convert::surface::snapshot_surface_from_c;
use crate::error::{AdResult, set_last_error};
use crate::ffi_try::trap_panic;
use crate::main_thread::require_main_thread;
use crate::pointer_guard::guard_non_null;
use agent_desktop_core::commands::snapshot::SnapshotArgs;
use agent_desktop_core::error::{AdapterError, ErrorCode};
use agent_desktop_core::output::{ErrorPayload, Response};
use std::ffi::c_char;
use std::ptr;

/// Takes a full CLI-format snapshot of the target application window,
/// allocates `@e` refs for all interactive elements, persists the refmap
/// to disk, and writes the JSON envelope into `*out`.
///
/// The JSON shape matches `agent-desktop snapshot`:
/// `{"version":"2.0","ok":true,"command":"snapshot","data":{"app":"...","window":{...},"ref_count":N,"snapshot_id":"...","tree":{...}}}`.
///
/// **`*out` ownership and error behaviour:**
/// - On success (`AD_RESULT_OK`): `*out` is a heap-allocated JSON string with `"ok":true`.
///   Caller must free it with `ad_free_string`.
/// - On a command-level error (e.g. app not found, snapshot failure): `*out` is a
///   heap-allocated JSON string with `"ok":false` and an `"error"` payload. Caller
///   must still free it with `ad_free_string`. The last-error slot is also set.
/// - On an argument or infrastructure error (null adapter, off-main-thread, invalid
///   UTF-8, bad surface discriminant, context failure): `*out` is set to null and no
///   allocation is made. Only the last-error slot is set.
///
/// `app` is tri-state:
/// - null — snapshot the currently focused window (same as running the command with no `--app`).
/// - valid UTF-8 string — snapshot the named application's focused window.
/// - non-null but invalid UTF-8 or exceeding `AD_MAX_STRING_BYTES` — returns `ErrInvalidArgs`.
///
/// `surface` is an `AdSnapshotSurface` discriminant (0 = Window, 1 = Focused, …).
/// An out-of-range value returns `ErrInvalidArgs`.
///
/// Skeleton mode and `--root` drill-down are not exposed here; they are a
/// fast-follow to this entrypoint.
///
/// # Safety
///
/// `adapter` must be a non-null pointer from `ad_adapter_create` or
/// `ad_adapter_create_with_session`. `out` must be a non-null writable
/// `*mut *mut c_char`. `app` must be null or a NUL-terminated string within
/// `AD_MAX_STRING_BYTES + 1` bytes. All pointers must remain valid for the
/// duration of the call. `adapter` must be used from the main thread on macOS.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_snapshot(
    adapter: *const AdAdapter,
    app: *const c_char,
    surface: i32,
    max_depth: u8,
    interactive_only: bool,
    compact: bool,
    out: *mut *mut c_char,
) -> AdResult {
    guard_non_null!(out, c"out is null");
    unsafe { *out = ptr::null_mut() };
    trap_panic(|| {
        if let Err(rc) = require_main_thread() {
            return rc;
        }
        guard_non_null!(adapter, c"adapter is null");

        let app_filter = unsafe { decode_optional_filter!(app, "app") };

        let core_surface = match snapshot_surface_from_c(surface, "surface") {
            Ok(s) => s,
            Err(e) => {
                set_last_error(&e);
                return AdResult::ErrInvalidArgs;
            }
        };

        let adapter_ref = unsafe { &*adapter };
        let context = match adapter_ref.command_context() {
            Ok(ctx) => ctx,
            Err(e) => {
                let ae = app_error_to_adapter(e);
                set_last_error(&ae);
                return crate::error::last_error_code();
            }
        };

        let args = SnapshotArgs {
            app: app_filter,
            window_id: None,
            max_depth,
            include_bounds: false,
            interactive_only,
            compact,
            surface: core_surface,
            skeleton: false,
            root_ref: None,
            snapshot_id: None,
        };

        let (envelope, had_error) = match agent_desktop_core::commands::snapshot::execute(
            args,
            adapter_ref.inner.as_ref(),
            &context,
        ) {
            Ok(data) => (Response::ok("snapshot", data), false),
            Err(app_err) => {
                let payload = ErrorPayload::from_app_error(&app_err);
                let ae = app_error_to_adapter(app_err);
                set_last_error(&ae);
                (Response::err("snapshot", payload), true)
            }
        };

        let json = match serde_json::to_string(&envelope) {
            Ok(s) => s,
            Err(e) => {
                let ae = AdapterError::new(
                    ErrorCode::Internal,
                    format!("failed to serialize snapshot envelope: {e}"),
                );
                set_last_error(&ae);
                return AdResult::ErrInternal;
            }
        };

        let c_ptr = string_to_c(&json);
        if c_ptr.is_null() {
            let ae = AdapterError::new(ErrorCode::Internal, "snapshot JSON contains interior NUL");
            set_last_error(&ae);
            return AdResult::ErrInternal;
        }

        unsafe { *out = c_ptr };

        if had_error {
            crate::error::last_error_code()
        } else {
            AdResult::Ok
        }
    })
}
