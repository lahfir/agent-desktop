use crate::{
    AdapterError, ErrorCode, adapter::PlatformAdapter, context::CommandContext, refs::RefEntry,
};
use serde_json::json;

pub(crate) fn enrich_with_process_state(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    err: AdapterError,
    deadline: crate::Deadline,
) -> AdapterError {
    if !matches!(err.code, ErrorCode::StaleRef | ErrorCode::AppNotFound) {
        return err;
    }
    let Some(instance) = entry
        .process
        .process_instance
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return err;
    };
    let Ok(state) = adapter.process_state(
        crate::ProcessIdentity::new(entry.process.pid, instance),
        deadline,
    ) else {
        return err;
    };
    if state == crate::process_state::ProcessState::Unresponsive {
        if !process_identity_matches(adapter, entry, deadline) {
            return err;
        }
        let app = entry
            .source
            .source_app
            .as_deref()
            .unwrap_or("target application");
        let unresponsive = AdapterError::app_unresponsive(app);
        let mut details = match err.details {
            Some(serde_json::Value::Object(details)) => details,
            Some(cause) => serde_json::Map::from_iter([("cause".into(), cause)]),
            None => serde_json::Map::new(),
        };
        details.insert("kind".into(), json!("app_unresponsive"));
        details.insert("retryable".into(), json!(false));
        return unresponsive.with_details(details.into());
    }
    attach_process_state_detail(err, state)
}

fn process_identity_matches(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: crate::Deadline,
) -> bool {
    let Some(expected_name) = entry
        .source
        .source_app
        .as_deref()
        .filter(|name| !name.is_empty())
    else {
        return false;
    };
    let Some(expected_instance) = entry
        .process
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
    else {
        return false;
    };
    let Ok(apps) = adapter.list_apps(deadline) else {
        return false;
    };
    let mut same_pid = apps.iter().filter(|app| app.pid == entry.process.pid);
    let Some(app) = same_pid.next() else {
        return false;
    };
    same_pid.next().is_none()
        && app.name == expected_name
        && app.process_instance.as_deref() == Some(expected_instance)
}

fn attach_process_state_detail(
    err: AdapterError,
    state: crate::process_state::ProcessState,
) -> AdapterError {
    let mut details = err.details.clone().unwrap_or_else(|| json!({}));
    match details.as_object_mut() {
        Some(obj) => {
            obj.insert("process_state".into(), json!(state.label()));
        }
        None => details = json!({ "process_state": state.label() }),
    }
    err.with_details(details)
}

pub(crate) fn trace_resolve_error(context: &CommandContext, ref_id: &str, err: &AdapterError) {
    let _ = context.trace_lazy("ref.resolve.error", || {
        json!({
            "ref": ref_id,
            "code": err.code.as_str(),
            "message": err.message.clone(),
            "details": err.details.clone()
        })
    });
}

pub(crate) fn trace_resolve_ok(context: &CommandContext, ref_id: &str) {
    let _ = context.trace_lazy("ref.resolve.ok", || json!({ "ref": ref_id }));
}
