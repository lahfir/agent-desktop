mod common;

use common::{AdResult, CStr, ad_free_string, ad_snapshot, with_adapter};

#[test]
fn snapshot_null_out_returns_invalid_args() {
    with_adapter(|adapter| unsafe {
        let rc = ad_snapshot(
            adapter,
            std::ptr::null(),
            0,
            6,
            false,
            false,
            std::ptr::null_mut(),
        );
        assert_eq!(
            rc,
            AdResult::ErrInvalidArgs,
            "null out is rejected by the outer guard before any adapter or thread check"
        );
    });
}

#[test]
fn snapshot_null_adapter_rejected() {
    unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_snapshot(
            std::ptr::null(),
            std::ptr::null(),
            0,
            6,
            false,
            false,
            &mut out,
        );
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "null adapter must fail — got {rc:?} (ErrInternal on macOS off-main-thread is expected)"
        );
        assert!(out.is_null(), "out must stay null on null-adapter failure");
    }
}

#[test]
fn snapshot_invalid_utf8_app_rejected() {
    with_adapter(|adapter| unsafe {
        let bad: [u8; 3] = [0xC3, 0xFF, 0x00];
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_snapshot(
            adapter,
            bad.as_ptr() as *const std::os::raw::c_char,
            0,
            6,
            false,
            false,
            &mut out,
        );
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "invalid UTF-8 app must fail — got {rc:?}"
        );
        assert!(
            out.is_null(),
            "out must stay null on arg validation failure"
        );
    });
}

#[test]
fn snapshot_invalid_surface_rejected() {
    with_adapter(|adapter| unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_snapshot(adapter, std::ptr::null(), 99, 6, false, false, &mut out);
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "out-of-range surface must fail — got {rc:?}"
        );
        assert!(out.is_null(), "out must stay null on invalid surface");
    });
}

#[test]
fn snapshot_returns_a_result_code_and_frees_cleanly() {
    with_adapter(|adapter| unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_snapshot(adapter, std::ptr::null(), 0, 6, false, false, &mut out);
        let rc_i32 = rc as i32;
        assert!(
            rc_i32 <= 0,
            "ad_snapshot must return a valid AdResult (<=0), got {rc_i32}"
        );
        if !out.is_null() {
            let s = CStr::from_ptr(out).to_string_lossy();
            assert!(!s.is_empty(), "non-null out must be a non-empty string");
            ad_free_string(out);
        }
    });
}

#[test]
fn snapshot_out_contains_envelope_fields_on_any_response() {
    with_adapter(|adapter| unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_snapshot(adapter, std::ptr::null(), 0, 6, false, false, &mut out);
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
            assert!(
                s.contains("\"snapshot\""),
                "command value must be 'snapshot', got: {s}"
            );
            ad_free_string(out);
        } else {
            let rc_i32 = rc as i32;
            assert!(
                rc_i32 < 0,
                "null out is only valid on failure, got rc={rc_i32}"
            );
        }
    });
}
