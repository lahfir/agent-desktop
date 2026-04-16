use crate::convert::string::c_to_string;
use crate::convert::window::{free_window_info_fields, window_info_to_c};
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::{trap_panic, trap_panic_void};
use crate::types::AdWindowInfo;
use crate::AdAdapter;
use agent_desktop_core::adapter::WindowFilter;
use std::os::raw::c_char;
use std::ptr;

/// # Safety
/// `adapter` must be valid. `out` and `out_count` must be writable.
#[no_mangle]
pub unsafe extern "C" fn ad_list_windows(
    adapter: *const AdAdapter,
    app_filter: *const c_char,
    out: *mut *mut AdWindowInfo,
    out_count: *mut u32,
) -> AdResult {
    trap_panic(|| unsafe {
        *out = ptr::null_mut();
        *out_count = 0;
        let adapter = &*adapter;
        let filter = WindowFilter {
            focused_only: false,
            app: c_to_string(app_filter),
        };
        match adapter.inner.list_windows(&filter) {
            Ok(windows) => {
                let c_wins: Vec<AdWindowInfo> = windows.iter().map(window_info_to_c).collect();
                let count = c_wins.len() as u32;
                if c_wins.is_empty() {
                    return AdResult::Ok;
                }
                let mut boxed = c_wins.into_boxed_slice();
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
    })
}

/// # Safety
/// `windows` must be null or from `ad_list_windows`.
#[no_mangle]
pub unsafe extern "C" fn ad_free_windows(windows: *mut AdWindowInfo, count: u32) {
    trap_panic_void(|| unsafe {
        if windows.is_null() {
            return;
        }
        let slice = std::slice::from_raw_parts_mut(windows, count as usize);
        for w in slice.iter_mut() {
            free_window_info_fields(w);
        }
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            windows,
            count as usize,
        )));
    })
}
