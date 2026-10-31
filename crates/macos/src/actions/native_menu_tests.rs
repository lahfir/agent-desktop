use super::*;
use agent_desktop_core::ErrorCode;

#[test]
fn headed_requirements_share_provenance_and_preserve_pointer_checks_for_gestures() {
    use agent_desktop_core::{Action, ActionRequest, HeadedRequirement};
    for action in [
        Action::Click,
        Action::RightClick,
        Action::DoubleClick,
        Action::TripleClick,
    ] {
        let request = ActionRequest::headed(action.clone());
        let native = headed_requirement(&request, || Ok(true)).unwrap();
        assert_eq!(
            native,
            if matches!(action, Action::Click) {
                HeadedRequirement::FocusedApplication
            } else {
                HeadedRequirement::FocusedApplicationAndCursor
            }
        );
        assert_eq!(native.requires_cursor(), !matches!(action, Action::Click));
        assert_eq!(
            headed_requirement(&request, || Ok(false)).unwrap(),
            HeadedRequirement::FocusedWindowAndCursor
        );
        let error = headed_requirement(&request, || {
            Err(AdapterError::stale_ref_because("identity changed"))
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::StaleRef);
        let headless = ActionRequest::headless(action);
        assert_eq!(
            headed_requirement(&headless, || panic!(
                "headless requirement must not resolve"
            ))
            .unwrap(),
            HeadedRequirement::None
        );
    }
}

#[test]
fn non_pointer_headed_actions_do_not_probe_menu_ancestry() {
    let request = agent_desktop_core::ActionRequest::headed(agent_desktop_core::Action::SetFocus);
    assert_eq!(
        headed_requirement(&request, || panic!("not a menu gesture")).unwrap(),
        agent_desktop_core::HeadedRequirement::FocusedWindow
    );
}
