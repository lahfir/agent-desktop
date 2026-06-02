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
    let role = unsafe { c_to_string(entry.role) }.ok_or_else(|| {
        agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            "role is null or invalid UTF-8",
        )
    })?;
    let name = unsafe { optional_string(entry.name, "name") }?;
    let value = unsafe { optional_string(entry.value, "value") }?;
    let description = unsafe { optional_string(entry.description, "description") }?;
    let states = unsafe { string_array(entry.states, entry.state_count, "states") }?;
    let available_actions = unsafe {
        string_array(
            entry.available_actions,
            entry.available_action_count,
            "available_actions",
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
    let source_surface = source_surface_from_c(entry.source_surface)?;
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
    unsafe { try_c_to_string(ptr) }.map_err(|()| {
        agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            format!("{field} is not valid UTF-8"),
        )
    })
}

unsafe fn string_array(
    ptr: *const *const std::os::raw::c_char,
    len: usize,
    field: &str,
) -> Result<Vec<String>, agent_desktop_core::error::AdapterError> {
    if len == 0 {
        return Ok(Vec::new());
    }
    if ptr.is_null() {
        return Err(agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            format!("{field} count is nonzero but pointer is null"),
        ));
    }
    let items = unsafe { std::slice::from_raw_parts(ptr, len) };
    items
        .iter()
        .enumerate()
        .map(|(index, item)| unsafe {
            c_to_string(*item).ok_or_else(|| {
                agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    format!("{field}[{index}] is null or invalid UTF-8"),
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
    if ptr.is_null() {
        return Err(agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            "path_count is nonzero but path pointer is null",
        ));
    }
    let mut path = smallvec::SmallVec::new();
    path.extend(
        unsafe { std::slice::from_raw_parts(ptr, len) }
            .iter()
            .map(|item| *item as usize),
    );
    Ok(path)
}

fn source_surface_from_c(
    raw: i32,
) -> Result<agent_desktop_core::adapter::SnapshotSurface, agent_desktop_core::error::AdapterError> {
    match raw {
        0 => Ok(agent_desktop_core::adapter::SnapshotSurface::Window),
        1 => Ok(agent_desktop_core::adapter::SnapshotSurface::Focused),
        2 => Ok(agent_desktop_core::adapter::SnapshotSurface::Menu),
        3 => Ok(agent_desktop_core::adapter::SnapshotSurface::Menubar),
        4 => Ok(agent_desktop_core::adapter::SnapshotSurface::Sheet),
        5 => Ok(agent_desktop_core::adapter::SnapshotSurface::Popover),
        6 => Ok(agent_desktop_core::adapter::SnapshotSurface::Alert),
        _ => Err(agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            "invalid source_surface discriminant",
        )),
    }
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
