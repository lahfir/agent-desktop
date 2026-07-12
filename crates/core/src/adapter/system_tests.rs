use super::*;

struct DefaultOnly;
impl SystemOps for DefaultOnly {}

#[test]
fn default_interaction_lease_is_unsupported_in_test_builds_too() {
    let error = match DefaultOnly.acquire_interaction_lease(crate::Deadline::standard().unwrap()) {
        Ok(_) => panic!("default lease must fail closed"),
        Err(error) => error,
    };

    assert_eq!(error.code, crate::ErrorCode::PlatformNotSupported);
}

#[test]
fn default_surfaces_fail_closed() {
    assert!(DefaultOnly.supported_surfaces().is_empty());
}

#[test]
fn default_open_session_is_explicitly_unsupported() {
    let affinity = crate::SessionAffinity {
        session_id: Some("test-session".into()),
    };
    let error = match DefaultOnly.open_session(&affinity, crate::Deadline::standard().unwrap()) {
        Ok(_) => panic!("default session affinity must fail closed"),
        Err(error) => error,
    };

    assert_eq!(error.code, crate::ErrorCode::PlatformNotSupported);
    assert!(error.message.contains("open_session"));
}
