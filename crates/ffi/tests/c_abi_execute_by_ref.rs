mod common;

use common::{AdResult, CStr, ad_execute_by_ref, ad_free_string, default_action, with_adapter};

#[test]
fn execute_by_ref_null_out_returns_invalid_args() {
    with_adapter(|adapter| unsafe {
        let ref_id = std::ffi::CString::new("@e1").unwrap();
        let action = default_action();
        let rc = ad_execute_by_ref(
            adapter,
            ref_id.as_ptr(),
            std::ptr::null(),
            &action,
            0,
            std::ptr::null_mut(),
        );
        assert_eq!(
            rc,
            AdResult::ErrInvalidArgs,
            "null out is rejected by the outer guard before any thread or adapter check"
        );
    });
}

#[test]
fn execute_by_ref_null_adapter_rejected() {
    unsafe {
        let ref_id = std::ffi::CString::new("@e1").unwrap();
        let action = default_action();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(
            std::ptr::null(),
            ref_id.as_ptr(),
            std::ptr::null(),
            &action,
            0,
            &mut out,
        );
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "null adapter must fail — got {rc:?} (ErrInternal on macOS off-main-thread is expected)"
        );
        assert!(out.is_null(), "out must stay null on failure");
    }
}

#[test]
fn execute_by_ref_null_ref_id_returns_invalid_args() {
    with_adapter(|adapter| unsafe {
        let action = default_action();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(
            adapter,
            std::ptr::null(),
            std::ptr::null(),
            &action,
            0,
            &mut out,
        );
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "null ref_id must fail — got {rc:?}"
        );
        assert!(out.is_null(), "out must stay null on null ref_id");
    });
}

#[test]
fn execute_by_ref_invalid_utf8_ref_id_returns_invalid_args() {
    with_adapter(|adapter| unsafe {
        let bad: [u8; 3] = [0xC3, 0xFF, 0x00];
        let action = default_action();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(
            adapter,
            bad.as_ptr() as *const std::os::raw::c_char,
            std::ptr::null(),
            &action,
            0,
            &mut out,
        );
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "invalid UTF-8 ref_id must fail — got {rc:?}"
        );
        assert!(out.is_null(), "out must stay null on invalid UTF-8 ref_id");
    });
}

#[test]
fn execute_by_ref_null_action_rejected() {
    with_adapter(|adapter| unsafe {
        let ref_id = std::ffi::CString::new("@e1").unwrap();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(
            adapter,
            ref_id.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            &mut out,
        );
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "null action must fail — got {rc:?}"
        );
        assert!(out.is_null(), "out must stay null on null action");
    });
}

#[test]
fn execute_by_ref_invalid_ref_format_returns_invalid_args() {
    with_adapter(|adapter| unsafe {
        let bad_ref = std::ffi::CString::new("@e0").unwrap();
        let action = default_action();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(
            adapter,
            bad_ref.as_ptr(),
            std::ptr::null(),
            &action,
            0,
            &mut out,
        );
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "bad ref format must fail — got {rc:?}"
        );
        assert!(out.is_null(), "out must stay null on bad ref format");
    });
}

#[test]
fn execute_by_ref_returns_error_envelope_when_no_refmap_exists() {
    with_adapter(|adapter| unsafe {
        let ref_id = std::ffi::CString::new("@e1").unwrap();
        let action = default_action();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(
            adapter,
            ref_id.as_ptr(),
            std::ptr::null(),
            &action,
            0,
            &mut out,
        );
        let rc_i32 = rc as i32;
        assert!(
            rc_i32 <= 0,
            "must return a valid AdResult (<=0), got {rc_i32}"
        );
        if !out.is_null() {
            let s = CStr::from_ptr(out).to_string_lossy();
            assert!(
                s.contains("\"version\""),
                "envelope must carry 'version', got: {s}"
            );
            assert!(s.contains("\"ok\""), "envelope must carry 'ok', got: {s}");
            assert!(
                s.contains("\"command\""),
                "envelope must carry 'command', got: {s}"
            );
            ad_free_string(out);
        }
    });
}
