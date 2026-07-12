use crate::AdAdapter;
use crate::convert::string::decode_optional_filter;
use crate::convert::window::{
    exact_window_info_to_c, free_exact_window_info_fields, free_window_info_fields,
    validate_exact_window_info, window_info_to_c,
};
use crate::error::{AdResult, set_last_error};
use crate::ffi_try::{trap_panic, trap_panic_void};
use crate::types::{AdExactWindowInfo, AdExactWindowList, AdWindowInfo, AdWindowList};
use agent_desktop_core::adapter::WindowFilter;
use std::os::raw::c_char;
use std::ptr;

/// # Safety
/// `adapter` must be valid. `out` must be a valid writable
/// `*mut *mut AdWindowList`. `app_filter` may be null or a C string.
/// Success produces a list handle freed via `ad_window_list_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_list_windows(
    adapter: *const AdAdapter,
    app_filter: *const c_char,
    focused_only: bool,
    out: *mut *mut AdWindowList,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = ptr::null_mut();
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let filter = WindowFilter {
            focused_only,
            app: decode_optional_filter!(app_filter, "app_filter"),
        };
        let adapter = crate::adapter::acquire_adapter!(adapter);
        let deadline = crate::operation::operation_deadline!();
        match adapter.inner.list_windows(&filter, deadline) {
            Ok(windows) => {
                if let Err(error) = crate::resource::validate_list_len(windows.len(), "Window list")
                {
                    set_last_error(&error);
                    return crate::error::last_error_code();
                }
                let items: Vec<AdWindowInfo> = windows.iter().map(window_info_to_c).collect();
                let list = Box::new(AdWindowList {
                    items: items.into_boxed_slice(),
                });
                *out = Box::into_raw(list);
                AdResult::Ok
            }
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}

/// # Safety
/// `list` must be null or a pointer returned by `ad_list_windows`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_window_list_count(list: *const AdWindowList) -> u32 {
    if list.is_null() {
        return 0;
    }
    let list_ref: &AdWindowList = unsafe { &*list };
    list_ref.items.len() as u32
}

/// Borrow a window info entry. Null if `index` is out of range.
///
/// # Safety
/// `list` must be null or a pointer returned by `ad_list_windows`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_window_list_get(
    list: *const AdWindowList,
    index: u32,
) -> *const AdWindowInfo {
    if list.is_null() {
        return ptr::null();
    }
    let list_ref: &AdWindowList = unsafe { &*list };
    match list_ref.items.get(index as usize) {
        Some(item) => item as *const AdWindowInfo,
        None => ptr::null(),
    }
}

/// Frees the list and each entry's interior strings.
///
/// # Safety
/// `list` must be null or a pointer returned by `ad_list_windows`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_window_list_free(list: *mut AdWindowList) {
    trap_panic_void(|| unsafe {
        if list.is_null() {
            return;
        }
        let mut list = Box::from_raw(list);
        for item in list.items.iter_mut() {
            free_window_info_fields(item);
        }
    })
}

/// Lists windows with explicit process-generation evidence.
///
/// # Safety
/// `adapter` and `out` must be valid. `app_filter` may be null or a valid
/// bounded UTF-8 C string. The returned list must be freed with
/// `ad_exact_window_list_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_list_windows_exact(
    adapter: *const AdAdapter,
    app_filter: *const c_char,
    focused_only: bool,
    out: *mut *mut AdExactWindowList,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = ptr::null_mut();
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let filter = WindowFilter {
            focused_only,
            app: decode_optional_filter!(app_filter, "app_filter"),
        };
        let adapter = crate::adapter::acquire_adapter!(adapter);
        let deadline = crate::operation::operation_deadline!();
        match adapter.inner.list_windows(&filter, deadline) {
            Ok(windows) => {
                if let Err(error) =
                    crate::resource::validate_list_len(windows.len(), "Exact window list")
                {
                    set_last_error(&error);
                    return crate::error::last_error_code();
                }
                if let Err(error) = windows.iter().try_for_each(validate_exact_window_info) {
                    set_last_error(&error);
                    return crate::error::last_error_code();
                }
                let items: Vec<AdExactWindowInfo> =
                    windows.iter().map(exact_window_info_to_c).collect();
                *out = Box::into_raw(Box::new(AdExactWindowList {
                    items: items.into_boxed_slice(),
                }));
                AdResult::Ok
            }
            Err(error) => {
                set_last_error(&error);
                crate::error::last_error_code()
            }
        }
    })
}

/// # Safety
/// `list` must be null or returned by `ad_list_windows_exact`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_exact_window_list_count(list: *const AdExactWindowList) -> u32 {
    if list.is_null() {
        return 0;
    }
    unsafe { &*list }.items.len() as u32
}

/// # Safety
/// `list` must be null or returned by `ad_list_windows_exact`. The returned
/// pointer is borrowed until the list is freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_exact_window_list_get(
    list: *const AdExactWindowList,
    index: u32,
) -> *const AdExactWindowInfo {
    if list.is_null() {
        return ptr::null();
    }
    unsafe { &*list }
        .items
        .get(index as usize)
        .map_or(ptr::null(), std::ptr::from_ref)
}

/// # Safety
/// `list` must be null or returned by `ad_list_windows_exact`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_exact_window_list_free(list: *mut AdExactWindowList) {
    trap_panic_void(|| unsafe {
        if list.is_null() {
            return;
        }
        let mut list = Box::from_raw(list);
        for item in &mut list.items {
            free_exact_window_info_fields(item);
        }
    })
}
