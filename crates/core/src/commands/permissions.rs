use crate::{AppError, PermissionReport, adapter::PlatformAdapter};
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
        let lease = crate::commands::helpers::acquire_interaction_lease(adapter)?;
        adapter.request_permissions(&lease)?
    } else {
        report.clone()
    };
    Ok(json!({
        "accessibility": report.accessibility,
        "screen_recording": report.screen_recording,
        "automation": report.automation
    }))
}
