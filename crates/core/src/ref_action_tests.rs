use super::*;
use crate::{
    action::Action,
    action_result::ActionResult,
    adapter::{NativeHandle, SnapshotSurface},
};

struct ReleaseFailingAdapter;

impl PlatformAdapter for ReleaseFailingAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::new("click"))
    }

    fn release_handle(&self, _handle: &NativeHandle) -> Result<(), AdapterError> {
        Err(AdapterError::internal("release failed"))
    }
}

fn entry() -> RefEntry {
    RefEntry {
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
        source_surface: SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    }
}

#[test]
fn successful_action_survives_release_failure() {
    let result = execute_entry(
        &ReleaseFailingAdapter,
        &entry(),
        ActionRequest::headless(Action::Click),
    )
    .unwrap();

    assert_eq!(result.action, "click");
}
