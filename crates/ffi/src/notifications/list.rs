use crate::convert::notification::{free_notification_info_fields, notification_info_to_c};
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::{trap_panic, trap_panic_void};
use crate::notifications::filter::filter_from_c;
use crate::types::{AdNotificationFilter, AdNotificationInfo, AdNotificationList};
use crate::AdAdapter;
use std::ptr;

/// Lists the notifications currently on-screen.
///
/// Notification indexes are only stable within a single list response.
/// Pass them straight to `ad_dismiss_notification` /
/// `ad_notification_action` without caching across ticks — the adapter
/// re-queries Notification Center internally on every call.
///
/// # Safety
/// `adapter` must be valid. `filter` may be null. `out` must be a valid
/// writable `*mut *mut AdNotificationList`. On success `*out` is a
/// non-null handle freed with `ad_notification_list_free`.
#[no_mangle]
pub unsafe extern "C" fn ad_list_notifications(
    adapter: *const AdAdapter,
    filter: *const AdNotificationFilter,
    out: *mut *mut AdNotificationList,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = ptr::null_mut();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = &*adapter;
        let core_filter = match filter_from_c(filter) {
            Ok(f) => f,
            Err(e) => {
                set_last_error(&e);
                return crate::error::last_error_code();
            }
        };
        match adapter.inner.list_notifications(&core_filter) {
            Ok(notifications) => {
                let items: Vec<AdNotificationInfo> =
                    notifications.iter().map(notification_info_to_c).collect();
                let list = Box::new(AdNotificationList {
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
/// `list` must be null or a pointer returned by `ad_list_notifications`.
#[no_mangle]
pub unsafe extern "C" fn ad_notification_list_count(list: *const AdNotificationList) -> u32 {
    if list.is_null() {
        return 0;
    }
    let list_ref: &AdNotificationList = unsafe { &*list };
    list_ref.items.len() as u32
}

/// Borrows a notification entry. Null if `index` is out of range.
///
/// # Safety
/// `list` must be null or a pointer returned by `ad_list_notifications`.
#[no_mangle]
pub unsafe extern "C" fn ad_notification_list_get(
    list: *const AdNotificationList,
    index: u32,
) -> *const AdNotificationInfo {
    if list.is_null() {
        return ptr::null();
    }
    let list_ref: &AdNotificationList = unsafe { &*list };
    match list_ref.items.get(index as usize) {
        Some(item) => item as *const AdNotificationInfo,
        None => ptr::null(),
    }
}

/// Frees the list and each entry's interior strings.
///
/// # Safety
/// `list` must be null or a pointer returned by `ad_list_notifications`.
#[no_mangle]
pub unsafe extern "C" fn ad_notification_list_free(list: *mut AdNotificationList) {
    trap_panic_void(|| unsafe {
        if list.is_null() {
            return;
        }
        let mut list = Box::from_raw(list);
        for item in list.items.iter_mut() {
            free_notification_info_fields(item);
        }
    })
}
