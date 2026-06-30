use super::test_support::{entry, text_entry};
use super::*;
use crate::adapter::{NativeHandle, WindowFilter};
use crate::context::WaitSelector;
use crate::error::AdapterError;
use crate::node::{AccessibilityNode, WindowInfo};
use crate::refs::RefMap;
use crate::refs_test_support::HomeGuard;
use crate::{
    action::Action, action_result::ActionResult, action_step::ActionStep,
    element_state::ElementState, interaction_policy::InteractionPolicy,
};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

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

    execute_ref_action_with_context(
        args,
        &adapter,
        ActionRequest::headless(Action::Click),
        &CommandContext::default(),
    )
    .unwrap();

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

    let err = execute_ref_action_with_context(
        args,
        &adapter,
        ActionRequest::headless(Action::Click),
        &CommandContext::default(),
    )
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

struct ScopedWaitAdapter {
    request: Mutex<Option<ActionRequest>>,
    polled_app: Mutex<Option<String>>,
}

impl PlatformAdapter for ScopedWaitAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
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

    fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        *self.polled_app.lock().unwrap() = filter.app.clone();
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: filter.app.clone().unwrap_or_else(|| "TargetApp".into()),
            pid: 1,
            bounds: None,
            is_focused: true,
        }])
    }

    fn get_tree(
        &self,
        _win: &WindowInfo,
        _opts: &crate::adapter::TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        Ok(AccessibilityNode {
            ref_id: None,
            role: "window".into(),
            name: Some("Saved!".into()),
            value: None,
            description: None,
            hint: None,
            states: vec![],
            available_actions: vec![],
            bounds: None,
            children_count: None,
            children: vec![],
        })
    }
}

#[test]
fn post_action_wait_scopes_to_source_app_and_merges_action_result() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    let mut entry = entry();
    entry.source_app = Some("TargetApp".into());
    refmap.allocate(entry);
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = ScopedWaitAdapter {
        request: Mutex::new(None),
        polled_app: Mutex::new(None),
    };
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: ":saved!".into(),
        gone: false,
        timeout_ms: 5_000,
    }));
    let args = RefArgs {
        ref_id: "@e1".into(),
        snapshot_id: Some(snapshot_id),
    };

    let value = execute_ref_action_with_context(
        args,
        &adapter,
        ActionRequest::headless(Action::Click),
        &context,
    )
    .unwrap();

    assert_eq!(
        adapter.polled_app.lock().unwrap().as_deref(),
        Some("TargetApp")
    );
    assert_eq!(value["after_action"]["action"], "ok");
    assert_eq!(value["matched_selector"], ":saved!");
}

#[test]
fn post_action_wait_without_flag_returns_action_only() {
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

    let value = execute_ref_action_with_context(
        args,
        &adapter,
        ActionRequest::headless(Action::Click),
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["action"], "ok");
    assert!(value.get("after_action").is_none());
}
