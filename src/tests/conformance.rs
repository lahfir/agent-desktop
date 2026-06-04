use agent_desktop_core::{
    action_request::ActionRequest,
    action_result::ActionResult,
    adapter::{LiveElement, NativeHandle, PlatformAdapter, SnapshotSurface},
    element_state::ElementState,
    error::{AdapterError, ErrorCode},
    node::Rect,
    refs::RefEntry,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

#[path = "../../tests/conformance/ref_action_contract.rs"]
mod ref_action_contract;

struct ContractAdapter {
    resolve: ResolveMode,
    live_bounds: Option<Rect>,
    dispatches: AtomicU32,
}

#[derive(Clone, Copy)]
enum ResolveMode {
    Ok,
    Stale,
    Ambiguous,
}

impl PlatformAdapter for ContractAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        self.resolve()
    }

    fn resolve_element_strict_with_timeout(
        &self,
        _entry: &RefEntry,
        _timeout: Duration,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve()
    }

    fn get_live_element(&self, _handle: &NativeHandle) -> Result<LiveElement, AdapterError> {
        Ok(LiveElement {
            state: Some(ElementState {
                role: "button".into(),
                states: vec![],
                value: None,
            }),
            bounds: self.live_bounds,
            available_actions: Some(vec!["Click".into()]),
        })
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        self.dispatches.fetch_add(1, Ordering::SeqCst);
        Ok(ActionResult::new("click"))
    }
}

impl ContractAdapter {
    fn new(resolve: ResolveMode, live_bounds: Option<Rect>) -> Self {
        Self {
            resolve,
            live_bounds,
            dispatches: AtomicU32::new(0),
        }
    }

    fn resolve(&self) -> Result<NativeHandle, AdapterError> {
        match self.resolve {
            ResolveMode::Ok => Ok(NativeHandle::null()),
            ResolveMode::Stale => Err(AdapterError::new(ErrorCode::StaleRef, "stale ref")),
            ResolveMode::Ambiguous => Err(AdapterError::ambiguous_target("2 candidates matched")),
        }
    }
}

fn entry(bounds: Rect) -> RefEntry {
    RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("OK".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: Some(bounds),
        bounds_hash: Some(bounds.bounds_hash()),
        available_actions: vec!["Click".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: true,
        path: Default::default(),
    }
}

#[test]
fn adapter_contract_dispatches_when_live_identity_moved() {
    let snapshot_bounds = Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    let adapter = ContractAdapter::new(
        ResolveMode::Ok,
        Some(Rect {
            x: 100.0,
            y: 100.0,
            width: 20.0,
            height: 20.0,
        }),
    );

    let result = ref_action_contract::run_click_command(&adapter, entry(snapshot_bounds)).unwrap();

    assert_eq!(result["action"], "click");
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 1);
}

#[test]
fn adapter_contract_dispatches_when_live_identity_is_stable() {
    let bounds = Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    let adapter = ContractAdapter::new(ResolveMode::Ok, Some(bounds));

    let result = ref_action_contract::run_click_command(&adapter, entry(bounds)).unwrap();

    assert_eq!(result["action"], "click");
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 1);
}

#[test]
fn adapter_contract_resolution_failures_stop_before_dispatch() {
    let bounds = Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    for (mode, code) in [
        (ResolveMode::Stale, "STALE_REF"),
        (ResolveMode::Ambiguous, "AMBIGUOUS_TARGET"),
    ] {
        let adapter = ContractAdapter::new(mode, Some(bounds));

        let err = ref_action_contract::run_click_command(&adapter, entry(bounds)).unwrap_err();

        assert_eq!(err.code(), code);
        assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn adapter_contract_wait_element_uses_session_snapshot() {
    let bounds = Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    let adapter = ContractAdapter::new(ResolveMode::Ok, Some(bounds));
    let context =
        agent_desktop_core::context::CommandContext::new(Some("shared-agent".into()), None, false)
            .unwrap();

    let result =
        ref_action_contract::run_wait_element_command(&adapter, entry(bounds), &context).unwrap();

    assert_eq!(result["found"], true);
    assert_eq!(result["ref"], "@e1");
    assert_eq!(result["predicate"], "exists");
}
