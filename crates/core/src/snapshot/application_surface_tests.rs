use crate::{
    AppInfo, CommandContext, Deadline, ErrorCode, NativeHandle, ObservationRoot, RefEntry,
    SnapshotOwner, SnapshotSurface, TreeOptions, WindowFilter, WindowInfo,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps, minimal_accessibility_node},
    commands::{find, wait_selector},
    refs::refs_test_support::HomeGuard,
};

struct MenuAdapter {
    apps: Vec<AppInfo>,
    windows: Vec<WindowInfo>,
}

impl MenuAdapter {
    fn windowless() -> Self {
        Self {
            apps: vec![app(42)],
            windows: vec![],
        }
    }
}

impl ObservationOps for MenuAdapter {
    fn is_application_surface(&self, surface: SnapshotSurface) -> bool {
        matches!(surface, SnapshotSurface::Menubar | SnapshotSurface::Menu)
    }

    fn focused_app(&self, _deadline: Deadline) -> Result<Option<AppInfo>, crate::AdapterError> {
        Ok(self.apps.first().cloned())
    }

    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, crate::AdapterError> {
        Ok(self.apps.clone())
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, crate::AdapterError> {
        Ok(self.windows.clone())
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &crate::ObservationRequest,
    ) -> Result<crate::ObservedTree, crate::AdapterError> {
        assert_eq!(root.surface(), request.surface);
        let mut item = minimal_accessibility_node("menuitem", "Help");
        item.presentation.available_actions = vec!["AXPress".into()];
        item.presentation.bounds = Some(crate::Rect {
            x: 300.0,
            y: 10.0,
            width: 40.0,
            height: 20.0,
        });
        crate::adapter::observed_tree(&root, item)
    }

    fn resolve_element_strict(
        &self,
        entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, crate::AdapterError> {
        if !self.apps.iter().any(|app| {
            app.pid == entry.process.pid && app.process_instance == entry.process.process_instance
        }) {
            return Err(crate::AdapterError::stale_ref_because("process changed"));
        }
        Ok(NativeHandle::null())
    }
}

impl ActionOps for MenuAdapter {}
impl InputOps for MenuAdapter {}
impl SystemOps for MenuAdapter {
    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        vec![SnapshotSurface::Menubar, SnapshotSurface::Menu]
    }
}

fn app(pid: u32) -> AppInfo {
    AppInfo {
        name: "MenuApp".into(),
        pid: crate::ProcessId::new(pid),
        bundle_id: Some("org.example.menu".into()),
        process_instance: Some(format!("instance-{pid}")),
        presentation: None,
    }
}

fn options(surface: SnapshotSurface) -> TreeOptions {
    TreeOptions {
        surface,
        ..Default::default()
    }
}

fn assert_application_ref(entry: &RefEntry, surface: SnapshotSurface) {
    assert_eq!(entry.process.pid, crate::ProcessId::new(42));
    assert_eq!(
        entry.process.process_instance.as_deref(),
        Some("instance-42")
    );
    assert_eq!(entry.source.source_app.as_deref(), Some("MenuApp"));
    assert_eq!(entry.source.source_surface, surface);
    assert!(entry.source.source_window_id.is_none());
    assert!(entry.source.source_window_title.is_none());
    assert!(entry.source.source_window_bounds_hash.is_none());
    assert_ne!(
        entry.geometry.bounds_space,
        Some(crate::BoundsSpace::Window)
    );
}

#[test]
fn application_surface_snapshot_and_drill_do_not_require_a_document_window() {
    let _guard = HomeGuard::new();
    let adapter = MenuAdapter::windowless();
    let context = CommandContext::default();
    for surface in [SnapshotSurface::Menubar, SnapshotSurface::Menu] {
        for app_name in [Some("MenuApp"), None] {
            let snapshot = super::run_with_context(
                &adapter,
                &options(surface),
                &super::SnapshotTarget {
                    app_name,
                    window_id: None,
                },
                &context,
                1_000,
            )
            .unwrap();
            assert!(matches!(snapshot.owner, SnapshotOwner::Application(_)));
            let data =
                crate::commands::snapshot::format_snapshot_fields(&snapshot, None, None).unwrap();
            assert_eq!(data["app"], "MenuApp");
            assert!(data.get("window").is_none());
            assert_application_ref(snapshot.refmap.get("@e1").unwrap(), surface);
            let root = snapshot.tree.ref_id.as_deref().unwrap();
            let drilled = super::snapshot_ref::run_from_ref_with_context(
                &adapter,
                &TreeOptions::default(),
                root,
                &context,
                1_000,
            )
            .unwrap();
            assert!(drilled.owner.window().is_none());
            let local =
                crate::refs::ref_token::resolve_ref_target(drilled.tree.ref_id.as_deref().unwrap())
                    .unwrap()
                    .1;
            assert_application_ref(drilled.refmap.get(&local).unwrap(), surface);
        }
    }
}

#[test]
fn find_materializes_refs_from_the_application_surface() {
    let _guard = HomeGuard::new();
    let adapter = MenuAdapter::windowless();
    let context = CommandContext::default();
    let result = find::execute(
        find::FindArgs {
            app: Some("MenuApp".into()),
            window_id: None,
            root: None,
            surface: SnapshotSurface::Menubar,
            filter: find::FindFilterArgs {
                role: Some("menuitem".into()),
                name: Some("Help".into()),
                description: None,
                native_id: None,
                value: None,
                text: None,
                exact: true,
            },
            selection: find::FindSelectionArgs {
                count: false,
                first: true,
                last: false,
                nth: None,
                limit: None,
            },
            states: vec![],
            timeout_ms: Some(1_000),
        },
        &adapter,
        &context,
    )
    .unwrap();
    let entry = crate::commands::helpers::load_ref_entry(
        result["match"]["ref_id"].as_str().unwrap(),
        &context,
    )
    .unwrap();
    assert_application_ref(&entry, SnapshotSurface::Menubar);
}

#[test]
fn a_missing_document_window_does_not_satisfy_a_menu_disappearance_wait() {
    let _guard = HomeGuard::new();
    let adapter = MenuAdapter::windowless();
    for gone in [false, true] {
        let result = wait_selector::execute(
            wait_selector::WaitSelectorInput {
                query_raw: "menuitem:Help".into(),
                gone,
                app: Some("MenuApp".into()),
                window_id: None,
                opts: options(SnapshotSurface::Menubar),
                timeout_ms: 30,
            },
            &adapter,
            &CommandContext::default(),
        );
        if gone {
            assert_eq!(result.unwrap_err().code(), "TIMEOUT");
        } else {
            let data = result.unwrap();
            assert_eq!(data["matched_selector"], "menuitem:Help");
            assert!(data.get("window").is_none());
        }
    }
}

#[test]
fn an_explicit_window_selects_its_exact_application_without_becoming_the_menu_owner() {
    let mut adapter = MenuAdapter::windowless();
    adapter.apps.push(app(99));
    let duplicate = super::build(
        &adapter,
        &options(SnapshotSurface::Menubar),
        Some("MenuApp"),
        None,
        Deadline::standard().unwrap(),
    );
    assert_eq!(duplicate.err().unwrap().code(), "AMBIGUOUS_TARGET");
    adapter.windows.push(WindowInfo {
        id: "w-99".into(),
        title: "Document".into(),
        app: "MenuApp".into(),
        pid: crate::ProcessId::new(99),
        process_instance: Some("instance-99".into()),
        bounds: None,
        state: Default::default(),
    });
    let snapshot = super::build(
        &adapter,
        &options(SnapshotSurface::Menubar),
        Some("MenuApp"),
        Some("w-99"),
        Deadline::standard().unwrap(),
    )
    .unwrap();
    let SnapshotOwner::Application(owner) = snapshot.owner else {
        panic!("menu must be application-owned")
    };
    assert_eq!(owner.pid, crate::ProcessId::new(99));
    assert!(
        snapshot
            .refmap
            .get("@e1")
            .unwrap()
            .source
            .source_window_id
            .is_none()
    );
    let missing = super::build(
        &adapter,
        &options(SnapshotSurface::Menubar),
        Some("MenuApp"),
        Some("w-missing"),
        Deadline::standard().unwrap(),
    );
    assert_eq!(missing.err().unwrap().code(), "WINDOW_NOT_FOUND");
}

#[test]
fn drilling_an_application_ref_rejects_a_restarted_process_without_writing_refs() {
    let _guard = HomeGuard::new();
    let mut adapter = MenuAdapter::windowless();
    let snapshot = super::run_with_context(
        &adapter,
        &options(SnapshotSurface::Menubar),
        &super::SnapshotTarget {
            app_name: Some("MenuApp"),
            window_id: None,
        },
        &CommandContext::default(),
        1_000,
    )
    .unwrap();
    adapter.apps[0].process_instance = Some("replacement".into());
    let error = super::snapshot_ref::run_from_ref_with_context(
        &adapter,
        &TreeOptions::default(),
        snapshot.tree.ref_id.as_deref().unwrap(),
        &CommandContext::default(),
        1_000,
    )
    .err()
    .unwrap();
    assert_eq!(error.code(), ErrorCode::StaleRef.as_str());
    let saved = crate::RefStore::new()
        .unwrap()
        .load_snapshot(snapshot.snapshot_id.as_deref().unwrap())
        .unwrap();
    assert_eq!(saved.len(), snapshot.refmap.len());
}
