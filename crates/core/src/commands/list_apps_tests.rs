use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{error::AdapterError, node::AppInfo};

struct AppsAdapter;

impl ObservationOps for AppsAdapter {
    fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![
            AppInfo {
                name: "Finder".into(),
                pid: 1,
                bundle_id: Some("com.apple.finder".into()),
            },
            AppInfo {
                name: "TextEdit".into(),
                pid: 2,
                bundle_id: Some("com.apple.TextEdit".into()),
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
