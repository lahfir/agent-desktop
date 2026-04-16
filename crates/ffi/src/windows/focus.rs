use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::AdWindowInfo;
use crate::windows::to_core::ad_window_to_core;
use crate::AdAdapter;

/// Brings `win` to the foreground on the current space. Returns
/// `AD_RESULT_ERR_WINDOW_NOT_FOUND` when the referenced window no longer
/// exists (the caller should re-list and retry).
///
/// # Safety
/// `adapter` must be a non-null pointer from `ad_adapter_create`. `win`
/// must be a non-null pointer to an `AdWindowInfo` whose `id` and
/// `title` fields are non-null, valid UTF-8 C strings.
#[no_mangle]
pub unsafe extern "C" fn ad_focus_window(
    adapter: *const AdAdapter,
    win: *const AdWindowInfo,
) -> AdResult {
    trap_panic(|| unsafe {
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(win, c"win is null");
        let adapter = &*adapter;
        let core_win = match ad_window_to_core(&*win) {
            Ok(w) => w,
            Err(e) => {
                set_last_error(&e);
                return crate::error::last_error_code();
            }
        };
        match adapter.inner.focus_window(&core_win) {
            Ok(()) => AdResult::Ok,
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
