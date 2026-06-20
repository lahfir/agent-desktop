mod common;

use common::{
    AdNativeHandle, AdResult, AdWindowInfo, AdWindowList, ad_launch_app, ad_list_windows,
    ad_resolve_element, c_char, default_ref_entry, with_adapter,
};

#[test]
fn invalid_utf8_filter_rejected_not_silently_widened() {
    with_adapter(|adapter| unsafe {
        let bad: [u8; 2] = [0xC3, 0x00];
        let mut list: *mut AdWindowList = std::ptr::null_mut();
        let rc = ad_list_windows(adapter, bad.as_ptr() as *const c_char, false, &mut list);
        assert!(matches!(
            rc,
            AdResult::ErrInvalidArgs | AdResult::ErrInternal
        ));
        assert!(list.is_null());
    });
}

#[test]
fn invalid_utf8_app_id_rejected() {
    with_adapter(|adapter| unsafe {
        let bad: [u8; 2] = [0xC3, 0];
        let mut out: AdWindowInfo = std::mem::zeroed();
        let rc = ad_launch_app(adapter, bad.as_ptr() as *const c_char, 0, &mut out);
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "must reject without UB, got {:?}",
            rc
        );
    });
}

#[test]
fn resolve_element_rejects_null_role() {
    with_adapter(|adapter| unsafe {
        let mut out = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let rc = ad_resolve_element(adapter, &default_ref_entry(), &mut out);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
        assert!(out.ptr.is_null());
    });
}

#[test]
fn resolve_element_rejects_invalid_utf8_name() {
    with_adapter(|adapter| unsafe {
        let role = std::ffi::CString::new("button").unwrap();
        let bad_name: [u8; 2] = [0xC3, 0x00];
        let mut entry = default_ref_entry();
        entry.role = role.as_ptr();
        entry.name = bad_name.as_ptr() as *const c_char;
        let mut out = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let rc = ad_resolve_element(adapter, &entry, &mut out);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
        assert!(out.ptr.is_null());
    });
}

#[test]
fn resolve_element_rejects_invalid_utf8_description() {
    with_adapter(|adapter| unsafe {
        let role = std::ffi::CString::new("button").unwrap();
        let bad_description: [u8; 2] = [0xC3, 0x00];
        let mut entry = default_ref_entry();
        entry.role = role.as_ptr();
        entry.description = bad_description.as_ptr() as *const c_char;
        let mut out = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let rc = ad_resolve_element(adapter, &entry, &mut out);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
        assert!(out.ptr.is_null());
    });
}
