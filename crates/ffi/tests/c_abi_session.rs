mod common;

use common::{
    AdResult, CStr, ad_adapter_create, ad_adapter_create_with_session, ad_adapter_destroy,
    ad_last_error_code, ad_last_error_message,
};

#[test]
fn sessionless_adapter_has_no_session_id() {
    unsafe {
        let ptr = ad_adapter_create();
        assert!(!ptr.is_null(), "ad_adapter_create must not return null");
        let ctx = (*ptr)
            .command_context()
            .expect("command_context must succeed");
        assert_eq!(ctx.session_id(), None);
        ad_adapter_destroy(ptr);
    }
}

#[test]
fn session_adapter_carries_session_id() {
    unsafe {
        let session = std::ffi::CString::new("agent-a").unwrap();
        let ptr = ad_adapter_create_with_session(session.as_ptr());
        assert!(
            !ptr.is_null(),
            "ad_adapter_create_with_session must not return null"
        );
        let ctx = (*ptr)
            .command_context()
            .expect("command_context must succeed");
        assert_eq!(ctx.session_id(), Some("agent-a"));
        ad_adapter_destroy(ptr);
    }
}

#[test]
fn null_session_adapter_is_sessionless() {
    unsafe {
        let ptr = ad_adapter_create_with_session(std::ptr::null());
        assert!(
            !ptr.is_null(),
            "null session must yield a sessionless adapter"
        );
        let ctx = (*ptr)
            .command_context()
            .expect("command_context must succeed");
        assert_eq!(ctx.session_id(), None);
        ad_adapter_destroy(ptr);
    }
}

#[test]
fn invalid_utf8_session_returns_null_and_sets_invalid_args() {
    unsafe {
        let bad: [u8; 3] = [0xC3, 0xFF, 0x00];
        let ptr = ad_adapter_create_with_session(bad.as_ptr() as *const std::os::raw::c_char);
        assert!(ptr.is_null(), "invalid UTF-8 session must return null");
        assert_eq!(
            ad_last_error_code(),
            AdResult::ErrInvalidArgs,
            "invalid UTF-8 must set ErrInvalidArgs"
        );
        let msg = ad_last_error_message();
        assert!(
            !msg.is_null(),
            "error message must be set on invalid UTF-8 session"
        );
    }
}

#[test]
fn empty_session_returns_null_and_sets_invalid_args() {
    unsafe {
        let empty = std::ffi::CString::new("").unwrap();
        let ptr = ad_adapter_create_with_session(empty.as_ptr());
        assert!(ptr.is_null(), "empty session id must return null");
        assert_eq!(
            ad_last_error_code(),
            AdResult::ErrInvalidArgs,
            "empty session id must set ErrInvalidArgs"
        );
        let msg = ad_last_error_message();
        assert!(!msg.is_null(), "error message must be set on empty session");
        let _ = CStr::from_ptr(msg).to_string_lossy();
    }
}
