use crate::convert::app::{app_info_to_c, free_app_info_fields};
use crate::error::{clear_last_error, set_last_error, AdResult};
use crate::ffi_try::{trap_panic, trap_panic_void};
use crate::types::AdAppInfo;
use crate::AdAdapter;
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
    trap_panic(|| unsafe {
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
    })
}

/// # Safety
/// `apps` must be null or a pointer previously returned by `ad_list_apps`.
#[no_mangle]
pub unsafe extern "C" fn ad_free_apps(apps: *mut AdAppInfo, count: u32) {
    trap_panic_void(|| unsafe {
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
    })
}
