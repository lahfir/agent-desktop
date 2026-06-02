mod common;

use common::{
    AdActionResult, AdNativeHandle, AdPolicyKind, AdResult, ad_execute_action,
    ad_execute_action_with_policy, ad_execute_ref_action_with_policy, default_action,
    default_ref_entry, with_adapter,
};

#[test]
fn enum_fuzz_invalid_discriminant_rejected() {
    with_adapter(|adapter| unsafe {
        let mut action = default_action();
        action.kind = i32::MAX;
        let handle = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let mut out: AdActionResult = std::mem::zeroed();
        let rc = ad_execute_action(adapter, &handle, &action, &mut out);
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "arbitrary enum bit pattern must be rejected, got {:?}",
            rc
        );
    });
}

#[test]
fn invalid_policy_discriminant_rejected_without_ub() {
    with_adapter(|adapter| unsafe {
        let action = default_action();
        let handle = AdNativeHandle {
            ptr: std::ptr::dangling::<std::ffi::c_void>(),
        };
        let mut out: AdActionResult = std::mem::zeroed();
        let rc = ad_execute_action_with_policy(
            adapter,
            &handle,
            &action,
            AdPolicyKind::Physical as i32 + 1,
            &mut out,
        );
        assert!(matches!(
            rc,
            AdResult::ErrInvalidArgs | AdResult::ErrInternal
        ));
    });
}

#[test]
fn execute_action_rejects_null_handle_ptr() {
    with_adapter(|adapter| unsafe {
        let action = default_action();
        let handle = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let mut out: AdActionResult = std::mem::zeroed();
        let rc = ad_execute_action(adapter, &handle, &action, &mut out);
        assert!(matches!(
            rc,
            AdResult::ErrInvalidArgs | AdResult::ErrInternal
        ));
    });
}

#[test]
fn execute_ref_action_uses_strict_resolution_before_dispatch() {
    with_adapter(|adapter| unsafe {
        let role = std::ffi::CString::new("button").unwrap();
        let mut entry = default_ref_entry();
        entry.role = role.as_ptr();
        let action = default_action();
        let mut out: AdActionResult = std::mem::zeroed();

        let rc = ad_execute_ref_action_with_policy(
            adapter,
            &entry,
            &action,
            AdPolicyKind::Headless as i32,
            &mut out,
        );

        assert!(matches!(
            rc,
            AdResult::ErrStaleRef | AdResult::ErrElementNotFound | AdResult::ErrInternal
        ));
    });
}

#[test]
fn execute_action_policy_requires_main_thread_on_macos() {
    let rc = std::thread::spawn(|| unsafe {
        let action = default_action();
        let handle = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let mut out: AdActionResult = std::mem::zeroed();
        ad_execute_action_with_policy(
            std::ptr::null(),
            &handle,
            &action,
            AdPolicyKind::Headless as i32,
            &mut out,
        )
    })
    .join()
    .unwrap();

    #[cfg(target_os = "macos")]
    assert_eq!(rc, AdResult::ErrInternal);
    #[cfg(not(target_os = "macos"))]
    assert_eq!(rc, AdResult::ErrInvalidArgs);
}
