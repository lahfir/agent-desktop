use crate::convert::string::{free_c_string, opt_string_to_c, string_to_c_lossy};
use crate::types::AdNotificationInfo;
use agent_desktop_core::notification::NotificationInfo;
use std::os::raw::c_char;
use std::ptr;

pub(crate) fn notification_info_to_c(info: &NotificationInfo) -> AdNotificationInfo {
    let (actions_ptr, action_count) = strings_to_c_array(&info.actions);
    AdNotificationInfo {
        index: info.index as u32,
        app_name: string_to_c_lossy(&info.app_name),
        title: string_to_c_lossy(&info.title),
        body: opt_string_to_c(info.body.as_deref()),
        actions: actions_ptr,
        action_count,
    }
}

pub(crate) unsafe fn free_notification_info_fields(info: &mut AdNotificationInfo) {
    free_c_string(info.app_name as *mut c_char);
    free_c_string(info.title as *mut c_char);
    free_c_string(info.body as *mut c_char);
    free_c_string_array(info.actions, info.action_count);
    info.app_name = ptr::null();
    info.title = ptr::null();
    info.body = ptr::null();
    info.actions = ptr::null_mut();
    info.action_count = 0;
}

fn strings_to_c_array(strings: &[String]) -> (*mut *mut c_char, u32) {
    if strings.is_empty() {
        return (ptr::null_mut(), 0);
    }
    let ptrs: Vec<*mut c_char> = strings.iter().map(|s| string_to_c_lossy(s)).collect();
    let count = ptrs.len() as u32;
    let mut boxed = ptrs.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    (ptr, count)
}

unsafe fn free_c_string_array(arr: *mut *mut c_char, count: u32) {
    if arr.is_null() {
        return;
    }
    let slice = std::slice::from_raw_parts_mut(arr, count as usize);
    for p in slice.iter_mut() {
        free_c_string(*p);
    }
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
        arr,
        count as usize,
    )));
}
