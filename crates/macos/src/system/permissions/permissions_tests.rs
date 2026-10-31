use super::*;

#[test]
fn expired_permission_deadline_fails_without_native_calls() {
    let error = report(Deadline::after(0).unwrap()).unwrap_err();

    assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
}

#[test]
fn automation_probe_maps_without_a_prompting_state() {
    assert_eq!(map_automation_probe(0), PermissionState::Granted);
    assert!(matches!(
        map_automation_probe(-1743),
        PermissionState::Denied { .. }
    ));
    assert_eq!(map_automation_probe(-1744), PermissionState::Unknown);
    assert_eq!(map_automation_probe(-600), PermissionState::Unknown);
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
                automation: PermissionState::Unknown,
            })
        },
    )
    .unwrap();

    assert_eq!(requested.len(), 3);
    assert_eq!(report.accessibility, PermissionState::Granted);
    assert_eq!(report.screen_recording, PermissionState::Granted);
    assert_eq!(report.automation, PermissionState::Unknown);
}

#[test]
fn authorization_denial_is_structured_and_nonprompting() {
    let error = automation_permission_error(-1744);

    assert_eq!(error.code, agent_desktop_core::ErrorCode::PermDenied);
    assert_eq!(error.details.unwrap()["prompted"], false);
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|value| value.contains("Automation"))
    );
}

#[cfg(unix)]
#[test]
fn osascript_denial_output_maps_to_permission_denied() {
    use std::os::unix::process::ExitStatusExt;

    let error = map_automation_command_failure(
        std::process::ExitStatus::from_raw(1 << 8),
        b"execution error: Not authorized to send Apple events to System Events. (-1743)",
    );

    assert_eq!(error.code, agent_desktop_core::ErrorCode::PermDenied);
    assert_eq!(error.details.unwrap()["os_status"], -1743);
}

#[test]
fn automation_prompt_gets_a_bounded_budget_and_reports_pending() {
    use crate::system::permissions::permission_operation::PermissionOperation;
    let mut automation_budget = None;
    let error = request_report_with(
        Deadline::after(60_000).unwrap(),
        |operation, deadline| {
            if operation == PermissionOperation::Automation {
                automation_budget = Some(deadline.remaining());
                return Err(AdapterError::timeout("helper did not answer"));
            }
            Ok(true)
        },
        |_| panic!("report must not run after a pending automation prompt"),
    )
    .unwrap_err();

    assert!(automation_budget.unwrap() <= AUTOMATION_PROMPT_BUDGET);
    assert_eq!(error.code, agent_desktop_core::ErrorCode::PermDenied);
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "automation_permission_pending");
    assert_eq!(details["reason"], "AUTOMATION_PERMISSION_PENDING");
}

#[test]
fn expired_outer_deadline_keeps_the_timeout_error() {
    let error = automation_prompt_error(
        AdapterError::timeout("expired"),
        Deadline::after(0).unwrap(),
    );

    assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
}
