use crate::convert::window::free_window_info_fields;
use crate::ffi_try::trap_panic_void;
use crate::types::{AdExactWindowInfo, AdWindowInfo};

/// Releases the heap-allocated string fields (`id`, `title`, `app_name`)
/// inside a single `AdWindowInfo` previously written by `ad_launch_app`
/// or returned through a list accessor. Does not free the `AdWindowInfo`
/// struct itself — that memory is owned by the caller's stack or by the
/// enclosing list.
///
/// Named `ad_release_window_fields` (not `ad_free_window`) to disambiguate
/// from the now-removed list-free function and make the semantics clear
/// in the header.
///
/// # Safety
/// `win` must be null or point to a valid `AdWindowInfo` whose string
/// fields were allocated by this crate. Do not call on pointers inside
/// an `AdWindowList` — free the list instead.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_release_window_fields(win: *mut AdWindowInfo) {
    trap_panic_void(|| unsafe {
        if win.is_null() {
            return;
        }
        free_window_info_fields(&mut *win);
    })
}

/// Releases every owned string inside one exact window value.
///
/// # Safety
/// `win` must be null or point to a value written by `ad_launch_app_exact`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_release_exact_window_fields(win: *mut AdExactWindowInfo) {
    trap_panic_void(|| unsafe {
        if let Some(window) = win.as_mut() {
            crate::convert::window::free_exact_window_info_fields(window);
        }
    })
}
