use crate::actions::result::action_result_to_c;
use crate::convert::string::{c_to_string, decode_optional_filter};
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::AdActionResult;
use crate::AdAdapter;
use agent_desktop_core::notification::NotificationIdentity;
use std::os::raw::c_char;

/// Triggers the named action on the notification at `index`. Typical
/// action names are those reported in `AdNotificationInfo.actions`
/// (e.g. `"Reply"`, `"Open"`).
///
/// ## Identity / reorder safety
///
/// Notification Center reindexes entries on every listing — a new
/// notification arriving (or another one being dismissed) shifts which
/// notification sits at any given `index`. Calling this function with
/// an index obtained from a prior `ad_list_notifications` can therefore
/// press the action button on a different notification than the host
/// intended.
///
/// `expected_app` and `expected_title` let the host pin the targeted
/// notification to an observed fingerprint. If either pointer is
/// non-null, the row currently at `index` must match that field or the
/// call fails closed with `AD_RESULT_ERR_NOTIFICATION_NOT_FOUND`. Both
/// null preserves the legacy index-only behavior for hosts that do
/// their own reconciliation.
///
/// # Safety
/// `adapter` must be valid. `action_name` must be a non-null UTF-8
/// C string. `expected_app` and `expected_title` must each be null
/// or a NUL-terminated UTF-8 C string. Invalid UTF-8 in either field
/// is rejected with `AD_RESULT_ERR_INVALID_ARGS` rather than silently
/// treated as "no fingerprint". `out` must be a valid writable
/// `*mut AdActionResult`; on error it is zero-initialized.
#[no_mangle]
pub unsafe extern "C" fn ad_notification_action(
    adapter: *const AdAdapter,
    index: u32,
    expected_app: *const c_char,
    expected_title: *const c_char,
    action_name: *const c_char,
    out: *mut AdActionResult,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = std::mem::zeroed();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = &*adapter;
        let action = match c_to_string(action_name) {
            Some(s) => s,
            None => {
                set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "action_name is null or invalid UTF-8",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        let expected_app = decode_optional_filter!(expected_app, "expected_app");
        let expected_title = decode_optional_filter!(expected_title, "expected_title");
        let identity = if expected_app.is_some() || expected_title.is_some() {
            Some(NotificationIdentity {
                expected_app,
                expected_title,
            })
        } else {
            None
        };
        match adapter
            .inner
            .notification_action(index as usize, identity.as_ref(), &action)
        {
            Ok(result) => {
                *out = action_result_to_c(&result);
                AdResult::Ok
            }
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
