use super::*;

#[test]
fn default_adapter_screenshot_skips_cleanly() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    run_ref_action(&context, &DefaultScreenshotAdapter, 1).unwrap();
}

#[test]
fn failing_action_still_captures_post_screenshot() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let adapter = FailingActionAdapter {
        screenshot_calls: AtomicU32::new(0),
    };
    let err = run_ref_action(&context, &adapter, 1).unwrap_err();
    assert_eq!(err.code(), "INTERNAL");
    assert_eq!(adapter.screenshot_calls.load(Ordering::SeqCst), 2);
}

#[test]
fn capture_targets_window_for_pid() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let adapter = png_adapter();
    run_ref_action(&context, &adapter, 99).unwrap();
    match adapter.target.lock().unwrap().take() {
        Some(ScreenshotTarget::ExactWindow(window)) => {
            assert_eq!(window.pid, 99);
            assert_eq!(window.id, "w-99");
        }
        _ => panic!("expected exact window screenshot target"),
    }
}
