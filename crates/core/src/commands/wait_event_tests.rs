use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::node::WindowInfo;

struct SequenceAdapter {
    snapshots: Vec<SignalBaseline>,
    calls: std::sync::Mutex<usize>,
}

impl SequenceAdapter {
    fn new(snapshots: Vec<SignalBaseline>) -> Self {
        Self {
            snapshots,
            calls: std::sync::Mutex::new(0),
        }
    }
}

impl ObservationOps for SequenceAdapter {}
impl ActionOps for SequenceAdapter {}
impl InputOps for SequenceAdapter {}

impl SystemOps for SequenceAdapter {
    fn capture_signal_baseline(
        &self,
        _filter: &SignalFilter,
    ) -> Result<SignalBaseline, AdapterError> {
        let mut calls = self.calls.lock().unwrap();
        let idx = (*calls).min(self.snapshots.len().saturating_sub(1));
        *calls += 1;
        Ok(self.snapshots[idx].clone())
    }
}

fn empty_baseline() -> SignalBaseline {
    SignalBaseline {
        windows: Vec::new(),
        apps: Vec::new(),
        surfaces: Vec::new(),
    }
}

fn baseline_with_windows(windows: Vec<WindowInfo>) -> SignalBaseline {
    SignalBaseline {
        windows,
        apps: Vec::new(),
        surfaces: Vec::new(),
    }
}

fn window(id: &str, title: &str) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: title.into(),
        app: "TextEdit".into(),
        pid: 42,
        bounds: None,
        is_focused: true,
    }
}

#[test]
fn window_opened_matches_without_caller_supplied_id_or_title() {
    let adapter = SequenceAdapter::new(vec![
        empty_baseline(),
        empty_baseline(),
        baseline_with_windows(vec![window("w-99", "Untitled")]),
    ]);

    let result = wait_for_event(
        "window-opened",
        Some("TextEdit".into()),
        None,
        None,
        5_000,
        &adapter,
    )
    .unwrap();

    assert_eq!(result["found"], true);
    assert_eq!(result["event"]["window_id"], "w-99");
    assert_eq!(result["event"]["title"], "Untitled");
}

#[test]
fn window_id_narrows_match_to_the_named_window() {
    let adapter = SequenceAdapter::new(vec![
        empty_baseline(),
        baseline_with_windows(vec![window("w-1", "Untitled"), window("w-2", "Untitled")]),
    ]);

    let result = wait_for_event(
        "window-opened",
        None,
        Some("w-2".into()),
        None,
        5_000,
        &adapter,
    )
    .unwrap();

    assert_eq!(result["event"]["window_id"], "w-2");
}

#[test]
fn timeout_carries_wait_timeout_kind_in_details() {
    let adapter = SequenceAdapter::new(vec![empty_baseline()]);

    let err = wait_for_event("window-opened", None, None, None, 50, &adapter).unwrap_err();

    let AppError::Adapter(adapter_err) = err else {
        panic!("expected AppError::Adapter");
    };
    assert_eq!(adapter_err.code, ErrorCode::Timeout);
    let details = adapter_err.details.expect("timeout must carry details");
    assert_eq!(details["kind"], "wait_timeout");
    assert_eq!(details["predicate"], "event");
    assert_eq!(details["event"], "window-opened");
}

#[test]
fn unknown_event_value_returns_invalid_args() {
    let adapter = SequenceAdapter::new(vec![empty_baseline()]);

    let err = wait_for_event("bogus-event", None, None, None, 10, &adapter).unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn parse_event_kind_accepts_every_documented_token() {
    for token in EventKind::all_tokens() {
        assert!(
            parse_event_kind(token).is_ok(),
            "token '{token}' from EventKind::all_tokens() must parse"
        );
    }
}
