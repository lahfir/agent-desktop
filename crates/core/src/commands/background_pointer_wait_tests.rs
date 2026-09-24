//! `--wait-for` after a background pointer event must observe the target
//! window, not whatever app the user has frontmost.
use super::test_support::{PID, WINDOW_ID, left_click, point_args, window_bounds};
use super::*;
use crate::commands::background_wait_test_support::FrontmostElsewhereAdapter;
use crate::context::WaitSelector;
use crate::refs_test_support::HomeGuard;
use crate::{ProcessId, WindowState};

#[test]
fn coordinate_click_waits_on_the_target_window_not_the_frontmost_app() {
    let _guard = HomeGuard::new();
    let adapter = FrontmostElsewhereAdapter::new(WindowInfo {
        id: WINDOW_ID.into(),
        title: "Target".into(),
        app: "Code".into(),
        pid: ProcessId::new(PID),
        process_instance: Some("test-instance".into()),
        bounds: Some(window_bounds()),
        state: WindowState::default(),
    });
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: ":saved".into(),
        gone: false,
        timeout_ms: 500,
    }));

    let value = execute(
        point_args(left_click(1), -2500.0, 300.0),
        &adapter,
        &context,
    )
    .expect("the confirmation exists only in the target window");

    assert_eq!(value["matched_selector"], ":saved");
    assert_eq!(value["after_action"]["clicked"], true);
    let observed = adapter.observed.lock().unwrap();
    assert!(
        !observed.is_empty() && observed.iter().all(|id| id == WINDOW_ID),
        "only the target window may be observed: {observed:?}"
    );
}
