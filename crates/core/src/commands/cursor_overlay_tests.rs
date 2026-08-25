use super::*;
use crate::commands::session::{self, SessionAction};
use crate::refs_test_support::HomeGuard;

#[test]
fn session_and_overlay_responses_show_the_exact_activation_export() {
    let _guard = HomeGuard::new();
    let started = session::execute(SessionAction::Start {
        name: None,
        no_trace: false,
        screenshots: false,
    })
    .expect("session start");
    let id = started["session_id"].as_str().expect("session id");
    let expected = format!("export AGENT_DESKTOP_SESSION={id}");

    assert_eq!(started["next"], expected);

    let enabled = execute(
        id,
        CursorOverlayAction::Enable(CursorOverlayConfig::enabled(None, 6).unwrap()),
    )
    .expect("cursor overlay enable");

    assert_eq!(enabled["next"], expected);
}
