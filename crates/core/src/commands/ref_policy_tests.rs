use crate::{
    action::{Action, ActionRequest, ActionResult, Direction, InteractionPolicy},
    adapter::{NativeHandle, PlatformAdapter},
    commands::{
        check, clear, click, collapse, double_click, expand, focus, helpers::RefArgs, right_click,
        scroll, scroll_to, select, set_value, toggle, triple_click, type_text, uncheck,
    },
    context::CommandContext,
    error::AdapterError,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::Mutex;

struct RecordingAdapter {
    requests: Mutex<Vec<ActionRequest>>,
}

impl RecordingAdapter {
    fn new() -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
        }
    }

    fn last_request(&self) -> ActionRequest {
        self.requests.lock().unwrap().last().cloned().unwrap()
    }
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
        self.requests.lock().unwrap().push(request);
        Ok(ActionResult::new("ok"))
    }
}

fn snapshot_id() -> String {
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        pid: 1,
        role: "textfield".into(),
        name: Some("Target".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec![
            "Check".into(),
            "Clear".into(),
            "Click".into(),
            "Collapse".into(),
            "DoubleClick".into(),
            "Expand".into(),
            "RightClick".into(),
            "Scroll".into(),
            "Select".into(),
            "SetFocus".into(),
            "SetValue".into(),
            "Toggle".into(),
            "TripleClick".into(),
            "TypeText".into(),
            "Uncheck".into(),
        ],
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

fn ref_args(snapshot_id: &str) -> RefArgs {
    RefArgs {
        ref_id: "@e1".into(),
        snapshot_id: Some(snapshot_id.to_owned()),
    }
}

fn assert_headless(request: &ActionRequest) {
    assert_eq!(request.policy, InteractionPolicy::headless());
}

#[test]
fn default_ref_commands_are_headless() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_id();
    let adapter = RecordingAdapter::new();
    let context = CommandContext::default();

    click::execute(ref_args(&snapshot_id), &adapter, &context).unwrap();
    double_click::execute(ref_args(&snapshot_id), &adapter, &context).unwrap();
    triple_click::execute(ref_args(&snapshot_id), &adapter, &context).unwrap();
    let before_right_click = adapter.requests.lock().unwrap().len();
    let _ = right_click::execute(ref_args(&snapshot_id), &adapter);
    assert_eq!(
        adapter.requests.lock().unwrap().len(),
        before_right_click + 1
    );
    clear::execute(ref_args(&snapshot_id), &adapter, &context).unwrap();
    toggle::execute(ref_args(&snapshot_id), &adapter, &context).unwrap();
    check::execute(ref_args(&snapshot_id), &adapter, &context).unwrap();
    uncheck::execute(ref_args(&snapshot_id), &adapter, &context).unwrap();
    expand::execute(ref_args(&snapshot_id), &adapter, &context).unwrap();
    collapse::execute(ref_args(&snapshot_id), &adapter, &context).unwrap();
    scroll_to::execute(ref_args(&snapshot_id), &adapter, &context).unwrap();
    set_value::execute(
        set_value::SetValueArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id.clone()),
            value: "value".into(),
        },
        &adapter,
        &context,
    )
    .unwrap();
    select::execute(
        select::SelectArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id.clone()),
            value: "choice".into(),
        },
        &adapter,
        &context,
    )
    .unwrap();
    let before_type = adapter.requests.lock().unwrap().len();
    type_text::execute(
        type_text::TypeArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id.clone()),
            text: "text".into(),
        },
        &adapter,
        &context,
    )
    .unwrap();
    let type_request = adapter.requests.lock().unwrap()[before_type].clone();
    assert_eq!(type_request.policy, InteractionPolicy::focus_fallback());
    scroll::execute(
        scroll::ScrollArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            direction: Direction::Down,
            amount: 1,
        },
        &adapter,
        &context,
    )
    .unwrap();

    for request in adapter.requests.lock().unwrap().iter() {
        if matches!(request.action, Action::TypeText(_)) {
            continue;
        }
        assert_headless(request);
    }
}

#[test]
fn focus_command_is_explicit_headless_policy() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_id();
    let adapter = RecordingAdapter::new();

    focus::execute(ref_args(&snapshot_id), &adapter, &CommandContext::default()).unwrap();

    let request = adapter.last_request();
    assert!(matches!(request.action, Action::SetFocus));
    assert_eq!(request.policy, InteractionPolicy::headless());
}
