use super::*;
use crate::{
    adapter::{NativeHandle, PlatformAdapter},
    commands::wait_predicate,
    error::{AdapterError, ErrorCode},
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::Mutex;
use std::time::Duration;

use super::test_support::wait_for_element_test;

struct AmbiguousResolveAdapter;

impl PlatformAdapter for AmbiguousResolveAdapter {
    fn resolve_element_strict_with_timeout(
        &self,
        _entry: &RefEntry,
        _timeout: Duration,
    ) -> Result<NativeHandle, AdapterError> {
        Err(AdapterError::ambiguous_target("2 candidates matched"))
    }
}

struct TransientResolveAdapter {
    errors: Mutex<Vec<ErrorCode>>,
}

impl PlatformAdapter for TransientResolveAdapter {
    fn resolve_element_strict_with_timeout(
        &self,
        _entry: &RefEntry,
        _timeout: Duration,
    ) -> Result<NativeHandle, AdapterError> {
        if let Some(code) = self.errors.lock().unwrap().pop() {
            return Err(AdapterError::new(code, "transient resolution failure"));
        }
        Ok(NativeHandle::null())
    }
}

struct PermissionResolveAdapter;

impl PlatformAdapter for PermissionResolveAdapter {
    fn resolve_element_strict_with_timeout(
        &self,
        _entry: &RefEntry,
        _timeout: Duration,
    ) -> Result<NativeHandle, AdapterError> {
        Err(AdapterError::permission_denied())
    }
}

struct StrictOnlyResolveAdapter;

impl PlatformAdapter for StrictOnlyResolveAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }
}

struct TimeoutCaptureAdapter {
    captured_ms: Mutex<Vec<u128>>,
}

impl PlatformAdapter for TimeoutCaptureAdapter {
    fn resolve_element_strict_with_timeout(
        &self,
        _entry: &RefEntry,
        timeout: Duration,
    ) -> Result<NativeHandle, AdapterError> {
        self.captured_ms.lock().unwrap().push(timeout.as_millis());
        Ok(NativeHandle::null())
    }
}

fn snapshot_with_one_ref() -> String {
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("Run".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec!["Click".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
    RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap()
}

#[test]
fn element_wait_retries_transient_ambiguous_resolution() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();
    let adapter = TransientResolveAdapter {
        errors: Mutex::new(vec![ErrorCode::AmbiguousTarget]),
    };

    let value = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Exists,
        250,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["predicate"], "exists");
    assert_eq!(value["observed"]["exists"], true);
}

#[test]
fn element_wait_retries_transient_resolution_timeout() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();
    let adapter = TransientResolveAdapter {
        errors: Mutex::new(vec![ErrorCode::Timeout]),
    };

    let value = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Exists,
        250,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["observed"]["exists"], true);
}

#[test]
fn element_wait_passes_remaining_budget_to_resolver() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();
    let adapter = TimeoutCaptureAdapter {
        captured_ms: Mutex::new(vec![]),
    };

    let value = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Exists,
        75,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["observed"]["exists"], true);
    let captured = adapter.captured_ms.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert!(captured[0] <= 75);
}

#[test]
fn element_wait_delegates_to_strict_only_resolution() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();

    let value = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Exists,
        250,
        &StrictOnlyResolveAdapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["found"], true);
    assert_eq!(value["observed"]["exists"], true);
}

#[test]
fn element_wait_times_out_after_persistent_ambiguous_resolution() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();

    let err = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Exists,
        1,
        &AmbiguousResolveAdapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "TIMEOUT");
    match err {
        AppError::Adapter(adapter_error) => {
            assert_eq!(
                adapter_error.details.unwrap()["last_observed"]["error"],
                "AMBIGUOUS_TARGET"
            );
        }
        _ => panic!("expected adapter error"),
    }
}

#[test]
fn element_wait_aborts_terminal_permission_error() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();

    let err = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Exists,
        250,
        &PermissionResolveAdapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "PERM_DENIED");
}
