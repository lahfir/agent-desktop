use super::*;
use crate::PermissionState;

struct DeniedAdapter;

impl PlatformAdapter for DeniedAdapter {
    fn permission_report(&self) -> PermissionReport {
        PermissionReport {
            accessibility: PermissionState::Denied {
                suggestion: "should not be used".into(),
            },
            screen_recording: PermissionState::Denied {
                suggestion: "should not be used".into(),
            },
            automation: PermissionState::Unknown,
        }
    }
}

#[test]
fn status_uses_precomputed_permission_report() {
    let report = PermissionReport {
        accessibility: PermissionState::Granted,
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };

    let value =
        execute_with_report_with_context(&DeniedAdapter, &report, &CommandContext::default())
            .unwrap();
    let permissions = value.get("permissions").unwrap();

    assert_eq!(permissions["accessibility"]["state"], "granted");
    assert_eq!(permissions["screen_recording"]["state"], "granted");
    assert_eq!(permissions["automation"]["state"], "not_required");
}

#[test]
fn status_reports_tracing_false_when_writer_failed() {
    let _guard = crate::refs_test_support::HomeGuard::new();
    let session = crate::session::start_session(crate::session::StartSessionOptions {
        name: None,
        trace: crate::session::SessionTraceMode::On,
        force: true,
        ..Default::default()
    })
    .unwrap();
    let unopenable = std::env::temp_dir()
        .join("agent-desktop-status-nodir")
        .join("trace.jsonl");
    let context = CommandContext::new(Some(session.id), Some(unopenable), false).unwrap();
    let report = PermissionReport {
        accessibility: PermissionState::Granted,
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };

    let value = execute_with_report_with_context(&DeniedAdapter, &report, &context).unwrap();

    assert_eq!(
        value["tracing"], false,
        "a failed trace writer must not report tracing:true"
    );
}

#[test]
fn status_surfaces_artifacts_mode_for_active_session() {
    let _guard = crate::refs_test_support::HomeGuard::new();
    let session = crate::session::start_session(crate::session::StartSessionOptions {
        artifacts: crate::session::ArtifactsMode::Full,
        ..Default::default()
    })
    .unwrap();
    let context = CommandContext::new(Some(session.id.clone()), None, false).unwrap();
    let report = PermissionReport {
        accessibility: PermissionState::Granted,
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };

    let value = execute_with_report_with_context(&DeniedAdapter, &report, &context).unwrap();

    assert_eq!(value["artifacts"], "full");
}
