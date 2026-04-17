use crate::convert::string::{c_to_string, string_to_c_lossy};
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::AdNativeHandle;
use crate::AdAdapter;
use agent_desktop_core::adapter::NativeHandle;
use std::os::raw::c_char;

/// Reads a single property off a previously-resolved element handle.
///
/// Supported properties:
/// - `"value"`  — live textual value (text fields, sliders, progress
///   indicators). Null out-string when the element has no value.
/// - `"bounds"` — JSON-encoded `{"x":..,"y":..,"width":..,"height":..}`.
///   Null out-string when bounds are unavailable.
///
/// The returned string must be freed with `ad_free_string`.
///
/// # Safety
/// `adapter` must be valid. `handle` must be a non-null `AdNativeHandle`.
/// `property` must be a non-null UTF-8 C string. `out` must be a valid
/// writable `*mut *mut c_char`; it is null-initialized on entry.
#[no_mangle]
pub unsafe extern "C" fn ad_get(
    adapter: *const AdAdapter,
    handle: *const AdNativeHandle,
    property: *const c_char,
    out: *mut *mut c_char,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = std::ptr::null_mut();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(handle, c"handle is null");
        let adapter = &*adapter;
        let raw = (*handle).ptr;
        if raw.is_null() {
            set_last_error(&agent_desktop_core::error::AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                "handle.ptr is null — the handle has already been freed or was never resolved",
            ));
            return AdResult::ErrInvalidArgs;
        }
        let native = NativeHandle::from_ptr(raw);
        let prop = match c_to_string(property) {
            Some(s) => s,
            None => {
                set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "property is null or invalid UTF-8",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };

        match prop.as_str() {
            "value" => match adapter.inner.get_live_value(&native) {
                Ok(Some(v)) => {
                    *out = string_to_c_lossy(&v);
                    AdResult::Ok
                }
                Ok(None) => AdResult::Ok,
                Err(e) => {
                    set_last_error(&e);
                    crate::error::last_error_code()
                }
            },
            "bounds" => match adapter.inner.get_element_bounds(&native) {
                Ok(Some(r)) => {
                    let json = format!(
                        "{{\"x\":{},\"y\":{},\"width\":{},\"height\":{}}}",
                        r.x, r.y, r.width, r.height
                    );
                    *out = string_to_c_lossy(&json);
                    AdResult::Ok
                }
                Ok(None) => AdResult::Ok,
                Err(e) => {
                    set_last_error(&e);
                    crate::error::last_error_code()
                }
            },
            _ => {
                set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "unknown property — expected one of: value, bounds",
                ));
                AdResult::ErrInvalidArgs
            }
        }
    })
}
