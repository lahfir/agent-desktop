use crate::convert::string::{c_to_string, free_c_string, string_to_c};
use crate::error::{self, AdResult};
use crate::ffi_try::{trap_panic, trap_panic_void};
use crate::AdAdapter;
use std::os::raw::c_char;

/// Reads the current clipboard text and writes an owned C string into
/// `*out`. The caller must free the returned pointer with
/// `ad_free_string`. On error `*out` is left null.
///
/// # Safety
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `out` must be a non-null writable `*mut *mut c_char`.
#[no_mangle]
pub unsafe extern "C" fn ad_get_clipboard(
    adapter: *const AdAdapter,
    out: *mut *mut c_char,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = std::ptr::null_mut();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = &*adapter;
        match adapter.inner.get_clipboard() {
            Ok(text) => {
                let c = string_to_c(&text);
                if c.is_null() {
                    error::set_last_error(
                        &agent_desktop_core::error::AdapterError::new(
                            agent_desktop_core::error::ErrorCode::Internal,
                            "clipboard text contains an interior NUL and cannot be represented as a C string",
                        ),
                    );
                    return AdResult::ErrInternal;
                }
                *out = c;
                AdResult::Ok
            }
            Err(e) => {
                error::set_last_error(&e);
                error::last_error_code()
            }
        }
    })
}

/// Writes UTF-8 `text` to the clipboard. Null or non-UTF-8 input returns
/// `AD_RESULT_ERR_INVALID_ARGS` with a diagnostic last-error.
///
/// # Safety
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `text` must be a non-null, NUL-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn ad_set_clipboard(
    adapter: *const AdAdapter,
    text: *const c_char,
) -> AdResult {
    trap_panic(|| unsafe {
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = &*adapter;
        let text = match c_to_string(text) {
            Some(s) => s,
            None => {
                error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "text is null or invalid UTF-8",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        match adapter.inner.set_clipboard(&text) {
            Ok(()) => AdResult::Ok,
            Err(e) => {
                error::set_last_error(&e);
                error::last_error_code()
            }
        }
    })
}

/// Clears the clipboard.
///
/// # Safety
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
#[no_mangle]
pub unsafe extern "C" fn ad_clear_clipboard(adapter: *const AdAdapter) -> AdResult {
    trap_panic(|| unsafe {
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = &*adapter;
        match adapter.inner.clear_clipboard() {
            Ok(()) => AdResult::Ok,
            Err(e) => {
                error::set_last_error(&e);
                error::last_error_code()
            }
        }
    })
}

/// Frees a C string previously returned by `ad_get_clipboard` or any
/// other FFI call documented as allocating a C string for the caller.
/// Null-tolerant — safe to call on `NULL`. Double-free is undefined.
///
/// # Safety
/// `s` must be null or a pointer previously handed out by this crate.
/// After this call the pointer is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn ad_free_string(s: *mut c_char) {
    trap_panic_void(|| unsafe { free_c_string(s) })
}
