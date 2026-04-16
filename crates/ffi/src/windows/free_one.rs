use crate::convert::window::free_window_info_fields;
use crate::ffi_try::trap_panic_void;
use crate::types::AdWindowInfo;

/// # Safety
/// `win` must be null or point to a valid `AdWindowInfo`.
#[no_mangle]
pub unsafe extern "C" fn ad_free_window(win: *mut AdWindowInfo) {
    trap_panic_void(|| unsafe {
        if win.is_null() {
            return;
        }
        free_window_info_fields(&mut *win);
    })
}
