use crate::AdAdapter;
use crate::convert::window::window_info_to_c;
use crate::error::{AdResult, set_last_error};
use crate::ffi_try::trap_panic;
use crate::types::AdWindowInfo;
use std::os::raw::c_char;

/// Launches the application identified by `id` (bundle id on macOS,
/// executable path on other platforms) and, on success, writes the
/// first window that becomes available into `*out`. Waits up to
/// `timeout_ms` for the window to appear; zero means "no wait".
///
/// The returned `AdWindowInfo` owns heap-allocated interior strings that
/// must be released with `ad_release_window_fields` once done. On error
/// the out-param is zero-initialized, so calling the release fn on it
/// is a safe no-op.
///
/// # Safety
/// `adapter` must be non-null. `id` must be a non-null UTF-8 C string.
/// `out` must be a non-null writable `*mut AdWindowInfo`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_launch_app(
    adapter: *const AdAdapter,
    id: *const c_char,
    timeout_ms: u64,
    out: *mut AdWindowInfo,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = std::mem::zeroed();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let id_str = match super::decode_app_id(id) {
            Ok(id) => id,
            Err(err) => {
                set_last_error(&err);
                return crate::error::last_error_code();
            }
        };

        let adapter = &*adapter;
        match adapter.inner.launch_app(&id_str, timeout_ms) {
            Ok(win) => {
                *out = window_info_to_c(&win);
                AdResult::Ok
            }
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
