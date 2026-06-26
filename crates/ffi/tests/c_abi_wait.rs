mod common;

use common::{
    AdResult, AdWaitArgs, CStr, ad_free_string, ad_last_error_code, ad_last_error_message, ad_wait,
    with_adapter,
};

#[test]
fn ad_wait_null_args_rejected() {
    with_adapter(|adapter| unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_wait(adapter, std::ptr::null(), &mut out);
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "null args must be rejected, got {:?}",
            rc
        );
        assert!(out.is_null(), "out must stay null on null-args rejection");
        assert_eq!(
            ad_last_error_code(),
            rc,
            "last-error code must match returned AdResult (errno invariant)"
        );
    });
}

#[test]
fn ad_wait_null_out_rejected() {
    with_adapter(|adapter| unsafe {
        let args = AdWaitArgs {
            ms: 1,
            has_ms: true,
            element: std::ptr::null(),
            window: std::ptr::null(),
            text: std::ptr::null(),
            menu: false,
            menu_closed: false,
            notification: false,
            snapshot_id: std::ptr::null(),
            predicate: std::ptr::null(),
            value: std::ptr::null(),
            action: std::ptr::null(),
            count: 0,
            has_count: false,
            timeout_ms: 500,
            app: std::ptr::null(),
        };
        let rc = ad_wait(adapter, &args, std::ptr::null_mut());
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "null out must be rejected, got {:?}",
            rc
        );
        assert_eq!(
            ad_last_error_code(),
            rc,
            "last-error code must match returned AdResult (errno invariant)"
        );
    });
}

#[test]
fn ad_wait_ms_mode_returns_ok_or_off_thread_error() {
    with_adapter(|adapter| unsafe {
        let args = AdWaitArgs {
            ms: 50,
            has_ms: true,
            element: std::ptr::null(),
            window: std::ptr::null(),
            text: std::ptr::null(),
            menu: false,
            menu_closed: false,
            notification: false,
            snapshot_id: std::ptr::null(),
            predicate: std::ptr::null(),
            value: std::ptr::null(),
            action: std::ptr::null(),
            count: 0,
            has_count: false,
            timeout_ms: 500,
            app: std::ptr::null(),
        };
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_wait(adapter, &args, &mut out);

        match rc {
            AdResult::Ok => {
                assert!(!out.is_null(), "Ok result must set out");
                let json_cstr = CStr::from_ptr(out);
                let json: serde_json::Value =
                    serde_json::from_str(json_cstr.to_str().unwrap()).unwrap();
                assert_eq!(json["ok"], serde_json::Value::Bool(true));
                assert_eq!(json["command"], "wait");
                ad_free_string(out);
            }
            AdResult::ErrInternal => {
                assert!(out.is_null(), "ErrInternal must leave out null");
                let msg = ad_last_error_message();
                assert!(!msg.is_null(), "error message must be set on failure");
                assert_eq!(
                    ad_last_error_code(),
                    AdResult::ErrInternal,
                    "last-error code must match returned AdResult (errno invariant)"
                );
            }
            other => panic!("unexpected result from ms-mode ad_wait: {:?}", other),
        }
    });
}

#[test]
fn ad_wait_command_error_writes_error_envelope_into_out() {
    with_adapter(|adapter| unsafe {
        let elem = std::ffi::CString::new("__nonexistent_element__").unwrap();
        let args = AdWaitArgs {
            ms: 0,
            has_ms: false,
            element: elem.as_ptr(),
            window: std::ptr::null(),
            text: std::ptr::null(),
            menu: false,
            menu_closed: false,
            notification: false,
            snapshot_id: std::ptr::null(),
            predicate: std::ptr::null(),
            value: std::ptr::null(),
            action: std::ptr::null(),
            count: 0,
            has_count: false,
            timeout_ms: 0,
            app: std::ptr::null(),
        };
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_wait(adapter, &args, &mut out);

        match rc {
            AdResult::Ok => {
                assert!(!out.is_null(), "Ok result must set out");
                ad_free_string(out);
            }
            AdResult::ErrInternal => {
                assert!(
                    out.is_null(),
                    "ErrInternal from off-main-thread guard must leave out null"
                );
            }
            _ => {
                assert!(
                    !out.is_null(),
                    "command-level error must write error envelope into *out, got rc={rc:?}"
                );
                let s = CStr::from_ptr(out).to_string_lossy();
                let val: serde_json::Value =
                    serde_json::from_str(&s).expect("error envelope must be valid JSON");
                assert_eq!(
                    val["ok"].as_bool(),
                    Some(false),
                    "error envelope ok must be false, got: {s}"
                );
                assert_eq!(
                    val["command"].as_str(),
                    Some("wait"),
                    "command field must be 'wait', got: {s}"
                );
                assert!(
                    val["error"].is_object(),
                    "error envelope must carry an error object, got: {s}"
                );
                assert_eq!(
                    ad_last_error_code(),
                    rc,
                    "last-error code must match returned AdResult (errno invariant)"
                );
                ad_free_string(out);
            }
        }
    });
}
