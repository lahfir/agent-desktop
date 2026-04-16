use crate::convert::string::decode_optional_filter;
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::AdAdapter;
use std::os::raw::c_char;

/// Dismisses the notification at `index`. Indexes are only valid within
/// the response to the most recent `ad_list_notifications` call on this
/// thread — the adapter re-queries internally, so dismissing by a stale
/// index returns `AD_RESULT_ERR_NOTIFICATION_NOT_FOUND`.
///
/// # Safety
/// `adapter` must be valid. `app_filter` may be null.
#[no_mangle]
pub unsafe extern "C" fn ad_dismiss_notification(
    adapter: *const AdAdapter,
    index: u32,
    app_filter: *const c_char,
) -> AdResult {
    trap_panic(|| unsafe {
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = &*adapter;
        let filter = decode_optional_filter!(app_filter, "app_filter");
        let filter_ref = filter.as_deref();
        match adapter
            .inner
            .dismiss_notification(index as usize, filter_ref)
        {
            Ok(_) => AdResult::Ok,
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
