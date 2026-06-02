use crate::{
    PermissionReport,
    adapter::PlatformAdapter,
    commands::permissions::{self, PermissionsArgs},
    context::CommandContext,
    error::AppError,
    refs_store::RefStore,
};
use serde_json::{Value, json};

pub fn execute(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let report = adapter.permission_report();
    execute_with_report(adapter, &report)
}

pub fn execute_with_report(
    adapter: &dyn PlatformAdapter,
    report: &PermissionReport,
) -> Result<Value, AppError> {
    execute_with_report_with_context(adapter, report, &CommandContext::default())
}

pub fn execute_with_report_with_context(
    adapter: &dyn PlatformAdapter,
    report: &PermissionReport,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let permissions =
        permissions::execute_with_report(PermissionsArgs { request: false }, adapter, report)?;

    let store = RefStore::for_session(context.session_id.as_deref()).ok();
    let ref_count = store
        .as_ref()
        .and_then(|s| s.load_latest().ok())
        .map(|m| m.len());
    let snapshot_id = store.and_then(|s| s.latest_snapshot_id());

    Ok(json!({
        "platform": std::env::consts::OS,
        "version": env!("CARGO_PKG_VERSION"),
        "permissions": permissions,
        "snapshot_id": snapshot_id,
        "ref_count": ref_count
    }))
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
