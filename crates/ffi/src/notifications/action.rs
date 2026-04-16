use crate::actions::result::action_result_to_c;
use crate::convert::string::c_to_string;
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::AdActionResult;
use crate::AdAdapter;
use std::os::raw::c_char;

/// Triggers the named action on the notification at `index`. Typical
/// action names are those reported in `AdNotificationInfo.actions`
/// (e.g. `"Reply"`, `"Open"`).
///
/// # Safety
/// `adapter` must be valid. `action_name` must be a non-null UTF-8
/// C string. `out` must be a valid writable `*mut AdActionResult`;
/// on error it is zero-initialized.
#[no_mangle]
pub unsafe extern "C" fn ad_notification_action(
    adapter: *const AdAdapter,
    index: u32,
    action_name: *const c_char,
    out: *mut AdActionResult,
) -> AdResult {
    trap_panic(|| unsafe {
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        *out = std::mem::zeroed();
        let adapter = &*adapter;
        let action = match c_to_string(action_name) {
            Some(s) => s,
            None => {
                set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "action_name is null or invalid UTF-8",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        match adapter.inner.notification_action(index as usize, &action) {
            Ok(result) => {
                *out = action_result_to_c(&result);
                AdResult::Ok
            }
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
