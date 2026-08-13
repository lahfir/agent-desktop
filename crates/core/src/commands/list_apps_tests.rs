use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{AdapterError, AppInfo};

struct AppsAdapter;

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

impl ActionOps for AppsAdapter {}

impl InputOps for AppsAdapter {}

impl SystemOps for AppsAdapter {}

#[test]
fn app_filter_matches_by_name_case_insensitively() {
    let value = execute(
        ListAppsArgs {
            app: Some("text".into()),
        },
        &AppsAdapter,
    )
    .unwrap();

    let apps = value["apps"].as_array().unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0]["name"], "TextEdit");
}
