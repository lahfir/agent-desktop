use super::*;

#[test]
fn application_menu_gestures_focus_the_process_and_keep_pointer_verification() {
    for action in [Action::RightClick, Action::DoubleClick, Action::TripleClick] {
        let mut adapter = adapter(false, true);
        adapter.application_menu = true;
        adapter.pointer_menu = Some(AtomicUsize::new(0));
        execute_action_within(&adapter, true, 2_000, action).unwrap();
        assert!(adapter.focused.load(Ordering::SeqCst));
        assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn application_menu_gestures_never_dispatch_through_an_occluder() {
    for action in [Action::RightClick, Action::DoubleClick, Action::TripleClick] {
        let mut adapter = adapter(true, true);
        adapter.application_menu = true;
        adapter.pointer_menu = Some(AtomicUsize::new(0));
        execute_action_within(&adapter, true, 150, action).unwrap_err();
        assert!(adapter.focused.load(Ordering::SeqCst));
        assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn disabled_application_menu_gestures_do_not_focus_or_dispatch() {
    let mut adapter = adapter(false, false);
    adapter.application_menu = true;
    adapter.pointer_menu = Some(AtomicUsize::new(0));
    execute_action_within(&adapter, true, 150, Action::DoubleClick).unwrap_err();
    assert!(!adapter.focused.load(Ordering::SeqCst));
    assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 0);
}

#[test]
fn positive_timeout_menu_gestures_wait_through_three_zero_geometry_samples() {
    for action in [Action::RightClick, Action::DoubleClick, Action::TripleClick] {
        let mut adapter = adapter(false, true);
        adapter.application_menu = true;
        adapter.pointer_menu = Some(AtomicUsize::new(3));
        execute_action_within(&adapter, true, 500, action).unwrap();
        assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn single_shot_menu_gesture_does_not_sample_zero_geometry() {
    let mut adapter = adapter(false, true);
    adapter.application_menu = true;
    adapter.pointer_menu = Some(AtomicUsize::new(3));
    let error = execute_action_within(&adapter, true, 0, Action::RightClick).unwrap_err();
    assert_eq!(error.code, crate::ErrorCode::ActionFailed);
    assert_eq!(
        adapter
            .pointer_menu
            .as_ref()
            .unwrap()
            .load(Ordering::SeqCst),
        3
    );
    assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 0);
}

#[test]
fn permanently_zero_geometry_uses_only_the_existing_deadline() {
    let mut adapter = adapter(false, true);
    adapter.application_menu = true;
    adapter.pointer_menu = Some(AtomicUsize::new(usize::MAX));
    let error = execute_action_within(&adapter, true, 100, Action::DoubleClick).unwrap_err();
    assert_eq!(error.code, crate::ErrorCode::Timeout);
    assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 0);
}

#[test]
fn geometry_settling_does_not_bypass_the_final_hit_test() {
    let mut adapter = adapter(true, true);
    adapter.application_menu = true;
    adapter.pointer_menu = Some(AtomicUsize::new(3));
    execute_action_within(&adapter, true, 500, Action::TripleClick).unwrap_err();
    assert_eq!(
        adapter
            .pointer_menu
            .as_ref()
            .unwrap()
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 0);
}
