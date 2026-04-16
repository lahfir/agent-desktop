use crate::convert::string::c_to_string;
use crate::convert::window::window_info_to_c;
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::AdWindowInfo;
use crate::AdAdapter;
use std::os::raw::c_char;

/// # Safety
/// `adapter` must be valid. `id` must be a valid C string. `out` must be writable.
#[no_mangle]
pub unsafe extern "C" fn ad_launch_app(
    adapter: *const AdAdapter,
    id: *const c_char,
    timeout_ms: u64,
    out: *mut AdWindowInfo,
) -> AdResult {
    trap_panic(|| unsafe {
        *out = std::mem::zeroed();
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
