use super::dispatch;
use crate::cli_args::cursor_overlay::CursorOverlayArgs;
use crate::cli_args::cursor_overlay_action::CursorOverlayAction;
use crate::dispatch::test_support::{FailingOverlayAdapter, HomeGuard};
use agent_desktop_core::context::CommandContext;
use agent_desktop_core::session::{ArtifactsMode, SessionTraceMode, StartSessionOptions};

#[test]
fn disable_reports_uncertain_when_overlay_teardown_fails_after_persisting() {
    let home = HomeGuard::new();
    let manifest = agent_desktop_core::session::start_session(StartSessionOptions {
        trace: SessionTraceMode::Off,
        artifacts: ArtifactsMode::Events,
        name: None,
    })
    .unwrap();
    agent_desktop_core::session::set_cursor_overlay(
        &manifest.id,
        agent_desktop_core::CursorOverlayConfig::enabled(None, 6).unwrap(),
    )
    .unwrap();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let result = dispatch(
        CursorOverlayArgs {
            action: CursorOverlayAction::Disable,
        },
        &FailingOverlayAdapter,
        &context,
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
    let saved = agent_desktop_core::session::read_manifest(&manifest.id)
        .unwrap()
        .expect("manifest remains readable");
    assert_eq!(saved.cursor_overlay, Default::default());
    assert!(home.path().join("sessions").join(manifest.id).is_dir());
}
