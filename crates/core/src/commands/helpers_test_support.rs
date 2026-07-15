use crate::refs::RefEntry;

pub(super) fn entry() -> RefEntry {
    let bounds = crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    RefEntry {
        process: crate::RefProcess {
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            role: "button".into(),
            name: Some("OK".into()),
            value: None,
            description: None,
            native_id: None,
        },
        geometry: crate::RefGeometry {
            bounds: Some(bounds),
            bounds_hash: bounds.bounds_hash(),
        },
        capabilities: crate::RefCapabilities {
            states: vec![],
            available_actions: vec!["Clear".into(), "Click".into()],
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

pub(super) fn text_entry() -> RefEntry {
    let mut entry = entry();
    entry.identity.role = "textfield".into();
    entry.capabilities.available_actions = vec!["SetValue".into()];
    entry
}

pub(in crate::commands) fn save_one_ref_snapshot(role: &str, available_action: &str) -> String {
    let mut entry = entry();
    entry.identity.role = role.into();
    entry.identity.name = Some("Target".into());
    entry.capabilities.available_actions = vec![available_action.into()];
    let mut refmap = crate::refs::RefMap::new();
    refmap.allocate(entry);
    crate::refs_store::RefStore::new()
        .unwrap()
        .save_new_snapshot(&refmap)
        .unwrap()
}
