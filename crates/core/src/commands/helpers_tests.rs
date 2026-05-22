use super::*;
use crate::action::{Action, ActionResult, InteractionPolicy};
use crate::adapter::NativeHandle;
use crate::error::{AdapterError, ErrorCode};
use crate::node::AppInfo;
use crate::refs::RefMap;
use crate::refs_test_support::HomeGuard;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

struct ReleaseCountingAdapter {
    releases: AtomicU32,
}

impl PlatformAdapter for ReleaseCountingAdapter {
    fn resolve_element(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn release_handle(&self, _handle: &NativeHandle) -> Result<(), AdapterError> {
        self.releases.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct RecordingAdapter {
    request: Mutex<Option<ActionRequest>>,
}

impl PlatformAdapter for RecordingAdapter {
    fn resolve_element(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        *self.request.lock().unwrap() = Some(request);
        Ok(ActionResult::new("ok"))
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

fn entry() -> RefEntry {
    RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("OK".into()),
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
        let (_entry, resolved) = resolve_ref("@e1", Some(&snapshot_id), &adapter).unwrap();
        let _handle = resolved.handle();
        assert_eq!(adapter.releases.load(Ordering::SeqCst), 0);
    }

    assert_eq!(adapter.releases.load(Ordering::SeqCst), 1);
}

#[test]
fn execute_ref_action_preserves_action_and_policy() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    refmap.allocate(entry());
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = RecordingAdapter {
        request: Mutex::new(None),
    };
    let args = RefArgs {
        ref_id: "@e1".into(),
        snapshot_id: Some(snapshot_id),
    };

    execute_ref_action(args, &adapter, ActionRequest::headless(Action::Clear)).unwrap();

    let request = adapter.request.lock().unwrap().clone().unwrap();
    assert!(matches!(request.action, Action::Clear));
    assert_eq!(request.policy, InteractionPolicy::headless());
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
