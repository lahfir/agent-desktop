use super::*;

#[test]
fn expired_permission_deadline_fails_without_native_calls() {
    let error = report(Deadline::after(0).unwrap()).unwrap_err();

    assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
}

#[test]
fn automation_is_not_required_without_apple_events_fallbacks() {
    assert_eq!(automation_report_state(), PermissionState::NotRequired);
}

#[test]
fn post_helper_preflight_is_the_authoritative_permission_state() {
    let mut requested = Vec::new();
    let report = request_report_with(
        Deadline::after(1_000).unwrap(),
        |operation, _| {
            requested.push(operation);
            Ok(false)
        },
        |_| {
            Ok(PermissionReport {
                accessibility: PermissionState::Granted,
                screen_recording: PermissionState::Granted,
                automation: PermissionState::NotRequired,
            })
        },
    )
    .unwrap();

    assert_eq!(requested.len(), 2);
    assert_eq!(report.accessibility, PermissionState::Granted);
    assert_eq!(report.screen_recording, PermissionState::Granted);
}
