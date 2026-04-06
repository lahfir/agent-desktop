use crate::convert::{app_info_to_c, c_to_str, free_app_info_fields, window_info_to_c};
use crate::error::{clear_last_error, set_last_error, AdResult};
use crate::types::{AdAppInfo, AdWindowInfo};
use crate::AdAdapter;
use std::os::raw::c_char;
use std::ptr;

/// # Safety
/// `adapter` must be a valid pointer from `ad_adapter_create`.
/// `out` and `out_count` must be valid writable pointers.
#[no_mangle]
pub unsafe extern "C" fn ad_list_apps(
    adapter: *const AdAdapter,
    out: *mut *mut AdAppInfo,
    out_count: *mut u32,
) -> AdResult {
    *out = ptr::null_mut();
    *out_count = 0;

    let adapter = &*adapter;
    match adapter.inner.list_apps() {
        Ok(apps) => {
            clear_last_error();
            let c_apps: Vec<AdAppInfo> = apps.iter().map(app_info_to_c).collect();
            let count = c_apps.len() as u32;
            if c_apps.is_empty() {
                return AdResult::Ok;
            }
            let mut boxed = c_apps.into_boxed_slice();
            *out = boxed.as_mut_ptr();
            *out_count = count;
            std::mem::forget(boxed);
            AdResult::Ok
        }
        Err(e) => {
            set_last_error(&e);
            crate::error::last_error_code()
        }
    }
}

/// # Safety
/// `apps` must be null or a pointer previously returned by `ad_list_apps`.
#[no_mangle]
pub unsafe extern "C" fn ad_free_apps(apps: *mut AdAppInfo, count: u32) {
    if apps.is_null() {
        return;
    }
    let slice = std::slice::from_raw_parts_mut(apps, count as usize);
    for app in slice.iter_mut() {
        free_app_info_fields(app);
    }
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
        apps,
        count as usize,
    )));
}

/// # Safety
/// `adapter` must be valid. `id` must be a valid C string. `out` must be writable.
#[no_mangle]
pub unsafe extern "C" fn ad_launch_app(
    adapter: *const AdAdapter,
    id: *const c_char,
    timeout_ms: u64,
    out: *mut AdWindowInfo,
) -> AdResult {
    let adapter = &*adapter;
    let id_str = match c_to_str(id) {
        Some(s) => s,
        None => {
            set_last_error(&agent_desktop_core::error::AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                "app id is null or invalid UTF-8",
            ));
            return AdResult::ErrInvalidArgs;
        }
    };

    match adapter.inner.launch_app(id_str, timeout_ms) {
        Ok(win) => {
            clear_last_error();
            *out = window_info_to_c(&win);
            AdResult::Ok
        }
        Err(e) => {
            set_last_error(&e);
            crate::error::last_error_code()
        }
    }
}

/// # Safety
/// `adapter` must be valid. `id` must be a valid C string.
#[no_mangle]
pub unsafe extern "C" fn ad_close_app(
    adapter: *const AdAdapter,
    id: *const c_char,
    force: bool,
) -> AdResult {
    let adapter = &*adapter;
    let id_str = match c_to_str(id) {
        Some(s) => s,
        None => {
            set_last_error(&agent_desktop_core::error::AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                "app id is null or invalid UTF-8",
            ));
            return AdResult::ErrInvalidArgs;
        }
    };

    match adapter.inner.close_app(id_str, force) {
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
