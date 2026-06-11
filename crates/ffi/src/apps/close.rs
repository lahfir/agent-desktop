use crate::AdAdapter;
use crate::convert::string::c_to_string;
use crate::error::{AdResult, set_last_error};
use crate::ffi_try::trap_panic;
use std::os::raw::c_char;

/// Closes the application identified by `id` (bundle id on macOS,
/// executable path on other platforms). `force = true` skips the
/// graceful-shutdown path (equivalent to `kill -9`). Session-critical
/// processes (loginwindow, WindowServer, Dock, Finder, launchd) are
/// refused with `AD_RESULT_ERR_INVALID_ARGS` — the protected-process
/// guard is enforced inside the adapter, so FFI and CLI behave
/// identically.
///
/// # Safety
/// `adapter` must be non-null. `id` must be a non-null UTF-8 C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_close_app(
    adapter: *const AdAdapter,
    id: *const c_char,
    force: bool,
) -> AdResult {
    trap_panic(|| unsafe {
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = &*adapter;
        let id_str = match c_to_string(id) {
            Some(s) => s,
            None => {
                set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "app id is null or invalid UTF-8",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };

        match adapter.inner.close_app(&id_str, force) {
            Ok(()) => AdResult::Ok,
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
