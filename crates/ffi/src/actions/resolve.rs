use crate::convert::string::c_to_str;
use crate::error::{self, AdResult};
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
    let adapter = &*adapter;
    let entry = &*entry;
    let role = match c_to_str(entry.role) {
        Some(s) => s.to_owned(),
        None => {
            error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                "role is null or invalid UTF-8",
            ));
            return AdResult::ErrInvalidArgs;
        }
    };
    let name = c_to_str(entry.name).map(|s| s.to_owned());
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
    };
    match adapter.inner.resolve_element(&core_entry) {
        Ok(handle) => {
            (*out).ptr = handle.as_raw();
            error::clear_last_error();
            AdResult::Ok
        }
        Err(e) => {
            error::set_last_error(&e);
            error::last_error_code()
        }
    }
}
