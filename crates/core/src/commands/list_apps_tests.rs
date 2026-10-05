use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{AdapterError, AppInfo};

struct AppsAdapter;

struct DeadlineAdapter(u64);

impl ObservationOps for AppsAdapter {
    fn list_apps(&self, _deadline: crate::Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![
            AppInfo {
                name: "Finder".into(),
                pid: crate::ProcessId::new(1),
                bundle_id: Some("com.apple.finder".into()),
                process_instance: Some("test-instance".into()),
                presentation: None,
            },
            AppInfo {
                name: "TextEdit".into(),
                pid: crate::ProcessId::new(2),
                bundle_id: Some("com.apple.TextEdit".into()),
                process_instance: Some("test-instance".into()),
                presentation: None,
            },
        ])
    }
}

impl ObservationOps for DeadlineAdapter {
    fn list_apps(&self, deadline: crate::Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        assert_eq!(deadline.timeout_ms(), self.0);
        Ok(Vec::new())
    }
    fn list_windows(
        &self,
        _: &crate::WindowFilter,
        deadline: crate::Deadline,
    ) -> Result<Vec<crate::WindowInfo>, AdapterError> {
        assert_eq!(deadline.timeout_ms(), self.0);
        Ok(Vec::new())
    }
}
impl ActionOps for DeadlineAdapter {}
impl InputOps for DeadlineAdapter {}
impl SystemOps for DeadlineAdapter {}

impl ActionOps for AppsAdapter {}

impl InputOps for AppsAdapter {}

impl SystemOps for AppsAdapter {}

#[test]
fn app_filter_matches_by_name_case_insensitively() {
    let value = execute(
        ListAppsArgs {
            app: Some("text".into()),
            timeout_ms: None,
        },
        &AppsAdapter,
    )
    .unwrap();

    let apps = value["apps"].as_array().unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0]["name"], "TextEdit");
}

#[test]
fn listing_timeout_reaches_both_adapter_deadlines() {
    for timeout_ms in [None, Some(37), Some(8000)] {
        let adapter = DeadlineAdapter(timeout_ms.unwrap_or(crate::DEFAULT_OPERATION_TIMEOUT_MS));
        execute(
            ListAppsArgs {
                app: None,
                timeout_ms,
            },
            &adapter,
        )
        .unwrap();
        crate::commands::list_windows::execute(
            crate::commands::list_windows::ListWindowsArgs {
                app: None,
                timeout_ms,
            },
            &adapter,
        )
        .unwrap();
    }
}

struct IncompleteAdapter;
impl ObservationOps for IncompleteAdapter {
    fn list_apps_inventory(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        Ok(crate::AppInventory {
            apps: AppsAdapter.list_apps(deadline)?,
            skipped: vec![json!({"pid":11,"field":"application_name"})],
        })
    }
}
impl ActionOps for IncompleteAdapter {}
impl InputOps for IncompleteAdapter {}
impl SystemOps for IncompleteAdapter {}

#[test]
fn list_apps_marks_skipped_records_even_when_filtered_to_no_matches() {
    for app in [None, Some("Missing".into())] {
        let value = execute(
            ListAppsArgs {
                app,
                timeout_ms: None,
            },
            &IncompleteAdapter,
        )
        .unwrap();
        assert_eq!(value["complete"], false);
        assert_eq!(
            value["skipped"],
            json!([{"pid":11,"field":"application_name"}])
        );
    }
    let value = execute(
        ListAppsArgs {
            app: None,
            timeout_ms: None,
        },
        &AppsAdapter,
    )
    .unwrap();
    assert!(value.get("complete").is_none());
    assert!(value.get("skipped").is_none());
}
