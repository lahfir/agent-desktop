use crate::commands::wait_element::{ElementWaitInput, wait_for_element};
use crate::commands::wait_predicate;
use crate::{
    adapter::{NativeHandle, PlatformAdapter},
    context::CommandContext,
    element_state::ElementState,
    error::{AdapterError, AppError},
    node::Rect,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
};
use serde_json::Value;

pub(super) fn wait_for_element_test(
    ref_id: String,
    snapshot_id: Option<String>,
    predicate: wait_predicate::ElementPredicate,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    wait_for_element(
        ElementWaitInput {
            ref_id,
            snapshot_id,
            predicate,
            timeout_ms,
        },
        adapter,
        context,
    )
}

pub(super) struct PredicateAdapter {
    pub(super) state: Option<ElementState>,
    pub(super) value: Option<String>,
    pub(super) bounds: Option<Rect>,
}

impl PlatformAdapter for PredicateAdapter {
    fn resolve_element_strict_with_timeout(
        &self,
        _entry: &RefEntry,
        _timeout: std::time::Duration,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        Ok(self.state.clone())
    }

    fn get_live_value(&self, _handle: &NativeHandle) -> Result<Option<String>, AdapterError> {
        Ok(self.value.clone())
    }

    fn get_element_bounds(&self, _handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> {
        Ok(self.bounds)
    }
}

pub(super) fn snapshot_with_one_ref() -> String {
    save_ref(Vec::new())
}

pub(super) fn snapshot_with_disabled_ref() -> String {
    save_ref(vec!["disabled".into()])
}

pub(super) fn save_ref(states: Vec<String>) -> String {
    save_ref_in_store(RefStore::new().unwrap(), states)
}

pub(super) fn save_ref_in_session(session_id: &str, states: Vec<String>) -> String {
    save_ref_in_store(RefStore::for_session(Some(session_id)).unwrap(), states)
}

fn save_ref_in_store(store: RefStore, states: Vec<String>) -> String {
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("Run".into()),
        value: None,
        description: None,
        states,
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
    store.save_new_snapshot(&refmap).unwrap()
}
