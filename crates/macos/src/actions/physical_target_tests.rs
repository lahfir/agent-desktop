use super::*;
use agent_desktop_core::HitTestResult;

#[test]
fn final_pointer_guard_requires_both_hit_target_and_same_generation() {
    verify_pointer_identity(Ok(HitTestResult::ReachesTarget), || Ok(true)).unwrap();
    let error =
        verify_pointer_identity(Ok(HitTestResult::ReachesTarget), || Ok(false)).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.details.unwrap()["physical_delivery_started"], false);
}

#[test]
fn final_pointer_guard_refuses_occluded_and_unknown_points() {
    for hit in [
        HitTestResult::Unknown,
        HitTestResult::InterceptedBy {
            role: Some("window".into()),
            name: Some("occluder".into()),
            bounds: None,
        },
    ] {
        let error =
            verify_pointer_identity(Ok(hit), || panic!("hit did not reach target")).unwrap_err();
        assert_eq!(error.code, ErrorCode::ActionFailed);
        assert_eq!(error.details.unwrap()["physical_delivery_started"], false);
    }
}

#[test]
fn final_pointer_guard_preserves_hit_test_and_identity_read_errors() {
    for code in [
        ErrorCode::PermDenied,
        ErrorCode::Timeout,
        ErrorCode::StaleRef,
    ] {
        let failure = AdapterError::new(code.clone(), "native read failed");
        let error = verify_pointer_identity(Err(failure.clone()), || panic!("hit test failed"))
            .unwrap_err();
        assert_eq!(error.code, code);
        let error =
            verify_pointer_identity(Ok(HitTestResult::ReachesTarget), || Err(failure.clone()))
                .unwrap_err();
        assert_eq!(error.code, code);
    }
}
