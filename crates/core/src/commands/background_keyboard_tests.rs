use super::test_support::*;
use super::*;
use crate::{DeliverySemantics, ErrorCode, Modifier, ProcessId, refs_test_support::HomeGuard};

#[test]
fn headed_context_is_rejected_before_any_delivery() {
    let adapter = KeyboardCaptureAdapter::new();

    let err = execute(
        press_args("cmd+s", false),
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(adapter.calls().is_empty());
}

#[test]
fn window_press_posts_only_to_the_exact_window_and_never_uses_the_menu_path() {
    let adapter = KeyboardCaptureAdapter::new();

    let value = execute(
        press_args("Cmd+S", false),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(adapter.calls(), ["background_keys"]);
    let (window, input) = adapter.delivered().remove(0);
    assert_eq!(window.id, WINDOW_ID);
    assert_eq!(window.pid, PID);
    let BackgroundKeyInput::Combo(combo) = input else {
        panic!("expected a combo");
    };
    assert_eq!(combo.key, "s");
    assert!(matches!(combo.modifiers.as_slice(), [Modifier::Meta]));

    let expected = adapter.recorded.lock().unwrap().expected_windows.clone();
    assert!(
        expected[0].title.is_empty(),
        "mutable titles must not pin identity"
    );

    assert_eq!(value["pressed"], true);
    assert_eq!(value["combo"], "Cmd+S");
    assert_eq!(value["disposition"]["delivery"], "delivered_unverified");
    assert_eq!(value["disposition"]["retry"], "unsafe");
    assert_eq!(value["background"]["window_id"], WINDOW_ID);
    assert_eq!(value["background"]["focus_change"], "unchanged");
    assert_eq!(
        value["background"]["layers"],
        serde_json::json!(["route", "skylight"])
    );
    assert!(value["background"].get("ax_focus").is_none());
}

#[test]
fn blocked_combo_needs_force() {
    let adapter = KeyboardCaptureAdapter::new();

    let err = execute(
        press_args("cmd+q", false),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "POLICY_DENIED");
    assert!(adapter.calls().is_empty());

    execute(
        press_args("cmd+q", true),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();
    assert_eq!(adapter.delivered().len(), 1);
}

#[test]
fn ref_type_focuses_the_element_then_posts_text_to_the_ref_window() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(Some(WINDOW_ID));
    let adapter = KeyboardCaptureAdapter::new();

    let value = execute(
        type_args("héllo from background, 42!", snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(adapter.calls(), ["action:focus", "background_keys"]);
    assert_eq!(
        adapter.recorded.lock().unwrap().focus_policies,
        [crate::InteractionPolicy::headless()],
        "the focus attempt must never fall back to physical input"
    );
    let (window, input) = adapter.delivered().remove(0);
    assert_eq!(window.id, WINDOW_ID);
    assert_eq!(window.pid, PID);
    assert_eq!(window.process_instance.as_deref(), Some("test-instance"));
    let BackgroundKeyInput::Text(text) = input else {
        panic!("expected text");
    };
    assert_eq!(text, "héllo from background, 42!");

    assert_eq!(value["typed"], true);
    assert_eq!(value["characters"], 26);
    assert_eq!(value["background"]["ax_focus"]["status"], "verified");
    assert_eq!(value["disposition"]["delivery"], "delivered_unverified");
}

/// Two fields in one window: the ref names field A, but the app keeps focus
/// on field B. Typing anyway would put the text into B, so nothing is sent.
#[test]
fn ref_type_refuses_when_focus_stays_on_another_field() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(Some(WINDOW_ID));
    let mut adapter = KeyboardCaptureAdapter::new();
    adapter.focus = FocusBehavior::StaysOnAnotherField;

    let err = execute(
        type_args("meant for field A", snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(adapter.calls(), ["action:focus"]);
    assert_refused_before_posting(&err);
}

#[test]
fn failed_accessibility_focus_refuses_delivery() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(Some(WINDOW_ID));
    let mut adapter = KeyboardCaptureAdapter::new();
    adapter.focus = FocusBehavior::Fails(ErrorCode::ActionFailed);

    let err = execute(
        type_args("hello", snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert!(adapter.delivered().is_empty());
    assert_refused_before_posting(&err);
}

fn assert_refused_before_posting(err: &AppError) {
    let AppError::Adapter(error) = err else {
        panic!("expected an adapter error, got {err:?}");
    };
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|hint| hint.contains("--window-id")),
        "the hint must point at the window-targeted fallback"
    );
}

#[test]
fn window_type_posts_text_to_the_window_without_any_focus_request() {
    let adapter = KeyboardCaptureAdapter::new();

    let value = execute(
        window_type_args("into the focused field"),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(adapter.calls(), ["background_keys"]);
    let (window, input) = adapter.delivered().remove(0);
    assert_eq!(window.id, WINDOW_ID);
    assert!(matches!(input, BackgroundKeyInput::Text(text) if text == "into the focused field"));
    assert_eq!(value["typed"], true);
    assert!(value["background"].get("ax_focus").is_none());
}

#[test]
fn stale_ref_is_not_delivered() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(Some(WINDOW_ID));
    let mut adapter = KeyboardCaptureAdapter::new();
    adapter.stale_ref = true;

    let err = execute(
        type_args("hello", snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "STALE_REF");
    assert!(adapter.delivered().is_empty());
}

#[test]
fn ref_without_exact_source_window_is_rejected() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(None);
    let adapter = KeyboardCaptureAdapter::new();

    let err = execute(
        type_args("hello", snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ACTION_NOT_SUPPORTED");
    assert!(adapter.calls().is_empty());
}

#[test]
fn window_owned_by_a_different_process_is_rejected_before_posting() {
    let mut adapter = KeyboardCaptureAdapter::new();
    adapter.live_pid = PID + 1;

    let err = execute(
        press_args("enter", false),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "STALE_REF");
    assert!(adapter.delivered().is_empty());
}

#[test]
fn empty_or_oversized_text_is_rejected_before_resolution() {
    let adapter = KeyboardCaptureAdapter::new();
    for text in [String::new(), "x".repeat(MAX_TEXT_LEN + 1)] {
        let err = execute(
            type_args(&text, "s1".into()),
            &adapter,
            &CommandContext::default(),
        )
        .unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGS");
    }
    assert!(adapter.calls().is_empty());
}

#[test]
fn deadline_grows_with_the_text_so_pacing_is_never_clipped() {
    let combo = delivery_deadline(Some(1_000), &BackgroundKeyInput::Text("x".into())).unwrap();
    let long = delivery_deadline(Some(1_000), &BackgroundKeyInput::Text("x".repeat(400))).unwrap();

    assert!(combo.remaining_ms() <= 1_000 + DELIVERY_BUDGET_MS + TEXT_BUDGET_PER_CHAR_MS);
    assert!(long.remaining_ms() > 1_000 + DELIVERY_BUDGET_MS + 399 * TEXT_BUDGET_PER_CHAR_MS - 500);
}

#[test]
fn an_enclosing_batch_deadline_bounds_background_text() {
    let batch = crate::Deadline::after(50).unwrap();
    let _scope = crate::deadline::enter_scope(Some(batch));

    let deadline =
        delivery_deadline(Some(5_000), &BackgroundKeyInput::Text("x".repeat(400))).unwrap();

    assert!(deadline.remaining_ms() <= 50);
}

#[test]
fn frontmost_change_is_reported_with_a_warning() {
    let mut adapter = KeyboardCaptureAdapter::new();
    adapter.report.frontmost_pid_after = Some(ProcessId::new(PID));
    adapter.report.degradations = vec!["activate:SLPSPostEventRecordTo_unavailable".into()];

    let value = execute(press_args("a", false), &adapter, &CommandContext::default()).unwrap();

    assert_eq!(value["background"]["focus_change"], "changed");
    assert_eq!(
        value["background"]["degraded"][0],
        "activate:SLPSPostEventRecordTo_unavailable"
    );
    assert!(value["warning"].is_string());
}
