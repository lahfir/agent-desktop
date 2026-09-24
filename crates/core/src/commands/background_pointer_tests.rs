use super::test_support::*;
use super::*;
use crate::{DeliverySemantics, ProcessId, refs_test_support::HomeGuard};

#[test]
fn headed_context_is_rejected_before_any_delivery() {
    let adapter = BackgroundCaptureAdapter::new();

    let err = execute(
        point_args(left_click(1), -2500.0, 300.0),
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(adapter.delivered().is_empty());
}

#[test]
fn headless_default_context_allows_background_delivery_without_real_cursor() {
    let adapter = BackgroundCaptureAdapter::new();

    execute(
        point_args(BackgroundPointerAction::Move, -2500.0, 300.0),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(adapter.delivered().len(), 1);
    assert_eq!(*adapter.real_mouse_events.lock().unwrap(), 0);
}

#[test]
fn ref_hover_derives_pid_and_exact_window_from_the_ref() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(Some(WINDOW_ID));
    let adapter = BackgroundCaptureAdapter::new();

    let value = execute(
        ref_args(BackgroundPointerAction::Hover, snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    let expected = adapter.expected_windows.lock().unwrap().clone();
    assert_eq!(expected.len(), 1);
    assert_eq!(expected[0].id, WINDOW_ID);
    assert_eq!(expected[0].pid, PID);
    assert_eq!(
        expected[0].process_instance.as_deref(),
        Some("test-instance")
    );
    assert!(
        expected[0].title.is_empty(),
        "mutable titles must not pin identity"
    );

    let delivered = adapter.delivered();
    assert_eq!(delivered.len(), 1);
    let (window, event) = &delivered[0];
    assert_eq!(window.id, WINDOW_ID);
    assert_eq!(window.pid, PID);
    assert!(matches!(event.kind, MouseEventKind::Move));
    assert_eq!((event.point.x, event.point.y), (-2850.0, 160.0));

    assert_eq!(value["hovered"], true);
    assert_eq!(value["background"]["pid"], PID);
    assert_eq!(value["background"]["window_id"], WINDOW_ID);
}

#[test]
fn ref_without_exact_source_window_is_rejected() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(None);
    let adapter = BackgroundCaptureAdapter::new();

    let err = execute(
        ref_args(BackgroundPointerAction::Hover, snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ACTION_NOT_SUPPORTED");
    assert!(adapter.delivered().is_empty());
}

#[test]
fn window_owned_by_a_different_process_is_rejected_before_posting() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(Some(WINDOW_ID));
    let mut adapter = BackgroundCaptureAdapter::new();
    adapter.live_pid = PID + 1;

    let err = execute(
        ref_args(left_click(1), snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "STALE_REF");
    assert!(adapter.delivered().is_empty());
}

#[test]
fn point_outside_window_bounds_is_rejected_as_not_delivered() {
    let adapter = BackgroundCaptureAdapter::new();

    let err = execute(
        point_args(left_click(1), -2200.0, 300.0),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    let AppError::Adapter(error) = err else {
        panic!("expected adapter error");
    };
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(adapter.delivered().is_empty());
}

#[test]
fn offscreen_window_point_is_delivered() {
    let adapter = BackgroundCaptureAdapter::new();

    execute(
        point_args(left_click(1), -3000.0, 100.0),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(adapter.delivered().len(), 1);
}

#[test]
fn click_reports_delivered_unverified_and_unchanged_focus() {
    let adapter = BackgroundCaptureAdapter::new();

    let value = execute(
        point_args(
            BackgroundPointerAction::Click {
                button: MouseButton::Right,
                count: 2,
                modifiers: vec![Modifier::Shift],
            },
            -2500.0,
            300.0,
        ),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    let (window, event) = adapter.delivered().remove(0);
    assert_eq!(window.id, WINDOW_ID);
    assert!(matches!(event.kind, MouseEventKind::Click { count: 2 }));
    assert!(matches!(event.button, MouseButton::Right));
    assert!(matches!(event.modifiers.as_slice(), [Modifier::Shift]));

    assert_eq!(value["clicked"], true);
    assert_eq!(value["count"], 2);
    assert_eq!(value["disposition"]["delivery"], "delivered_unverified");
    assert_eq!(value["disposition"]["retry"], "unsafe");
    assert_eq!(value["background"]["focus_change"], "unchanged");
    assert_eq!(value["background"]["frontmost_pid_before"], 7);
    assert!(value.get("warning").is_none());
}

#[test]
fn frontmost_change_is_reported_with_a_warning() {
    let mut adapter = BackgroundCaptureAdapter::new();
    adapter.report.frontmost_pid_after = Some(ProcessId::new(PID));

    let value = execute(
        point_args(left_click(1), -2500.0, 300.0),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["background"]["focus_change"], "changed");
    assert_eq!(value["background"]["frontmost_pid_after"], PID);
    assert!(value["warning"].is_string());
}

#[test]
fn a_user_switch_the_guard_yielded_to_is_not_blamed_on_the_target() {
    let mut adapter = BackgroundCaptureAdapter::new();
    adapter.report.frontmost_pid_after = Some(ProcessId::new(9));
    adapter.report.focus_guard = Some(crate::BackgroundFocusGuard {
        yielded: true,
        ..crate::BackgroundFocusGuard::default()
    });

    let value = execute(
        point_args(left_click(1), -2500.0, 300.0),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["background"]["focus_change"], "changed");
    assert_eq!(value["background"]["focus_guard"]["yielded"], true);
    let warning = value["warning"].as_str().unwrap();
    assert!(warning.contains("left it alone"), "{warning}");
}

#[test]
fn guarded_steal_reports_layers_degradations_and_guard_evidence() {
    let mut adapter = BackgroundCaptureAdapter::new();
    adapter.report.layers = vec!["route".into(), "guard".into()];
    adapter.report.degradations = vec!["skylight: SLEventPostToPid unavailable".into()];
    adapter.report.focus_guard = Some(crate::BackgroundFocusGuard {
        interventions: 1,
        restored: true,
        max_steal_ms: 42,
        yielded: false,
    });

    let value = execute(
        point_args(left_click(1), -2500.0, 300.0),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    let background = &value["background"];
    assert_eq!(background["focus_change"], "restored");
    assert_eq!(background["layers"], serde_json::json!(["route", "guard"]));
    assert_eq!(
        background["degraded"][0],
        "skylight: SLEventPostToPid unavailable"
    );
    assert_eq!(background["focus_guard"]["interventions"], 1);
    assert_eq!(background["focus_guard"]["restored"], true);
    assert_eq!(background["focus_guard"]["max_steal_ms"], 42);
    assert!(value["warning"].as_str().unwrap().contains("42 ms"));
    assert_eq!(value["disposition"]["delivery"], "delivered_unverified");
}

#[test]
fn unreadable_frontmost_is_reported_as_unknown() {
    let mut adapter = BackgroundCaptureAdapter::new();
    adapter.report.frontmost_pid_after = None;

    let value = execute(
        point_args(BackgroundPointerAction::Move, -2500.0, 300.0),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["moved"], true);
    assert_eq!(value["background"]["focus_change"], "unknown");
    assert!(value["background"].get("frontmost_pid_after").is_none());
}

#[test]
fn invalid_click_count_is_rejected_before_delivery() {
    let adapter = BackgroundCaptureAdapter::new();

    let err = execute(
        point_args(left_click(0), -2500.0, 300.0),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(adapter.delivered().is_empty());
}
