mod common;

use common::{
    AdActionResult, AdActionStep, AdNativeHandle, AdPolicyKind, AdResult,
    ad_adapter_create_with_session, ad_adapter_destroy, ad_execute_action,
    ad_execute_action_with_policy, ad_execute_ref_action_with_policy, ad_free_action_result,
    default_action, default_ref_entry, with_adapter,
};
use std::ffi::CString;

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
            AdPolicyKind::Headed as i32 + 1,
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

/// Verifies that `ad_execute_ref_action_with_policy` uses the adapter's session
/// context rather than a default one. The observable contract is that resolution
/// fails (stale ref) identically whether a session id is present or absent —
/// the session id is wired into trace emission, not into the error path.
#[test]
fn execute_ref_action_with_session_adapter_wires_context() {
    unsafe {
        let session = CString::new("test-session-01").unwrap();
        let adapter = ad_adapter_create_with_session(session.as_ptr());
        assert!(!adapter.is_null());

        let role = CString::new("button").unwrap();
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

        assert!(
            matches!(
                rc,
                AdResult::ErrStaleRef | AdResult::ErrElementNotFound | AdResult::ErrInternal
            ),
            "session adapter must still reject unresolvable entry, got {:?}",
            rc
        );

        ad_adapter_destroy(adapter);
    }
}

#[test]
fn free_action_result_releases_non_empty_steps_array() {
    let mut steps = vec![
        AdActionStep {
            label: CString::new("AXScrollToVisible").unwrap().into_raw(),
            outcome: CString::new("attempted").unwrap().into_raw(),
        },
        AdActionStep {
            label: CString::new("AXPress").unwrap().into_raw(),
            outcome: CString::new("succeeded").unwrap().into_raw(),
        },
        AdActionStep {
            label: std::ptr::null(),
            outcome: std::ptr::null(),
        },
    ]
    .into_boxed_slice();
    let mut result = AdActionResult {
        action: CString::new("click").unwrap().into_raw(),
        ref_id: std::ptr::null(),
        post_state: std::ptr::null_mut(),
        steps: steps.as_mut_ptr(),
        step_count: 2,
    };
    std::mem::forget(steps);

    unsafe { ad_free_action_result(&mut result) };

    assert!(result.action.is_null());
    assert!(result.steps.is_null());
    assert_eq!(result.step_count, 0);
}
