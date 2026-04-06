use crate::convert::{free_surface_info_fields, surface_info_to_c};
use crate::error::{clear_last_error, set_last_error, AdResult};
use crate::types::AdSurfaceInfo;
use crate::AdAdapter;
use std::ptr;

/// # Safety
/// `adapter` must be valid. `out` and `out_count` must be writable.
#[no_mangle]
pub unsafe extern "C" fn ad_list_surfaces(
    adapter: *const AdAdapter,
    pid: i32,
    out: *mut *mut AdSurfaceInfo,
    out_count: *mut u32,
) -> AdResult {
    *out = ptr::null_mut();
    *out_count = 0;

    let adapter = &*adapter;
    match adapter.inner.list_surfaces(pid) {
        Ok(surfaces) => {
            clear_last_error();
            let c_surfaces: Vec<AdSurfaceInfo> =
                surfaces.iter().map(surface_info_to_c).collect();
            let count = c_surfaces.len() as u32;
            if c_surfaces.is_empty() {
                return AdResult::Ok;
            }
            let mut boxed = c_surfaces.into_boxed_slice();
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
/// `surfaces` must be null or from `ad_list_surfaces`.
#[no_mangle]
pub unsafe extern "C" fn ad_free_surfaces(surfaces: *mut AdSurfaceInfo, count: u32) {
    if surfaces.is_null() {
        return;
    }
    let slice = std::slice::from_raw_parts_mut(surfaces, count as usize);
    for s in slice.iter_mut() {
        free_surface_info_fields(s);
    }
    drop(Box::from_raw(
        std::ptr::slice_from_raw_parts_mut(surfaces, count as usize),
    ));
}
