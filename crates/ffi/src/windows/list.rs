use crate::convert::string::decode_optional_filter;
use crate::convert::window::{free_window_info_fields, window_info_to_c};
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::{trap_panic, trap_panic_void};
use crate::types::{AdWindowInfo, AdWindowList};
use crate::AdAdapter;
use agent_desktop_core::adapter::WindowFilter;
use std::os::raw::c_char;
use std::ptr;

/// # Safety
/// `adapter` must be valid. `out` must be a valid writable
/// `*mut *mut AdWindowList`. `app_filter` may be null or a C string.
/// Success produces a list handle freed via `ad_window_list_free`.
#[no_mangle]
pub unsafe extern "C" fn ad_list_windows(
    adapter: *const AdAdapter,
    app_filter: *const c_char,
    focused_only: bool,
    out: *mut *mut AdWindowList,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = ptr::null_mut();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = &*adapter;
        let filter = WindowFilter {
            focused_only,
            app: decode_optional_filter!(app_filter, "app_filter"),
        };
        match adapter.inner.list_windows(&filter) {
            Ok(windows) => {
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
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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
