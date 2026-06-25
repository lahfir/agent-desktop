use crate::convert::string::string_to_c;
use crate::error::{self, AdResult};
use crate::ffi_try::trap_panic;
use crate::pointer_guard::guard_non_null;
use agent_desktop_core::commands::status::execute_with_report_with_context;
use agent_desktop_core::error::{AdapterError, AppError, ErrorCode};
use agent_desktop_core::output::Response;
use std::ffi::c_char;

fn record_app_error(app_err: AppError, fallback: &str) -> AdResult {
    let adapter_err = match app_err {
        AppError::Adapter(e) => e,
        _ => AdapterError::new(ErrorCode::Internal, fallback),
    };
    error::set_last_error(&adapter_err);
    error::last_error_code()
}

/// Returns the adapter's current health and permission state as a JSON
/// envelope matching the `agent-desktop status` CLI output.
///
/// `ad_status` does not query the accessibility tree; it reads the
/// permission report and ref-store metadata only, so it is safe to call
/// from any thread (unlike tree-traversal commands that require the
/// macOS main thread). On success `*out` is a NUL-terminated,
/// heap-allocated JSON string freed with `ad_free_string`. On error
/// `*out` is left null and the last-error slot is populated.
///
/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`
/// that has not been destroyed. `out` must be a non-null writable
/// `*mut *mut c_char`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_status(
    adapter: *const crate::AdAdapter,
    out: *mut *mut c_char,
) -> AdResult {
    guard_non_null!(out, c"out is null");
    unsafe { *out = std::ptr::null_mut() };
    guard_non_null!(adapter, c"adapter is null");

    trap_panic(|| {
        let adapter = unsafe { &*adapter };

        let ctx = match adapter.command_context() {
            Ok(c) => c,
            Err(app_err) => return record_app_error(app_err, "failed to build command context"),
        };

        let report = adapter.inner.permission_report();

        let response = match execute_with_report_with_context(&*adapter.inner, &report, &ctx) {
            Ok(data) => Response::ok("status", data),
            Err(app_err) => return record_app_error(app_err, "status command failed"),
        };

        let json = match serde_json::to_string(&response) {
            Ok(s) => s,
            Err(_) => {
                error::set_last_error_static(
                    AdResult::ErrInternal,
                    c"failed to serialize status response",
                );
                return AdResult::ErrInternal;
            }
        };

        let ptr = string_to_c(&json);
        if ptr.is_null() {
            error::set_last_error_static(
                AdResult::ErrInternal,
                c"status response contained interior NUL",
            );
            return AdResult::ErrInternal;
        }

        unsafe { *out = ptr };
        AdResult::Ok
    })
}
