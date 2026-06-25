use super::*;
use crate::{
    action::Action,
    action_result::ActionResult,
    adapter::{NativeHandle, SnapshotSurface},
    capability,
};
use std::sync::atomic::{AtomicU32, Ordering};

struct ReleaseFailingAdapter;

impl PlatformAdapter for ReleaseFailingAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::new("click"))
    }

    fn release_handle(&self, _handle: &NativeHandle) -> Result<(), AdapterError> {
        Err(AdapterError::internal("release failed"))
    }
}

struct ErrorReleasingAdapter {
    releases: AtomicU32,
}

impl PlatformAdapter for ErrorReleasingAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::internal("dispatch failed"))
    }

    fn release_handle(&self, _handle: &NativeHandle) -> Result<(), AdapterError> {
        self.releases.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn entry() -> RefEntry {
    RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("Run".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec![capability::CLICK.into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    }
}

#[test]
fn successful_action_survives_release_failure() {
    let result = execute_entry(
        &ReleaseFailingAdapter,
        &entry(),
        ActionRequest::headless(Action::Click),
    )
    .unwrap();

    assert_eq!(result.action, "click");
}

#[test]
fn failed_action_still_releases_resolved_handle() {
    let adapter = ErrorReleasingAdapter {
        releases: AtomicU32::new(0),
    };

    let err =
        execute_entry(&adapter, &entry(), ActionRequest::headless(Action::Click)).unwrap_err();

    assert_eq!(err.code, crate::error::ErrorCode::Internal);
    assert_eq!(adapter.releases.load(Ordering::SeqCst), 1);
}

#[test]
fn execute_entry_with_context_succeeds_and_matches_execute_entry() {
    let context = CommandContext::default();
    let result = execute_entry_with_context(
        &ReleaseFailingAdapter,
        &entry(),
        ActionRequest::headless(Action::Click),
        &context,
    )
    .unwrap();

    assert_eq!(result.action, "click");
}

#[test]
fn execute_entry_delegates_to_entry_with_context() {
    let adapter = ErrorReleasingAdapter {
        releases: AtomicU32::new(0),
    };

    let err =
        execute_entry(&adapter, &entry(), ActionRequest::headless(Action::Click)).unwrap_err();

    assert_eq!(err.code, crate::error::ErrorCode::Internal);
    assert_eq!(adapter.releases.load(Ordering::SeqCst), 1);
}

#[test]
fn execute_entry_with_context_emits_trace_events() {
    let trace_path = std::env::temp_dir().join(format!(
        "agent-desktop-ref-action-trace-{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let context =
        CommandContext::new(Some("test-session".into()), Some(trace_path.clone()), false).unwrap();

    let _ = execute_entry_with_context(
        &ReleaseFailingAdapter,
        &entry(),
        ActionRequest::headless(Action::Click),
        &context,
    );

    let body = std::fs::read_to_string(&trace_path).unwrap();
    assert!(
        body.contains("actionability.check.start"),
        "expected actionability trace event"
    );
    assert!(
        body.contains("action.dispatch.start"),
        "expected dispatch trace event"
    );
    let _ = std::fs::remove_file(trace_path);
}
