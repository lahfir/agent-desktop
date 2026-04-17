use crate::convert::string::{c_to_string, try_c_to_string};
use crate::error::{self, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::{AdNativeHandle, AdRefEntry};
use crate::AdAdapter;
use agent_desktop_core::refs::RefEntry as CoreRefEntry;

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `entry` must be a non-null pointer to a valid `AdRefEntry`.
/// `out` must be a non-null pointer to an `AdNativeHandle` to write the result into.
#[no_mangle]
pub unsafe extern "C" fn ad_resolve_element(
    adapter: *const AdAdapter,
    entry: *const AdRefEntry,
    out: *mut AdNativeHandle,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        (*out).ptr = std::ptr::null();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(entry, c"entry is null");
        let adapter = &*adapter;
        let entry = &*entry;
        let role = match c_to_string(entry.role) {
            Some(s) => s,
            None => {
                error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "role is null or invalid UTF-8",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        // Fail closed on invalid UTF-8 for `name`: conflating "null" with
        // "bad bytes" would widen a caller-specified filter into an
        // unfiltered re-resolution. try_c_to_string keeps null → None and
        // promotes invalid UTF-8 to an explicit InvalidArgs, matching the
        // optional-filter contract used by ad_list_* entrypoints.
        let name = match try_c_to_string(entry.name) {
            Ok(value) => value,
            Err(()) => {
                error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "name is not valid UTF-8",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        let bounds_hash = if entry.has_bounds_hash {
            Some(entry.bounds_hash)
        } else {
            None
        };
        let core_entry = CoreRefEntry {
            pid: entry.pid,
            role,
            name,
            value: None,
            states: vec![],
            bounds: None,
            bounds_hash,
            available_actions: vec![],
            source_app: None,
            root_ref: None,
        };
        match adapter.inner.resolve_element(&core_entry) {
            Ok(handle) => {
                (*out).ptr = handle.as_raw();
                AdResult::Ok
            }
            Err(e) => {
                error::set_last_error(&e);
                error::last_error_code()
            }
        }
    })
}
