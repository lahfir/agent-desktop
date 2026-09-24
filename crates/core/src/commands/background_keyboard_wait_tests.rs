//! `--wait-for` after `press --background` must observe the target window,
//! not whatever app the user has frontmost.
use super::test_support::{PID, live_window, press_args};
use super::*;
use crate::commands::background_wait_test_support::FrontmostElsewhereAdapter;
use crate::context::WaitSelector;
use crate::refs_test_support::HomeGuard;

#[test]
fn window_press_waits_on_the_target_window_not_the_frontmost_app() {
    let _guard = HomeGuard::new();
    let target = live_window(PID);
    let target_id = target.id.clone();
    let adapter = FrontmostElsewhereAdapter::new(target);
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: ":saved".into(),
        gone: false,
        timeout_ms: 500,
    }));

    let value = execute(press_args("cmd+s", false), &adapter, &context)
        .expect("the confirmation exists only in the target window");

    assert_eq!(value["matched_selector"], ":saved");
    assert_eq!(value["after_action"]["pressed"], true);
    let observed = adapter.observed.lock().unwrap();
    assert!(
        !observed.is_empty() && observed.iter().all(|id| *id == target_id),
        "only the target window may be observed: {observed:?}"
    );
}
