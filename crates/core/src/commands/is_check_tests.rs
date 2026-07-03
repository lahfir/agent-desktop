use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    adapter::NativeHandle, error::AdapterError, node::Rect, refs::RefMap, refs_store::RefStore,
    refs_test_support::HomeGuard, state,
};
use std::sync::Mutex;

struct LiveStateAdapter {
    state: Mutex<Option<ElementState>>,
    bounds: Mutex<Option<Rect>>,
    bounds_supported: bool,
    state_supported: bool,
}

impl LiveStateAdapter {
    fn with_live(bounds: Rect, states: Vec<String>) -> Self {
        Self {
            state: Mutex::new(Some(ElementState {
                role: "button".into(),
                states,
                value: None,
            })),
            bounds: Mutex::new(Some(bounds)),
            bounds_supported: true,
            state_supported: true,
        }
    }

    fn without_live_support() -> Self {
        Self {
            state: Mutex::new(None),
            bounds: Mutex::new(None),
            bounds_supported: false,
            state_supported: false,
        }
    }
}

impl ObservationOps for LiveStateAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        if !self.state_supported {
            return Err(AdapterError::not_supported("get_live_state"));
        }
        Ok(self.state.lock().unwrap().clone())
    }

    fn get_element_bounds(&self, _handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> {
        if !self.bounds_supported {
            return Err(AdapterError::not_supported("get_element_bounds"));
        }
        Ok(*self.bounds.lock().unwrap())
    }
}

impl ActionOps for LiveStateAdapter {}

impl InputOps for LiveStateAdapter {}

impl SystemOps for LiveStateAdapter {}

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

fn visible_bounds() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    }
}

#[test]
fn hidden_element_reports_not_visible() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry(vec![], None, vec![]));
    let adapter = LiveStateAdapter::with_live(visible_bounds(), vec![state::HIDDEN.into()]);

    let result = execute(
        IsArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: IsProperty::Visible,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(result["result"], false);
    assert_eq!(result["applicable"], true);
}

#[test]
fn zero_sized_bounds_report_not_visible() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry(vec![], None, vec![]));
    let adapter = LiveStateAdapter::with_live(
        Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 10.0,
        },
        vec![],
    );

    let result = execute(
        IsArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: IsProperty::Visible,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(result["result"], false);
    assert_eq!(result["applicable"], true);
}

#[test]
fn offscreen_element_reports_not_visible() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry(vec![], None, vec![]));
    let adapter = LiveStateAdapter::with_live(visible_bounds(), vec![state::OFFSCREEN.into()]);

    let result = execute(
        IsArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: IsProperty::Visible,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(result["result"], false);
    assert_eq!(result["applicable"], true);
}

#[test]
fn visible_element_with_live_evidence_reports_true() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry(vec![], None, vec![]));
    let adapter = LiveStateAdapter::with_live(visible_bounds(), vec![]);

    let result = execute(
        IsArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: IsProperty::Visible,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(result["result"], true);
    assert_eq!(result["applicable"], true);
}

#[test]
fn visible_degrades_applicability_when_live_reads_unsupported() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry(vec![], None, vec![]));
    let adapter = LiveStateAdapter::without_live_support();

    let result = execute(
        IsArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: IsProperty::Visible,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(result["result"], false);
    assert_eq!(result["applicable"], false);
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
        bounds: Mutex::new(None),
        bounds_supported: false,
        state_supported: true,
    };

    let result = execute(
        IsArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: IsProperty::Checked,
        },
        &adapter,
        &CommandContext::default(),
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
        bounds: Mutex::new(None),
        bounds_supported: false,
        state_supported: true,
    };

    let result = execute(
        IsArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: IsProperty::Checked,
        },
        &adapter,
        &CommandContext::default(),
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
        bounds: Mutex::new(None),
        bounds_supported: false,
        state_supported: true,
    };

    let result = execute(
        IsArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: IsProperty::Checked,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(result["result"], true);
    assert_eq!(result["applicable"], true);
}

#[test]
fn basic_state_properties_use_live_state() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry(vec![], None, vec![]));
    let adapter =
        LiveStateAdapter::with_live(visible_bounds(), vec!["focused".into(), "expanded".into()]);

    for (property, expected) in [
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
            &CommandContext::default(),
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
        bounds: Mutex::new(None),
        bounds_supported: false,
        state_supported: true,
    };

    for property in [IsProperty::Checked, IsProperty::Expanded] {
        let result = execute(
            IsArgs {
                ref_id: "@e1".into(),
                snapshot_id: Some(snapshot_id.clone()),
                property,
            },
            &adapter,
            &CommandContext::default(),
        )
        .unwrap();

        assert_eq!(result["applicable"], true);
    }
}

#[test]
fn state_vocabulary_conformance_guard() {
    for token in [
        state::FOCUSED,
        state::DISABLED,
        state::CHECKED,
        state::EXPANDED,
        state::HIDDEN,
        state::OFFSCREEN,
    ] {
        state::assert_states_in_vocabulary(&[token.to_string()]);
    }
}
