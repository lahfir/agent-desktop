use crate::convert::string::{c_to_string, free_c_string, string_to_c};
use crate::error::{self, AdResult};
use crate::ffi_try::{trap_panic, trap_panic_void};
use crate::AdAdapter;
use std::os::raw::c_char;

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `out` must be a non-null pointer to a `*mut c_char` to receive the allocated string.
/// Free the result with `ad_free_string`.
#[no_mangle]
pub unsafe extern "C" fn ad_get_clipboard(
    adapter: *const AdAdapter,
    out: *mut *mut c_char,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::main_thread::debug_assert_main_thread();
        let adapter = &*adapter;
        match adapter.inner.get_clipboard() {
            Ok(text) => {
                *out = string_to_c(&text);
                AdResult::Ok
            }
            Err(e) => {
                error::set_last_error(&e);
                error::last_error_code()
            }
        }
    })
}

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `text` must be a non-null, valid UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn ad_set_clipboard(
    adapter: *const AdAdapter,
    text: *const c_char,
) -> AdResult {
    trap_panic(|| unsafe {
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

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
#[no_mangle]
pub unsafe extern "C" fn ad_clear_clipboard(adapter: *const AdAdapter) -> AdResult {
    trap_panic(|| unsafe {
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

/// # Safety
///
/// `s` must be a pointer previously returned by `ad_get_clipboard`, or null.
/// After this call the pointer is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn ad_free_string(s: *mut c_char) {
    trap_panic_void(|| unsafe { free_c_string(s) })
}
