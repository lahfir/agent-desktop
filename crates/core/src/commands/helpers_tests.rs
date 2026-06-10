use super::*;
use crate::adapter::NativeHandle;
use crate::error::{AdapterError, ErrorCode};
use crate::node::AppInfo;
use crate::refs::RefMap;
use crate::refs_test_support::HomeGuard;
use crate::{
    action::Action, action_result::ActionResult, action_step::ActionStep,
    element_state::ElementState, interaction_policy::InteractionPolicy,
};
use std::sync::Mutex;
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

struct RecordingAdapter {
    request: Mutex<Option<ActionRequest>>,
}

impl PlatformAdapter for RecordingAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        *self.request.lock().unwrap() = Some(request);
        Ok(ActionResult::new("ok")
            .with_state(ElementState {
                role: "textfield".into(),
                states: vec!["focused".into()],
                value: Some("updated".into()),
            })
            .with_steps(vec![ActionStep::succeeded("AXPress")]))
    }
}

struct AmbiguousAdapter {
    executed: AtomicU32,
}

impl PlatformAdapter for AmbiguousAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Err(
            AdapterError::ambiguous_target("2 candidates matched").with_details(
                serde_json::json!({
                    "candidate_count": 2,
                    "candidates": [{ "name": "Private OK" }]
                }),
            ),
        )
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        self.executed.fetch_add(1, Ordering::SeqCst);
        Ok(ActionResult::new("unexpected"))
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
        available_actions: vec!["Clear".into(), "Click".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    }
}

fn text_entry() -> RefEntry {
    let mut entry = entry();
    entry.role = "textfield".into();
    entry.available_actions = vec!["SetValue".into()];
    entry
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

    let err = match resolve_ref("@e1", Some("smissing"), &adapter) {
        Ok(_) => panic!("expected missing snapshot to fail"),
        Err(err) => err,
    };

    assert_eq!(err.code(), "SNAPSHOT_NOT_FOUND");
    assert!(err.suggestion().unwrap().contains("snapshot_id"));
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

    execute_ref_action(args, &adapter, ActionRequest::headless(Action::Click)).unwrap();

    let request = adapter.request.lock().unwrap().clone().unwrap();
    assert!(matches!(request.action, Action::Click));
    assert_eq!(request.policy, InteractionPolicy::headless());
}

#[test]
fn execute_ref_action_does_not_dispatch_ambiguous_target() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    refmap.allocate(entry());
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = AmbiguousAdapter {
        executed: AtomicU32::new(0),
    };
    let args = RefArgs {
        ref_id: "@e1".into(),
        snapshot_id: Some(snapshot_id),
    };

    let err = execute_ref_action(args, &adapter, ActionRequest::headless(Action::Click))
        .expect_err("ambiguous targets fail before action dispatch");

    assert_eq!(err.code(), "AMBIGUOUS_TARGET");
    assert_eq!(adapter.executed.load(Ordering::SeqCst), 0);
}

#[test]
fn ref_action_trace_includes_ambiguous_details_without_candidate_names() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    refmap.allocate(entry());
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = AmbiguousAdapter {
        executed: AtomicU32::new(0),
    };
    let trace_path = std::env::temp_dir().join(format!(
        "agent-desktop-ambiguous-trace-{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let context = CommandContext::new(None, Some(trace_path.clone()), true).unwrap();
    let args = RefArgs {
        ref_id: "@e1".into(),
        snapshot_id: Some(snapshot_id),
    };

    let err = execute_ref_action_with_context(
        args,
        &adapter,
        ActionRequest::headless(Action::Click),
        &context,
    )
    .expect_err("ambiguous targets fail before action dispatch");

    assert_eq!(err.code(), "AMBIGUOUS_TARGET");
    let trace = std::fs::read_to_string(&trace_path).unwrap();
    assert!(trace.contains("\"candidate_count\":2"));
    assert!(!trace.contains("Private OK"));
    let _ = std::fs::remove_file(trace_path);
}

#[test]
fn ref_action_trace_does_not_include_typed_text_payload() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    refmap.allocate(text_entry());
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = RecordingAdapter {
        request: Mutex::new(None),
    };
    let trace_path = std::env::temp_dir().join(format!(
        "agent-desktop-type-trace-{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let context = CommandContext::new(None, Some(trace_path.clone()), true).unwrap();
    let args = RefArgs {
        ref_id: "@e1".into(),
        snapshot_id: Some(snapshot_id),
    };

    execute_ref_action_with_context(
        args,
        &adapter,
        ActionRequest::focus_fallback(Action::TypeText("super-secret".into())),
        &context,
    )
    .unwrap();

    let trace = std::fs::read_to_string(&trace_path).unwrap();
    assert!(trace.contains("\"action\":\"type\""));
    assert!(trace.contains("\"event\":\"action.dispatch.start\""));
    assert!(trace.contains("\"event\":\"action.dispatch.ok\""));
    assert!(trace.contains("\"post_state\""));
    assert!(trace.contains("\"steps\""));
    assert!(!trace.contains("super-secret"));
    let _ = std::fs::remove_file(trace_path);
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
