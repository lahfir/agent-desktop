use crate::AdAdapter;
use crate::convert::string::try_c_to_string;
use crate::convert::surface::snapshot_surface_from_c;
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
        match adapter.inner.resolve_element_strict(&core_entry) {
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

pub(crate) unsafe fn core_ref_entry_from_ffi(
    entry: &AdRefEntry,
) -> Result<CoreRefEntry, agent_desktop_core::error::AdapterError> {
    let role = unsafe { optional_string(entry.role, "role") }?.ok_or_else(|| {
        agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            "role is null",
        )
    })?;
    let name = unsafe { optional_string(entry.name, "name") }?;
    let value = unsafe { optional_string(entry.value, "value") }?;
    let description = unsafe { optional_string(entry.description, "description") }?;
    let states = unsafe {
        string_array(
            entry.states,
            entry.state_count,
            "states",
            "AD_MAX_REF_STATES",
            crate::types::ref_entry::AD_MAX_REF_STATES,
        )
    }?;
    let available_actions = unsafe {
        string_array(
            entry.available_actions,
            entry.available_action_count,
            "available_actions",
            "AD_MAX_REF_ACTIONS",
            crate::types::ref_entry::AD_MAX_REF_ACTIONS,
        )
    }?;
    let bounds = if entry.has_bounds {
        Some(agent_desktop_core::node::Rect {
            x: entry.bounds.x,
            y: entry.bounds.y,
            width: entry.bounds.width,
            height: entry.bounds.height,
        })
    } else {
        None
    };
    let bounds_hash = if entry.has_bounds_hash {
        Some(entry.bounds_hash)
    } else {
        None
    };
    let source_surface = snapshot_surface_from_c(entry.source_surface, "source_surface")?;
    let path = unsafe { ref_path(entry.path, entry.path_count)? };

    Ok(CoreRefEntry {
        pid: entry.pid,
        role,
        name,
        value,
        description,
        states,
        bounds,
        bounds_hash,
        available_actions,
        source_app: unsafe { optional_string(entry.source_app, "source_app") }?,
        source_window_id: unsafe { optional_string(entry.source_window_id, "source_window_id") }?,
        source_window_title: unsafe {
            optional_string(entry.source_window_title, "source_window_title")
        }?,
        source_surface,
        root_ref: unsafe { optional_string(entry.root_ref, "root_ref") }?,
        path_is_absolute: entry.path_is_absolute,
        path,
    })
}

unsafe fn optional_string(
    ptr: *const std::os::raw::c_char,
    field: &str,
) -> Result<Option<String>, agent_desktop_core::error::AdapterError> {
    unsafe { try_c_to_string(ptr) }.map_err(|err| {
        agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            err.describe(field),
        )
    })
}

fn check_array_len(
    len: usize,
    is_null: bool,
    field: &str,
    constant: &str,
    max: usize,
) -> Result<(), agent_desktop_core::error::AdapterError> {
    if len > max {
        return Err(agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            format!("{field} count {len} exceeds {constant} ({max})"),
        ));
    }
    if is_null {
        return Err(agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            format!("{field} count is nonzero but pointer is null"),
        ));
    }
    Ok(())
}

unsafe fn string_array(
    ptr: *const *const std::os::raw::c_char,
    len: usize,
    field: &str,
    constant: &str,
    max: usize,
) -> Result<Vec<String>, agent_desktop_core::error::AdapterError> {
    if len == 0 {
        return Ok(Vec::new());
    }
    check_array_len(len, ptr.is_null(), field, constant, max)?;
    let items = unsafe { std::slice::from_raw_parts(ptr, len) };
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let element = format!("{field}[{index}]");
            unsafe { optional_string(*item, &element) }?.ok_or_else(|| {
                agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    format!("{element} is null"),
                )
            })
        })
        .collect()
}

unsafe fn ref_path(
    ptr: *const u32,
    len: usize,
) -> Result<smallvec::SmallVec<[usize; 8]>, agent_desktop_core::error::AdapterError> {
    if len == 0 {
        return Ok(smallvec::SmallVec::new());
    }
    check_array_len(
        len,
        ptr.is_null(),
        "path",
        "AD_MAX_REF_PATH_DEPTH",
        crate::types::ref_entry::AD_MAX_REF_PATH_DEPTH,
    )?;
    let mut path = smallvec::SmallVec::new();
    path.extend(
        unsafe { std::slice::from_raw_parts(ptr, len) }
            .iter()
            .map(|item| *item as usize),
    );
    Ok(path)
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
