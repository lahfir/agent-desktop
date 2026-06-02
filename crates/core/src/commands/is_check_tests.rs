use super::*;
use crate::{
    adapter::NativeHandle, error::AdapterError, refs::RefMap, refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::Mutex;

struct LiveStateAdapter {
    state: Mutex<Option<ElementState>>,
}

impl PlatformAdapter for LiveStateAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        Ok(self.state.lock().unwrap().clone())
    }
}

fn save_entry(entry: RefEntry) -> String {
    let mut refmap = RefMap::new();
    refmap.allocate(entry);
    RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap()
}

fn entry(states: Vec<String>, value: Option<&str>, actions: Vec<&str>) -> RefEntry {
    RefEntry {
        pid: 1,
        role: "checkbox".into(),
        name: Some("Target".into()),
        value: value.map(str::to_string),
        description: None,
        states,
        bounds: None,
        bounds_hash: None,
        available_actions: actions.into_iter().map(str::to_string).collect(),
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
fn checked_uses_live_canonical_state() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry(vec![], None, vec!["Toggle"]));
    let adapter = LiveStateAdapter {
        state: Mutex::new(Some(ElementState {
            role: "checkbox".into(),
            states: vec!["checked".into()],
            value: Some("1".into()),
        })),
    };

    let result = execute(
        IsArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: IsProperty::Checked,
        },
        &adapter,
    )
    .unwrap();

    assert_eq!(result["result"], true);
    assert_eq!(result["applicable"], true);
}

#[test]
fn checked_does_not_infer_platform_values_in_core() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry(vec![], Some("1"), vec!["Toggle"]));
    let adapter = LiveStateAdapter {
        state: Mutex::new(None),
    };

    let result = execute(
        IsArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: IsProperty::Checked,
        },
        &adapter,
    )
    .unwrap();

    assert_eq!(result["result"], false);
    assert_eq!(result["applicable"], true);
}

#[test]
fn checked_falls_back_to_snapshot_state_when_live_state_is_missing() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry(vec!["checked".into()], None, vec!["Toggle"]));
    let adapter = LiveStateAdapter {
        state: Mutex::new(None),
    };

    let result = execute(
        IsArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: IsProperty::Checked,
        },
        &adapter,
    )
    .unwrap();

    assert_eq!(result["result"], true);
    assert_eq!(result["applicable"], true);
}

#[test]
fn basic_state_properties_use_live_state() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry(vec![], None, vec![]));
    let adapter = LiveStateAdapter {
        state: Mutex::new(Some(ElementState {
            role: "button".into(),
            states: vec!["focused".into(), "expanded".into()],
            value: None,
        })),
    };

    for (property, expected) in [
        (IsProperty::Visible, true),
        (IsProperty::Enabled, true),
        (IsProperty::Focused, true),
        (IsProperty::Expanded, true),
    ] {
        let result = execute(
            IsArgs {
                ref_id: "@e1".into(),
                snapshot_id: Some(snapshot_id.clone()),
                property,
            },
            &adapter,
        )
        .unwrap();

        assert_eq!(result["result"], expected);
        assert_eq!(result["applicable"], true);
    }
}

#[test]
fn action_availability_makes_toggle_and_expand_applicable() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(RefEntry {
        pid: 1,
        role: "cell".into(),
        name: Some("Disclosure".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec!["Check".into(), "Expand".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
    let adapter = LiveStateAdapter {
        state: Mutex::new(None),
    };

    for property in [IsProperty::Checked, IsProperty::Expanded] {
        let result = execute(
            IsArgs {
                ref_id: "@e1".into(),
                snapshot_id: Some(snapshot_id.clone()),
                property,
            },
            &adapter,
        )
        .unwrap();

        assert_eq!(result["applicable"], true);
    }
}
