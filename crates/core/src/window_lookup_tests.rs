use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};

struct WindowInventory(Vec<WindowInfo>);

impl ObservationOps for WindowInventory {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, crate::AdapterError> {
        Ok(self.0.clone())
    }
}

impl ActionOps for WindowInventory {}
impl InputOps for WindowInventory {}
impl SystemOps for WindowInventory {}

fn window(instance: Option<&str>) -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "Example".into(),
        app: "Example".into(),
        pid: crate::ProcessId::new(42),
        process_instance: instance.map(str::to_string),
        bounds: None,
        state: crate::WindowState {
            is_focused: true,
            ..Default::default()
        },
    }
}

#[test]
fn matching_pid_with_unknown_generation_fails_closed() {
    let adapter = WindowInventory(vec![window(Some("generation-a")), window(None)]);

    let error = find_window_for_process(
        ProcessIdentity::new(42, "generation-a"),
        &adapter,
        crate::Deadline::standard().unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.code(), "ACTION_NOT_SUPPORTED");
}

#[test]
fn shared_window_selector_emits_one_stable_ambiguity_shape() {
    let mut first = window(Some("generation-a"));
    first.state.is_focused = false;
    let mut second = first.clone();
    second.id = "w-2".into();
    second.state.is_focused = false;
    let error = select_window(
        vec![first, second],
        crate::AdapterError::new(ErrorCode::WindowNotFound, "none"),
        "multiple",
    )
    .unwrap_err();

    let AppError::Adapter(error) = error else {
        panic!("expected adapter error");
    };
    let details = error.details.as_ref().unwrap();
    assert_eq!(details["candidate_count"], 2);
    assert_eq!(details["candidate_summaries_truncated"], false);
    assert_eq!(details["candidates"][0]["id"], "w-1");
    assert_eq!(details["candidates"][0]["is_focused"], false);
    assert!(details["candidates"][0].get("focused").is_none());
    assert!(details["candidates"][0].get("pid").is_none());
    assert!(details["candidates"][0].get("process_instance").is_none());
    assert!(error.suggestion.as_deref().unwrap().contains("--window-id"));
}

#[test]
fn shared_window_selector_prefers_the_only_visible_window() {
    let mut hidden = window(Some("generation-a"));
    hidden.state.is_focused = false;
    hidden.state.visible = Some(false);
    let mut visible = hidden.clone();
    visible.id = "w-2".into();
    visible.state.visible = Some(true);

    let selected = select_window(
        vec![hidden, visible],
        crate::AdapterError::new(ErrorCode::WindowNotFound, "none"),
        "multiple",
    )
    .unwrap();

    assert_eq!(selected.id, "w-2");
}
