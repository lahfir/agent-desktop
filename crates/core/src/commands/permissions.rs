use crate::{PermissionReport, adapter::PlatformAdapter, error::AppError};
use serde_json::{Value, json};

pub struct PermissionsArgs {
    pub request: bool,
}

pub fn execute_with_report(
    args: PermissionsArgs,
    adapter: &dyn PlatformAdapter,
    report: &PermissionReport,
) -> Result<Value, AppError> {
    let report = if args.request {
        adapter.request_permissions()
    } else {
        report.clone()
    };
    Ok(json!({
        "accessibility": report.accessibility,
        "screen_recording": report.screen_recording,
        "automation": report.automation
    }))
}
