use crate::convert::surface::{free_surface_info_fields, surface_info_to_c};
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::{trap_panic, trap_panic_void};
use crate::types::{AdSurfaceInfo, AdSurfaceList};
use crate::AdAdapter;
use std::ptr;

/// # Safety
/// `adapter` must be valid. `out` must be a valid writable
/// `*mut *mut AdSurfaceList`. Success produces a list handle freed via
/// `ad_surface_list_free`.
#[no_mangle]
pub unsafe extern "C" fn ad_list_surfaces(
    adapter: *const AdAdapter,
    pid: i32,
    out: *mut *mut AdSurfaceList,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = ptr::null_mut();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = &*adapter;
        match adapter.inner.list_surfaces(pid) {
            Ok(surfaces) => {
                let items: Vec<AdSurfaceInfo> = surfaces.iter().map(surface_info_to_c).collect();
                let list = Box::new(AdSurfaceList {
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
/// `list` must be null or a pointer returned by `ad_list_surfaces`.
#[no_mangle]
pub unsafe extern "C" fn ad_surface_list_count(list: *const AdSurfaceList) -> u32 {
    if list.is_null() {
        return 0;
    }
    let list_ref: &AdSurfaceList = unsafe { &*list };
    list_ref.items.len() as u32
}

/// Borrow a surface info entry. Null if `index` is out of range.
///
/// # Safety
/// `list` must be null or a pointer returned by `ad_list_surfaces`.
#[no_mangle]
pub unsafe extern "C" fn ad_surface_list_get(
    list: *const AdSurfaceList,
    index: u32,
) -> *const AdSurfaceInfo {
    if list.is_null() {
        return ptr::null();
    }
    let list_ref: &AdSurfaceList = unsafe { &*list };
    match list_ref.items.get(index as usize) {
        Some(item) => item as *const AdSurfaceInfo,
        None => ptr::null(),
    }
}

/// Frees the list and each entry's interior strings.
///
/// # Safety
/// `list` must be null or a pointer returned by `ad_list_surfaces`.
#[no_mangle]
pub unsafe extern "C" fn ad_surface_list_free(list: *mut AdSurfaceList) {
    trap_panic_void(|| unsafe {
        if list.is_null() {
            return;
        }
        let mut list = Box::from_raw(list);
        for item in list.items.iter_mut() {
            free_surface_info_fields(item);
        }
    })
}
