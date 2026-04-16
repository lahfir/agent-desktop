use agent_desktop_ffi::error::AdResult;
use std::ffi::CStr;

#[allow(improper_ctypes)]
extern "C" {
    fn ad_adapter_create() -> *mut agent_desktop_ffi::AdAdapter;
    fn ad_adapter_destroy(adapter: *mut agent_desktop_ffi::AdAdapter);
    fn ad_launch_app(
        adapter: *const agent_desktop_ffi::AdAdapter,
        id: *const std::os::raw::c_char,
        timeout_ms: u64,
        out: *mut agent_desktop_ffi::AdWindowInfo,
    ) -> AdResult;
    fn ad_last_error_message() -> *const std::os::raw::c_char;
    fn ad_last_error_code() -> AdResult;
    fn ad_check_permissions(adapter: *const agent_desktop_ffi::AdAdapter) -> AdResult;
}

#[test]
fn last_error_pointer_survives_across_successful_calls() {
    unsafe {
        let adapter = ad_adapter_create();
        assert!(!adapter.is_null());

        let bad_id = std::ptr::null();
        let mut out_win: agent_desktop_ffi::AdWindowInfo = std::mem::zeroed();
        let rc = ad_launch_app(adapter, bad_id, 0, &mut out_win);
        // Worker-thread cargo tests hit the main-thread guard first
        // (ErrInternal); main-thread callers would see ErrInvalidArgs.
        // The contract we're testing here is that *some* failure
        // populates last-error and the pointer stays stable.
        assert!(matches!(
            rc,
            AdResult::ErrInvalidArgs | AdResult::ErrInternal
        ));

        let first_msg_ptr = ad_last_error_message();
        assert!(!first_msg_ptr.is_null());
        let first_msg = CStr::from_ptr(first_msg_ptr).to_string_lossy().into_owned();

        for _ in 0..10 {
            let _rc = ad_check_permissions(adapter);
        }

        let later_msg_ptr = ad_last_error_message();
        assert_eq!(first_msg_ptr, later_msg_ptr);
        let later_msg = CStr::from_ptr(later_msg_ptr).to_string_lossy().into_owned();
        assert_eq!(first_msg, later_msg);
        assert_eq!(ad_last_error_code(), rc);

        ad_adapter_destroy(adapter);
    }
}
