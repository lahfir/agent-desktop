use super::test_support::entry;
use super::*;
use crate::AppInfo;
use crate::action::Action;
use crate::adapter::NativeHandle;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::capability;
use crate::refs::RefMap;
use crate::refs_test_support::HomeGuard;
use crate::{AdapterError, ErrorCode, WindowInfo, WindowOp};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

struct DropProbe(Arc<AtomicU32>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct DropCountingAdapter {
    drops: Arc<AtomicU32>,
}

impl ObservationOps for DropCountingAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::new(DropProbe(Arc::clone(&self.drops))))
    }
}

impl ActionOps for DropCountingAdapter {}

impl InputOps for DropCountingAdapter {}

impl SystemOps for DropCountingAdapter {}

struct RestoreWithoutWindowAdapter {
    op_count: AtomicU32,
}

struct TargetedWindowAdapter {
    windows: Vec<WindowInfo>,
    operated: AtomicU32,
}

impl ObservationOps for TargetedWindowAdapter {
    fn list_windows(
        &self,
        _filter: &crate::adapter::WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(self.windows.clone())
    }
}

impl ActionOps for TargetedWindowAdapter {}

impl InputOps for TargetedWindowAdapter {}

impl SystemOps for TargetedWindowAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn resolve_window_strict(
        &self,
        win: &WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        Ok(win.clone())
    }

    fn window_op(
        &self,
        win: &WindowInfo,
        op: WindowOp,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        assert_eq!(win.id, "w-real");
        assert!(matches!(op, WindowOp::Restore));
        self.operated.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn targeted_window(id: &str, visible: bool) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: "Fixture".into(),
        app: "Fixture".into(),
        pid: crate::ProcessId::new(42),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: crate::WindowState {
            visible: Some(visible),
            ..Default::default()
        },
    }
}

impl ObservationOps for RestoreWithoutWindowAdapter {
    fn list_apps(&self, _deadline: crate::Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![AppInfo {
            name: "TextEdit".into(),
            pid: crate::ProcessId::new(42),
            bundle_id: None,
            process_instance: Some("test-instance".into()),
        }])
    }

    fn list_windows(
        &self,
        _filter: &crate::adapter::WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Err(AdapterError::new(ErrorCode::WindowNotFound, "no windows"))
    }
}

impl ActionOps for RestoreWithoutWindowAdapter {}

impl InputOps for RestoreWithoutWindowAdapter {}

impl SystemOps for RestoreWithoutWindowAdapter {
    fn window_op(
        &self,
        win: &WindowInfo,
        op: WindowOp,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        assert_eq!(win.pid, 42);
        assert!(matches!(op, WindowOp::Restore));
        self.op_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn normalize_action_timeout_ms_treats_zero_as_disabled() {
    assert_eq!(normalize_action_timeout_ms(0), None);
    assert_eq!(normalize_action_timeout_ms(1), Some(1));
}

#[test]
fn resolved_native_handle_drops_payload_once() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    refmap.allocate(entry());
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = DropCountingAdapter {
        drops: Arc::new(AtomicU32::new(0)),
    };

    {
        let (_entry, handle) = resolve_ref_with_context(
            "@e1",
            Some(&snapshot_id),
            &adapter,
            &CommandContext::default(),
        )
        .unwrap();
        assert!(!handle.is_null());
        assert_eq!(adapter.drops.load(Ordering::SeqCst), 0);
    }

    assert_eq!(adapter.drops.load(Ordering::SeqCst), 1);
}

#[test]
fn explicit_session_snapshot_resolves_with_matching_session_context() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    refmap.allocate(entry());
    let snapshot_id = RefStore::for_session(Some("agent-a"))
        .unwrap()
        .save_new_snapshot(&refmap)
        .unwrap();
    let adapter = DropCountingAdapter {
        drops: Arc::new(AtomicU32::new(0)),
    };

    let (_entry, handle) = resolve_ref_with_context(
        "@e1",
        Some(&snapshot_id),
        &adapter,
        &CommandContext::new(Some("agent-a".into()), None, false).unwrap(),
    )
    .unwrap();
    assert!(!handle.is_null());

    assert_eq!(adapter.drops.load(Ordering::SeqCst), 0);
}

#[test]
fn missing_snapshot_keeps_snapshot_not_found_error() {
    let _guard = HomeGuard::new();
    let adapter = DropCountingAdapter {
        drops: Arc::new(AtomicU32::new(0)),
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
fn restore_fails_closed_when_no_window_is_currently_listed() {
    let adapter = RestoreWithoutWindowAdapter {
        op_count: AtomicU32::new(0),
    };

    let err = window_op_command(
        AppArgs {
            app: Some("TextEdit".into()),
            window_id: None,
        },
        &adapter,
        WindowOp::Restore,
        "restored",
    )
    .unwrap_err();

    assert_eq!(err.code(), "WINDOW_NOT_FOUND");
    assert_eq!(adapter.op_count.load(Ordering::SeqCst), 0);
}

#[test]
fn window_id_targets_one_window_in_an_ambiguous_app_inventory() {
    let adapter = TargetedWindowAdapter {
        windows: vec![
            targeted_window("w-phantom", false),
            targeted_window("w-real", true),
        ],
        operated: AtomicU32::new(0),
    };

    let value = window_op_command(
        AppArgs {
            app: Some("Fixture".into()),
            window_id: Some("w-real".into()),
        },
        &adapter,
        WindowOp::Restore,
        "restored",
    )
    .unwrap();

    assert_eq!(value["restored"], true);
    assert_eq!(adapter.operated.load(Ordering::SeqCst), 1);
}

struct CountingPipelineAdapter {
    resolves: AtomicU32,
    live_reads: AtomicU32,
    executes: AtomicU32,
    drops: Arc<AtomicU32>,
}

impl CountingPipelineAdapter {
    fn new() -> Self {
        Self {
            resolves: AtomicU32::new(0),
            live_reads: AtomicU32::new(0),
            executes: AtomicU32::new(0),
            drops: Arc::new(AtomicU32::new(0)),
        }
    }
}

impl ObservationOps for CountingPipelineAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolves.fetch_add(1, Ordering::SeqCst);
        Ok(NativeHandle::new(DropProbe(Arc::clone(&self.drops))))
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<crate::adapter::LiveElement, AdapterError> {
        self.live_reads.fetch_add(1, Ordering::SeqCst);
        Ok(crate::adapter::LiveElement {
            identity: crate::adapter::live_identity("OK"),
            state: crate::element_state::ElementState {
                role: "button".into(),
                states: vec![],
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(crate::Rect {
                x: 1.0,
                y: 1.0,
                width: 20.0,
                height: 20.0,
            }),
            available_actions: vec![capability::CLICK.into()],
        })
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::Rect>, AdapterError> {
        Ok(Some(crate::Rect {
            x: 1.0,
            y: 1.0,
            width: 20.0,
            height: 20.0,
        }))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<crate::hit_test::HitTestResult, AdapterError> {
        Ok(crate::hit_test::HitTestResult::ReachesTarget)
    }
}

impl ActionOps for CountingPipelineAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::action_result::ActionResult, AdapterError> {
        self.executes.fetch_add(1, Ordering::SeqCst);
        Ok(crate::action_result::ActionResult::delivered_unverified(
            "click",
        ))
    }
}

impl InputOps for CountingPipelineAdapter {}

impl SystemOps for CountingPipelineAdapter {
    crate::adapter::guarded_interaction_lease!();
}

#[test]
fn ref_action_pipeline_resolves_under_lease_before_dispatch() {
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
    assert_eq!(adapter.drops.load(Ordering::SeqCst), 1);
}
