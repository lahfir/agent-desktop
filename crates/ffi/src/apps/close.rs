use crate::convert::string::c_to_string;
use crate::error::{clear_last_error, set_last_error, AdResult};
use crate::AdAdapter;
use std::os::raw::c_char;

/// # Safety
/// `adapter` must be valid. `id` must be a valid C string.
#[no_mangle]
pub unsafe extern "C" fn ad_close_app(
    adapter: *const AdAdapter,
    id: *const c_char,
    force: bool,
) -> AdResult {
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
        Ok(()) => {
            clear_last_error();
            AdResult::Ok
        }
        Err(e) => {
            set_last_error(&e);
            crate::error::last_error_code()
        }
    }
}
