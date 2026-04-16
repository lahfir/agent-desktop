use crate::actions::conversion::action_from_c;
use crate::actions::result::action_result_to_c;
use crate::error::{self, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::{AdAction, AdActionResult, AdNativeHandle};
use crate::AdAdapter;
use agent_desktop_core::adapter::NativeHandle;

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `handle` must be a non-null pointer to a valid `AdNativeHandle`.
/// `action` must be a non-null pointer to a valid `AdAction`.
/// `out` must be a non-null pointer to an `AdActionResult` to write the result into.
#[no_mangle]
pub unsafe extern "C" fn ad_execute_action(
    adapter: *const AdAdapter,
    handle: *const AdNativeHandle,
    action: *const AdAction,
    out: *mut AdActionResult,
) -> AdResult {
    trap_panic(|| unsafe {
        *out = std::mem::zeroed();
        let adapter = &*adapter;
        let handle_ref = &*handle;
        let action_ref = &*action;
        let core_action = match action_from_c(action_ref) {
            Ok(a) => a,
            Err(msg) => {
                error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    msg,
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        let native_handle = NativeHandle::from_ptr(handle_ref.ptr);
        match adapter.inner.execute_action(&native_handle, core_action) {
            Ok(result) => {
                *out = action_result_to_c(&result);
                AdResult::Ok
            }
            Err(e) => {
                error::set_last_error(&e);
                error::last_error_code()
            }
        }
    })
}
