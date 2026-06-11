use super::test_support::entry;
use super::*;
use crate::action::Action;
use crate::adapter::NativeHandle;
use crate::error::{AdapterError, ErrorCode};
use crate::node::AppInfo;
use crate::refs::RefMap;
use crate::refs_test_support::HomeGuard;
use std::sync::atomic::{AtomicU32, Ordering};

struct ReleaseCountingAdapter {
    releases: AtomicU32,
}

impl PlatformAdapter for ReleaseCountingAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn release_handle(&self, _handle: &NativeHandle) -> Result<(), AdapterError> {
        self.releases.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct RestoreWithoutWindowAdapter {
    op_count: AtomicU32,
}

impl PlatformAdapter for RestoreWithoutWindowAdapter {
    fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![AppInfo {
            name: "TextEdit".into(),
            pid: 42,
            bundle_id: None,
        }])
    }

    fn list_windows(
        &self,
        _filter: &crate::adapter::WindowFilter,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Err(AdapterError::new(ErrorCode::WindowNotFound, "no windows"))
    }

    fn window_op(&self, win: &WindowInfo, op: WindowOp) -> Result<(), AdapterError> {
        assert_eq!(win.pid, 42);
        assert!(matches!(op, WindowOp::Restore));
        self.op_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn resolved_element_releases_handle_once_on_drop() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    refmap.allocate(entry());
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = ReleaseCountingAdapter {
        releases: AtomicU32::new(0),
    };

    {
        let (_entry, resolved) = resolve_ref_with_context(
            "@e1",
            Some(&snapshot_id),
            &adapter,
            &CommandContext::default(),
        )
        .unwrap();
        let _handle = resolved.handle();
        assert_eq!(adapter.releases.load(Ordering::SeqCst), 0);
    }

    assert_eq!(adapter.releases.load(Ordering::SeqCst), 1);
}

#[test]
fn explicit_session_snapshot_resolves_without_session_context() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    refmap.allocate(entry());
    let snapshot_id = RefStore::for_session(Some("agent-a"))
        .unwrap()
        .save_new_snapshot(&refmap)
        .unwrap();
    let adapter = ReleaseCountingAdapter {
        releases: AtomicU32::new(0),
    };

    let (_entry, resolved) = resolve_ref_with_context(
        "@e1",
        Some(&snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();
    let _handle = resolved.handle();

    assert_eq!(adapter.releases.load(Ordering::SeqCst), 0);
}

#[test]
fn missing_snapshot_keeps_snapshot_not_found_error() {
    let _guard = HomeGuard::new();
    let adapter = ReleaseCountingAdapter {
        releases: AtomicU32::new(0),
    };

    let err = match resolve_ref_with_context(
        "@e1",
        Some("smissing"),
        &adapter,
        &CommandContext::default(),
    ) {
        Ok(_) => panic!("expected missing snapshot to fail"),
        Err(err) => err,
    };

    assert_eq!(err.code(), "SNAPSHOT_NOT_FOUND");
    assert!(err.suggestion().unwrap().contains("snapshot_id"));
}

#[test]
fn restore_can_run_when_no_window_is_currently_listed() {
    let adapter = RestoreWithoutWindowAdapter {
        op_count: AtomicU32::new(0),
    };

    let value = window_op_command(
        AppArgs {
            app: Some("TextEdit".into()),
        },
        &adapter,
        WindowOp::Restore,
        "restored",
    )
    .unwrap();

    assert_eq!(value["restored"], true);
    assert_eq!(adapter.op_count.load(Ordering::SeqCst), 1);
}

struct CountingPipelineAdapter {
    resolves: AtomicU32,
    live_reads: AtomicU32,
    executes: AtomicU32,
    releases: AtomicU32,
}

impl CountingPipelineAdapter {
    fn new() -> Self {
        Self {
            resolves: AtomicU32::new(0),
            live_reads: AtomicU32::new(0),
            executes: AtomicU32::new(0),
            releases: AtomicU32::new(0),
        }
    }
}

impl PlatformAdapter for CountingPipelineAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        self.resolves.fetch_add(1, Ordering::SeqCst);
        Ok(NativeHandle::null())
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
    ) -> Result<crate::adapter::LiveElement, AdapterError> {
        self.live_reads.fetch_add(1, Ordering::SeqCst);
        Ok(crate::adapter::LiveElement::default())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<crate::action_result::ActionResult, AdapterError> {
        self.executes.fetch_add(1, Ordering::SeqCst);
        Ok(crate::action_result::ActionResult::new("click"))
    }

    fn release_handle(&self, _handle: &NativeHandle) -> Result<(), AdapterError> {
        self.releases.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn ref_action_pipeline_makes_one_resolve_one_preflight_one_dispatch() {
    let _guard = HomeGuard::new();
    let store = crate::refs_store::RefStore::new().unwrap();
    let mut refmap = RefMap::new();
    refmap.allocate(entry());
    let snapshot_id = store.save_new_snapshot(&refmap).unwrap();
    let adapter = CountingPipelineAdapter::new();

    execute_ref_action_result_with_context(
        "@e1",
        Some(&snapshot_id),
        &adapter,
        ActionRequest::headless(Action::Click),
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(adapter.resolves.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.live_reads.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.executes.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.releases.load(Ordering::SeqCst), 1);
}
