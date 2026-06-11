use crate::refs::RefEntry;

pub(super) fn entry() -> RefEntry {
    RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("OK".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec!["Clear".into(), "Click".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    }
}

pub(super) fn text_entry() -> RefEntry {
    let mut entry = entry();
    entry.role = "textfield".into();
    entry.available_actions = vec!["SetValue".into()];
    entry
}
