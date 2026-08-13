use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

struct InventoryAdapter {
    apps: Vec<AppInfo>,
    windows: Vec<crate::WindowInfo>,
}

impl ObservationOps for InventoryAdapter {
    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(self.apps.clone())
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<crate::WindowInfo>, AdapterError> {
        Ok(self.windows.clone())
    }
}

impl ActionOps for InventoryAdapter {}
impl InputOps for InventoryAdapter {}
impl SystemOps for InventoryAdapter {}

fn app(instance: Option<&str>) -> AppInfo {
    AppInfo {
        name: "Example".into(),
        pid: crate::ProcessId::new(42),
        bundle_id: Some("com.example.app".into()),
        process_instance: instance.map(str::to_string),
        presentation: None,
    }
}

fn app_with_pid(pid: u32, instance: Option<&str>) -> AppInfo {
    AppInfo {
        pid: crate::ProcessId::new(pid),
        ..app(instance)
    }
}

struct ScopedInventorySpy {
    apps: Vec<AppInfo>,
    global_calls: AtomicUsize,
    scopes: Mutex<Vec<(String, Option<String>)>>,
}

impl ScopedInventorySpy {
    fn new(apps: Vec<AppInfo>) -> Self {
        Self {
            apps,
            global_calls: AtomicUsize::new(0),
            scopes: Mutex::new(Vec::new()),
        }
    }
}

impl ObservationOps for ScopedInventorySpy {
    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        self.global_calls.fetch_add(1, Ordering::Relaxed);
        Err(AdapterError::new(
            ErrorCode::AppUnresponsive,
            "unrelated global inventory failure",
        ))
    }

    fn list_apps_scoped(
        &self,
        name: &str,
        bundle_id: Option<&str>,
        _deadline: Deadline,
    ) -> Result<Vec<AppInfo>, AdapterError> {
        self.scopes
            .lock()
            .unwrap()
            .push((name.to_string(), bundle_id.map(str::to_string)));
        Ok(self.apps.clone())
    }
}

impl ActionOps for ScopedInventorySpy {}
impl InputOps for ScopedInventorySpy {}
impl SystemOps for ScopedInventorySpy {}

struct ScopedPermissionAdapter;

impl ObservationOps for ScopedPermissionAdapter {
    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Err(AdapterError::new(
            ErrorCode::AppUnresponsive,
            "unrelated global inventory failure",
        ))
    }

    fn list_apps_scoped(
        &self,
        _name: &str,
        _bundle_id: Option<&str>,
        _deadline: Deadline,
    ) -> Result<Vec<AppInfo>, AdapterError> {
        Err(AdapterError::new(
            ErrorCode::PermDenied,
            "permission denied reading requested process identity",
        ))
    }
}

impl ActionOps for ScopedPermissionAdapter {}
impl InputOps for ScopedPermissionAdapter {}
impl SystemOps for ScopedPermissionAdapter {}

#[test]
fn named_resolution_uses_scoped_inventory_when_global_inventory_fails() {
    let adapter = ScopedInventorySpy::new(vec![app(Some("generation-a"))]);

    let resolved = resolve_app(Some("example"), &adapter, Deadline::standard().unwrap()).unwrap();

    assert_eq!(resolved.pid, 42);
    assert_eq!(adapter.global_calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        *adapter.scopes.lock().unwrap(),
        vec![("example".to_string(), None)]
    );
}

#[test]
fn mutation_revalidation_uses_name_and_bundle_scope() {
    let expected = app(Some("generation-a"));
    let mut differently_cased = expected.clone();
    differently_cased.name = "example".into();
    differently_cased.bundle_id = Some("COM.EXAMPLE.APP".into());
    let adapter = ScopedInventorySpy::new(vec![differently_cased]);

    let resolved =
        revalidate_app_for_mutation(&adapter, &expected, Deadline::standard().unwrap()).unwrap();

    assert_eq!(resolved.pid, 42);
    assert_eq!(adapter.global_calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        *adapter.scopes.lock().unwrap(),
        vec![("Example".to_string(), Some("com.example.app".to_string()))]
    );
}

#[test]
fn scoped_target_permission_failure_is_preserved() {
    let error = resolve_app(
        Some("Example"),
        &ScopedPermissionAdapter,
        Deadline::standard().unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.code(), "PERM_DENIED");
}

#[test]
fn default_scoped_inventory_filters_exact_name_and_bundle_case_insensitively() {
    let mut wrong_name = app_with_pid(43, Some("generation-b"));
    wrong_name.name = "Example Helper".into();
    let mut wrong_bundle = app_with_pid(44, Some("generation-c"));
    wrong_bundle.bundle_id = Some("com.example.other".into());
    let adapter = InventoryAdapter {
        apps: vec![app(Some("generation-a")), wrong_name, wrong_bundle],
        windows: Vec::new(),
    };

    let scoped = adapter
        .list_apps_scoped(
            "example",
            Some("COM.EXAMPLE.APP"),
            Deadline::standard().unwrap(),
        )
        .unwrap();

    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].pid, 42);
}

#[test]
fn named_resolution_rejects_any_matching_candidate_without_generation() {
    let adapter = InventoryAdapter {
        apps: vec![app(Some("generation-a")), app(None)],
        windows: Vec::new(),
    };

    let error = resolve_app(Some("Example"), &adapter, Deadline::standard().unwrap()).unwrap_err();

    assert_eq!(error.code(), "ACTION_NOT_SUPPORTED");
}

#[test]
fn named_resolution_keeps_same_name_multiple_pids_ambiguous() {
    let adapter = InventoryAdapter {
        apps: vec![
            app_with_pid(42, Some("generation-a")),
            app_with_pid(43, Some("generation-b")),
        ],
        windows: Vec::new(),
    };

    let error = resolve_app(Some("Example"), &adapter, Deadline::standard().unwrap()).unwrap_err();

    assert_eq!(error.code(), "AMBIGUOUS_TARGET");
}

#[test]
fn omitted_app_with_no_focused_window_is_not_found() {
    let adapter = InventoryAdapter {
        apps: vec![app(Some("generation-a"))],
        windows: Vec::new(),
    };

    let error = resolve_app(None, &adapter, Deadline::standard().unwrap()).unwrap_err();

    assert_eq!(error.code(), "APP_NOT_FOUND");
}

#[test]
fn mutation_revalidation_rejects_pid_reuse() {
    let expected = app(Some("generation-a"));
    let adapter = InventoryAdapter {
        apps: vec![app(Some("generation-b"))],
        windows: Vec::new(),
    };

    let error = revalidate_app_for_mutation(&adapter, &expected, Deadline::standard().unwrap())
        .unwrap_err();

    assert_eq!(error.code(), "STALE_REF");
}

#[test]
fn mutation_revalidation_rejects_missing_generation() {
    let expected = app(Some("generation-a"));
    let adapter = InventoryAdapter {
        apps: vec![app(None)],
        windows: Vec::new(),
    };

    let error = revalidate_app_for_mutation(&adapter, &expected, Deadline::standard().unwrap())
        .unwrap_err();

    assert_eq!(error.code(), "ACTION_NOT_SUPPORTED");
}
