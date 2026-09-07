use super::resolve_end_session_id;
use crate::cli_args::session::{SessionAction, SessionArgs, SessionEndArgs};
use crate::dispatch::test_support::{FailingOverlayAdapter, HomeGuard};
use agent_desktop_core::context::CommandContext;
use agent_desktop_core::session::{ArtifactsMode, SessionTraceMode, StartSessionOptions};

#[test]
fn explicit_session_end_id_precedes_active_scope() {
    assert_eq!(
        resolve_end_session_id(Some("explicit".into()), Some("active")).unwrap(),
        "explicit"
    );
}

#[test]
fn session_end_falls_back_to_active_scope() {
    assert_eq!(
        resolve_end_session_id(None, Some("active")).unwrap(),
        "active"
    );
}

#[test]
fn session_end_without_any_scope_is_invalid() {
    let error = resolve_end_session_id(None, None).expect_err("missing scope must fail");

    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(error.to_string().contains("No session id"));
}

#[test]
fn session_end_reports_uncertain_when_overlay_teardown_fails_after_persisting() {
    let home = HomeGuard::new();
    let manifest = agent_desktop_core::session::start_session(StartSessionOptions {
        trace: SessionTraceMode::Off,
        artifacts: ArtifactsMode::Events,
        name: None,
    })
    .unwrap();
    let result = super::dispatch(
        SessionArgs {
            action: SessionAction::End(SessionEndArgs {
                id: Some(manifest.id.clone()),
            }),
        },
        &FailingOverlayAdapter,
        &CommandContext::default(),
    );

    let error = result.expect_err("failed teardown must be surfaced");
    assert_eq!(error.code(), "ACTION_FAILED");
    assert!(error.to_string().contains("Session state was saved"));
    let agent_desktop_core::AppError::Adapter(adapter_error) = &error else {
        panic!("teardown failure must preserve adapter disposition");
    };
    assert_eq!(
        adapter_error.disposition,
        agent_desktop_core::DeliverySemantics::uncertain()
    );
    let ended = agent_desktop_core::session::read_manifest(&manifest.id)
        .unwrap()
        .expect("manifest remains readable");
    assert!(ended.ended_at.is_some());
    assert!(home.path().join("sessions").join(manifest.id).is_dir());
}
