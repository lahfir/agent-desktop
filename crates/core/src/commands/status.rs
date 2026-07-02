use crate::{
    PermissionReport,
    adapter::PlatformAdapter,
    commands::permissions::{self, PermissionsArgs},
    context::CommandContext,
    error::AppError,
    refs_store::RefStore,
    session::read_current_session_pointer,
};
use serde_json::{Value, json};

pub fn execute_with_report_with_context(
    adapter: &dyn PlatformAdapter,
    report: &PermissionReport,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let permissions =
        permissions::execute_with_report(PermissionsArgs { request: false }, adapter, report)?;

    let store = RefStore::for_session(context.session_id()).ok();
    let ref_count = store
        .as_ref()
        .and_then(|s| s.load_latest().ok())
        .map(|m| m.len());
    let snapshot_id = store.and_then(|s| s.latest_snapshot_id());
    let session_id = context
        .session_id()
        .map(str::to_string)
        .or_else(|| read_current_session_pointer().ok().flatten());
    let tracing = context.trace_enabled();
    let artifacts = session_id
        .as_deref()
        .and_then(|id| crate::session::read_manifest(id).ok().flatten())
        .map(|manifest| manifest.artifacts);

    let mut body = json!({
        "platform": std::env::consts::OS,
        "version": env!("CARGO_PKG_VERSION"),
        "permissions": permissions,
        "snapshot_id": snapshot_id,
        "ref_count": ref_count,
        "session_id": session_id,
        "tracing": tracing,
    });
    if let Some(artifacts) = artifacts {
        body["artifacts"] = json!(artifacts);
    }
    Ok(body)
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
