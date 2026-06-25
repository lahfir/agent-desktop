use crate::adapter::AdAdapter;
use crate::convert::string::{decode_optional_filter, string_to_c, try_c_to_string};
use crate::error::{self, AdResult};
use crate::ffi_try::trap_panic;
use crate::main_thread::require_main_thread;
use crate::pointer_guard::guard_non_null;
use crate::types::wait_args::AdWaitArgs;
use agent_desktop_core::commands::wait::{WaitArgs, WaitModeArgs, WaitPredicateArgs};
use agent_desktop_core::error::{AdapterError, AppError, ErrorCode};
use agent_desktop_core::output::Response;
use std::ffi::c_char;

fn app_error_to_adapter_error(err: AppError) -> AdapterError {
    match err {
        AppError::Adapter(e) => e,
        other => AdapterError::new(ErrorCode::Internal, other.to_string()),
    }
}

/// Runs `wait` with the given args, blocking the calling thread until the
/// condition is met or `timeout_ms` elapses.
///
/// On success `*out` is set to a freshly allocated JSON string containing the
/// CLI-format wait envelope (`{version, ok, command, data}`). The caller must
/// release the string with `ad_free_string(*out)`.
///
/// On failure `*out` is zeroed, the last-error slot is set, and a negative
/// `AdResult` code is returned.
///
/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create` that
/// has not been destroyed. `args` must be non-null and point to a valid
/// zero-initialized `AdWaitArgs`. `out` must be non-null and point to a
/// writable `*mut c_char`.
///
/// All `*const c_char` fields inside `AdWaitArgs` must be null or point to
/// readable, NUL-terminated memory within `AD_MAX_STRING_BYTES + 1` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_wait(
    adapter: *const AdAdapter,
    args: *const AdWaitArgs,
    out: *mut *mut c_char,
) -> AdResult {
    guard_non_null!(args, c"args is null");
    guard_non_null!(out, c"out is null");
    unsafe { *out = std::ptr::null_mut() };

    trap_panic(|| {
        if let Err(rc) = require_main_thread() {
            return rc;
        }
        guard_non_null!(adapter, c"adapter is null");

        let args = unsafe { &*args };
        let adapter_ref = unsafe { &*adapter };

        let ms = if args.has_ms { Some(args.ms) } else { None };

        let element = match unsafe { try_c_to_string(args.element) } {
            Ok(v) => v,
            Err(e) => {
                error::set_last_error(&AdapterError::new(
                    ErrorCode::InvalidArgs,
                    e.describe("element"),
                ));
                return AdResult::ErrInvalidArgs;
            }
        };

        let window = match unsafe { try_c_to_string(args.window) } {
            Ok(v) => v,
            Err(e) => {
                error::set_last_error(&AdapterError::new(
                    ErrorCode::InvalidArgs,
                    e.describe("window"),
                ));
                return AdResult::ErrInvalidArgs;
            }
        };

        let text = match unsafe { try_c_to_string(args.text) } {
            Ok(v) => v,
            Err(e) => {
                error::set_last_error(&AdapterError::new(
                    ErrorCode::InvalidArgs,
                    e.describe("text"),
                ));
                return AdResult::ErrInvalidArgs;
            }
        };

        let snapshot_id = match unsafe { try_c_to_string(args.snapshot_id) } {
            Ok(v) => v,
            Err(e) => {
                error::set_last_error(&AdapterError::new(
                    ErrorCode::InvalidArgs,
                    e.describe("snapshot_id"),
                ));
                return AdResult::ErrInvalidArgs;
            }
        };

        let predicate = match unsafe { try_c_to_string(args.predicate) } {
            Ok(v) => v,
            Err(e) => {
                error::set_last_error(&AdapterError::new(
                    ErrorCode::InvalidArgs,
                    e.describe("predicate"),
                ));
                return AdResult::ErrInvalidArgs;
            }
        };

        let value = match unsafe { try_c_to_string(args.value) } {
            Ok(v) => v,
            Err(e) => {
                error::set_last_error(&AdapterError::new(
                    ErrorCode::InvalidArgs,
                    e.describe("value"),
                ));
                return AdResult::ErrInvalidArgs;
            }
        };

        let action_field = match unsafe { try_c_to_string(args.action) } {
            Ok(v) => v,
            Err(e) => {
                error::set_last_error(&AdapterError::new(
                    ErrorCode::InvalidArgs,
                    e.describe("action"),
                ));
                return AdResult::ErrInvalidArgs;
            }
        };

        let app = unsafe { decode_optional_filter!(args.app, "app") };

        let wait_args = WaitArgs {
            mode: WaitModeArgs {
                ms,
                element,
                window,
                text,
                menu: args.menu,
                menu_closed: args.menu_closed,
                notification: args.notification,
            },
            predicate: WaitPredicateArgs {
                snapshot_id,
                predicate,
                value,
                action: action_field,
                count: if args.has_count {
                    Some(args.count)
                } else {
                    None
                },
            },
            timeout_ms: args.timeout_ms,
            app,
        };

        let ctx = match adapter_ref.command_context() {
            Ok(c) => c,
            Err(app_err) => {
                let adapter_err = app_error_to_adapter_error(app_err);
                error::set_last_error(&adapter_err);
                return AdResult::ErrInternal;
            }
        };

        match agent_desktop_core::commands::wait::execute(
            wait_args,
            adapter_ref.inner.as_ref(),
            &ctx,
        ) {
            Ok(data) => {
                let response = Response::ok("wait", data);
                match serde_json::to_string(&response) {
                    Ok(json) => {
                        let ptr = string_to_c(&json);
                        if ptr.is_null() {
                            error::set_last_error(&AdapterError::new(
                                ErrorCode::Internal,
                                "failed to allocate output string",
                            ));
                            AdResult::ErrInternal
                        } else {
                            unsafe { *out = ptr };
                            AdResult::Ok
                        }
                    }
                    Err(_) => {
                        error::set_last_error(&AdapterError::new(
                            ErrorCode::Internal,
                            "failed to serialize wait response",
                        ));
                        AdResult::ErrInternal
                    }
                }
            }
            Err(app_err) => {
                let adapter_err = app_error_to_adapter_error(app_err);
                error::set_last_error(&adapter_err);
                error::last_error_code()
            }
        }
    })
}
