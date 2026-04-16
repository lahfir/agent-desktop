use crate::error::{clear_last_error, set_last_error, AdResult};
use crate::types::AdWindowInfo;
use crate::windows::to_core::ad_window_to_core;
use crate::AdAdapter;

/// # Safety
/// `adapter` and `win` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn ad_focus_window(
    adapter: *const AdAdapter,
    win: *const AdWindowInfo,
) -> AdResult {
    let adapter = &*adapter;
    let core_win = ad_window_to_core(&*win);
    match adapter.inner.focus_window(&core_win) {
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
