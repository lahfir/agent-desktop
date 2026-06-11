use super::test_support::{entry, text_entry};
use super::*;
use crate::adapter::NativeHandle;
use crate::error::AdapterError;
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
