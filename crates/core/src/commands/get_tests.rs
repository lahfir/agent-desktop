use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    AdapterError, Rect, adapter::NativeHandle, refs::RefEntry, refs::RefMap, refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::Mutex;

struct LiveBoundsAdapter {
    bounds: Mutex<Option<Rect>>,
    bounds_supported: bool,
}

impl LiveBoundsAdapter {
    fn with_live(bounds: Rect) -> Self {
        Self {
            bounds: Mutex::new(Some(bounds)),
            bounds_supported: true,
        }
    }

    fn without_live_support() -> Self {
        Self {
            bounds: Mutex::new(None),
            bounds_supported: false,
        }
    }

    fn resolvable_but_boundless() -> Self {
        Self {
            bounds: Mutex::new(None),
            bounds_supported: true,
        }
    }
}

impl ObservationOps for LiveBoundsAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        if !self.bounds_supported {
            return Err(AdapterError::not_supported("get_element_bounds"));
        }
        Ok(*self.bounds.lock().unwrap())
    }
}

impl ActionOps for LiveBoundsAdapter {}

impl InputOps for LiveBoundsAdapter {}

impl SystemOps for LiveBoundsAdapter {}

fn save_entry(entry: RefEntry) -> String {
    let mut refmap = RefMap::new();
    refmap.allocate(entry);
    RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap()
}

fn stale_snapshot_bounds() -> Rect {
    Rect {
        x: 84.0,
        y: -322.0,
        width: 200.0,
        height: 20.0,
    }
}

fn live_bounds() -> Rect {
    Rect {
        x: 84.0,
        y: 289.0,
        width: 200.0,
        height: 20.0,
    }
}

fn entry_with_bounds(bounds: Option<Rect>) -> RefEntry {
    RefEntry {
        process: crate::RefProcess {
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            role: "textfield".into(),
            name: Some("text-input".into()),
            value: None,
            description: None,
            native_id: None,
        },
        geometry: crate::RefGeometry {
            bounds,
            bounds_hash: bounds.and_then(|b| b.bounds_hash()),
        },
        capabilities: crate::RefCapabilities {
            states: vec![],
            available_actions: vec![],
        },
        source: crate::RefSource {
            source_app: None,
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: crate::adapter::SnapshotSurface::Window,
        },
        scope: crate::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::SmallVec::new(),
        },
    }
}

/// A ref's stored geometry is a snapshot-time fact: an ancestor scroll
/// between snapshot and `get` moves the element without ever touching that
/// ref's refmap entry. `get --property bounds` exists to answer "where is
/// this element right now", so it must prefer a live read the same way
/// `text`/`value` already do (get.rs) — never silently serve the
/// snapshot-time position as if it were current.
#[test]
fn bounds_prefers_a_live_read_over_the_stale_snapshot_geometry() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry_with_bounds(Some(stale_snapshot_bounds())));
    let adapter = LiveBoundsAdapter::with_live(live_bounds());

    let result = execute(
        GetArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: GetProperty::Bounds,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(result["value"]["y"], 289.0);
    assert_ne!(result["value"]["y"], -322.0);
    assert_eq!(result["live"], true);
}

/// When the platform cannot answer a live bounds read, the stored geometry
/// is the only evidence available and remains the fallback.
#[test]
fn bounds_falls_back_to_snapshot_geometry_when_no_live_read_is_available() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry_with_bounds(Some(stale_snapshot_bounds())));
    let adapter = LiveBoundsAdapter::without_live_support();

    let result = execute(
        GetArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: GetProperty::Bounds,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(result["value"]["y"], -322.0);
    assert_eq!(result["live"], false);
}

/// A live read can succeed and still report no bounds (collapsed, not laid
/// out, virtualized). The response must not be indistinguishable from a
/// live-verified rectangle: a caller piping this into `mouse-click --x --y`
/// needs to know the rectangle it received is the snapshot-time fallback,
/// not the element's current position.
#[test]
fn bounds_marks_snapshot_fallback_as_not_live_when_a_successful_live_read_finds_no_bounds() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry(entry_with_bounds(Some(stale_snapshot_bounds())));
    let adapter = LiveBoundsAdapter::resolvable_but_boundless();

    let result = execute(
        GetArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            property: GetProperty::Bounds,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(result["value"]["y"], -322.0);
    assert_eq!(result["live"], false);
}
