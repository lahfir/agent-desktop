use super::*;
use agent_desktop_core::PermissionState;

#[test]
fn automation_probe_maps_granted() {
    assert_eq!(map_automation_probe(0, true), PermissionState::Granted);
}

#[test]
fn automation_probe_maps_denied_when_not_permitted() {
    assert_eq!(
        map_automation_probe(-1743, false),
        PermissionState::Denied {
            suggestion: AUTOMATION_SUGGESTION.into(),
        }
    );
}

#[test]
fn automation_probe_maps_denied_when_allowed_flag_false() {
    assert_eq!(
        map_automation_probe(0, false),
        PermissionState::Denied {
            suggestion: AUTOMATION_SUGGESTION.into(),
        }
    );
}

#[test]
fn automation_probe_maps_unknown_when_system_events_not_running() {
    assert_eq!(map_automation_probe(-600, false), PermissionState::Unknown);
}

#[test]
fn automation_probe_maps_unknown_when_consent_not_yet_determined() {
    assert_eq!(map_automation_probe(-1744, false), PermissionState::Unknown);
}

#[test]
fn automation_probe_maps_unknown_for_unrecognized_status() {
    assert_eq!(map_automation_probe(-50, false), PermissionState::Unknown);
}

#[test]
fn permission_report_never_marks_automation_not_required() {
    let report = report();
    assert_ne!(report.automation, PermissionState::NotRequired);
}

#[test]
fn request_report_never_marks_automation_not_required() {
    let report = request_report();
    assert_ne!(report.automation, PermissionState::NotRequired);
}
