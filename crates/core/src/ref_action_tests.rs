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
