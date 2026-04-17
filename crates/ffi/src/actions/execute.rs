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
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = std::mem::zeroed();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(handle, c"handle is null");
        crate::pointer_guard::guard_non_null!(action, c"action is null");
        let adapter = &*adapter;
        let handle_ref = &*handle;
        if handle_ref.ptr.is_null() {
            error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                "handle.ptr is null — the handle has already been freed or was never resolved",
            ));
            return AdResult::ErrInvalidArgs;
        }
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
