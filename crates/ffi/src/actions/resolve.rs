use crate::AdAdapter;
use crate::convert::string::{c_to_string, try_c_to_string};
use crate::error::{self, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::{AdNativeHandle, AdRefEntry};
use agent_desktop_core::refs::RefEntry as CoreRefEntry;
use std::mem::ManuallyDrop;

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `entry` must be a non-null pointer to a valid `AdRefEntry`.
/// `out` must be a non-null pointer to an `AdNativeHandle` to write the result into.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_resolve_element(
    adapter: *const AdAdapter,
    entry: *const AdRefEntry,
    out: *mut AdNativeHandle,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        (*out).ptr = std::ptr::null();
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(entry, c"entry is null");
        let adapter = &*adapter;
        let entry = &*entry;
        let core_entry = match core_ref_entry_from_ffi(entry) {
            Ok(entry) => entry,
            Err(err) => {
                error::set_last_error(&err);
                return error::last_error_code();
            }
        };
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        match adapter.inner.resolve_element(&core_entry) {
            Ok(handle) => {
                let handle = ManuallyDrop::new(handle);
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

unsafe fn core_ref_entry_from_ffi(
    entry: &AdRefEntry,
) -> Result<CoreRefEntry, agent_desktop_core::error::AdapterError> {
    let role = unsafe { c_to_string(entry.role) }.ok_or_else(|| {
        agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            "role is null or invalid UTF-8",
        )
    })?;
    let name = unsafe { optional_string(entry.name, "name") }?;
    let description = unsafe { optional_string(entry.description, "description") }?;
    let bounds_hash = if entry.has_bounds_hash {
        Some(entry.bounds_hash)
    } else {
        None
    };

    Ok(CoreRefEntry {
        pid: entry.pid,
        role,
        name,
        value: None,
        description,
        states: vec![],
        bounds: None,
        bounds_hash,
        available_actions: vec![],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: agent_desktop_core::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    })
}

unsafe fn optional_string(
    ptr: *const std::os::raw::c_char,
    field: &str,
) -> Result<Option<String>, agent_desktop_core::error::AdapterError> {
    unsafe { try_c_to_string(ptr) }.map_err(|()| {
        agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            format!("{field} is not valid UTF-8"),
        )
    })
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
