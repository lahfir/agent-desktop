mod common;

use common::{
    AdNativeHandle, AdResult, AdWindowInfo, AdWindowList, CStr, ad_last_error_message,
    ad_launch_app, ad_list_windows, ad_resolve_element, ad_resolve_element_exact, c_char,
    default_exact_ref_entry, default_ref_entry, with_adapter,
};

#[test]
fn invalid_utf8_filter_rejected_not_silently_widened() {
    with_adapter(|adapter| unsafe {
        let bad: [u8; 2] = [0xC3, 0x00];
        let mut list: *mut AdWindowList = std::ptr::null_mut();
        let rc = ad_list_windows(adapter, bad.as_ptr() as *const c_char, false, &mut list);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
        assert!(list.is_null());
    });
}

#[test]
fn invalid_utf8_app_id_rejected() {
    with_adapter(|adapter| unsafe {
        let bad: [u8; 2] = [0xC3, 0];
        let mut out: AdWindowInfo = std::mem::zeroed();
        let rc = ad_launch_app(adapter, bad.as_ptr() as *const c_char, 0, &mut out);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
    });
}

#[test]
fn legacy_resolve_fails_closed_before_role_validation() {
    with_adapter(|adapter| unsafe {
        let mut out = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let rc = ad_resolve_element(adapter, &default_ref_entry(), &mut out);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
        assert!(out.ptr.is_null());
        let message = CStr::from_ptr(ad_last_error_message()).to_string_lossy();
        assert!(message.contains("legacy AdRefEntry lacks"));
    });
}

#[test]
fn exact_resolve_rejects_invalid_utf8_name_at_the_name_boundary() {
    with_adapter(|adapter| unsafe {
        let role = std::ffi::CString::new("button").unwrap();
        let process_instance = std::ffi::CString::new("generation-1").unwrap();
        let bad_name: [u8; 2] = [0xC3, 0x00];
        let mut exact = default_exact_ref_entry();
        exact.process_instance = process_instance.as_ptr();
        exact.entry.process.pid = 1;
        exact.entry.identity.role = role.as_ptr();
        exact.entry.identity.name = bad_name.as_ptr() as *const c_char;
        let mut out = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let rc = ad_resolve_element_exact(adapter, &exact, &mut out);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
        assert!(out.ptr.is_null());
        let message = CStr::from_ptr(ad_last_error_message()).to_string_lossy();
        assert!(message.contains("identity.name"));
    });
}

#[test]
fn exact_resolve_rejects_invalid_utf8_description_at_the_description_boundary() {
    with_adapter(|adapter| unsafe {
        let role = std::ffi::CString::new("button").unwrap();
        let process_instance = std::ffi::CString::new("generation-1").unwrap();
        let bad_description: [u8; 2] = [0xC3, 0x00];
        let mut exact = default_exact_ref_entry();
        exact.process_instance = process_instance.as_ptr();
        exact.entry.process.pid = 1;
        exact.entry.identity.role = role.as_ptr();
        exact.entry.identity.description = bad_description.as_ptr() as *const c_char;
        let mut out = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let rc = ad_resolve_element_exact(adapter, &exact, &mut out);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
        assert!(out.ptr.is_null());
        let message = CStr::from_ptr(ad_last_error_message()).to_string_lossy();
        assert!(message.contains("identity.description"));
    });
}
