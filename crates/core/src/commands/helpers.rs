use crate::{
    action::WindowOp,
    action_request::ActionRequest,
    action_result::ActionResult,
    adapter::{PlatformAdapter, TreeOptions, WindowFilter},
    commands::{wait_selector, wait_selector::WaitSelectorInput},
    context::CommandContext,
    error::AppError,
    node::WindowInfo,
    refs::{RefEntry, validate_ref_id},
    refs_store::RefStore,
    resolved_element::ResolvedElement,
    window_lookup,
};
use serde_json::{Value, json};

pub struct AppArgs {
    pub app: Option<String>,
}

pub struct RefArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
}

pub(crate) fn resolve_ref_with_context<'a>(
    ref_id: &str,
    snapshot_id: Option<&str>,
    adapter: &'a dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<(RefEntry, ResolvedElement<'a>), AppError> {
    validate_ref_id(ref_id)?;
    let store = RefStore::for_session(context.session_id())?;
    context.trace_lazy(
        "ref.resolve.start",
        || json!({ "ref": ref_id, "snapshot_id": snapshot_id }),
    )?;
    let refmap = store.load(snapshot_id).inspect_err(|e| {
        tracing::debug!("refmap load failed: {e}");
        let _ = context.trace_lazy("ref.resolve.error", || {
            json!({
                "ref": ref_id,
                "snapshot_id": snapshot_id,
                "code": e.code(),
                "message": e.to_string()
            })
        });
    })?;
    let entry = match refmap.get(ref_id) {
        Some(entry) => entry.clone(),
        None => {
            context.trace_lazy("ref.resolve.error", || {
                json!({
                    "ref": ref_id,
                    "snapshot_id": snapshot_id,
                    "code": "STALE_REF",
                    "message": "ref not found in current RefMap"
                })
            })?;
            return Err(AppError::stale_ref(ref_id));
        }
    };
    tracing::debug!(
        "resolve: {} -> pid={} role={} name_chars={:?}",
        ref_id,
        entry.pid,
        entry.role,
        entry.name.as_deref().map(|name| name.chars().count())
    );
    context.trace_lazy("ref.resolve.entry", || {
        json!({
            "ref": ref_id,
            "pid": entry.pid,
            "role": entry.role,
            "name": entry.name
        })
    })?;
    let handle = adapter.resolve_element_strict(&entry).inspect_err(|err| {
        let _ = context.trace_lazy("ref.resolve.error", || {
            json!({
                "ref": ref_id,
                "snapshot_id": snapshot_id,
                "code": err.code.as_str(),
                "message": err.message.clone(),
                "details": err.details.clone()
            })
        });
    })?;
    tracing::debug!("resolve: {} resolved successfully", ref_id);
    context.trace_lazy("ref.resolve.ok", || json!({ "ref": ref_id }))?;
    Ok((entry, ResolvedElement::new(adapter, handle)))
}

pub(crate) fn resolve_app_pid(
    app: Option<&str>,
    adapter: &dyn PlatformAdapter,
) -> Result<i32, AppError> {
    if let Some(name) = app {
        let apps = adapter.list_apps()?;
        apps.into_iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .map(|a| a.pid)
            .ok_or_else(|| AppError::invalid_input(format!("App '{name}' not found")))
    } else {
        let filter = WindowFilter {
            focused_only: true,
            app: None,
        };
        let windows = adapter.list_windows(&filter)?;
        windows
            .first()
            .map(|w| w.pid)
            .ok_or_else(|| AppError::invalid_input("No focused window. Use --app to specify."))
    }
}

pub(crate) fn execute_ref_action_with_context(
    args: RefArgs,
    adapter: &dyn PlatformAdapter,
    request: ActionRequest,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let (entry, result) = execute_ref_action_result_with_context(
        &args.ref_id,
        args.snapshot_id.as_deref(),
        adapter,
        request,
        context,
    )?;
    apply_post_action_wait(serde_json::to_value(result)?, &entry, adapter, context)
}

/// Resolves the app name a ref belongs to for post-action polling. Normal
/// refmaps always carry `source_app`; the pid lookup is a fallback for legacy
/// or partially-populated entries so the wait never silently polls the focused
/// window instead of the acted-on app.
pub(crate) fn probe_app_name(adapter: &dyn PlatformAdapter, entry: &RefEntry) -> Option<String> {
    if entry.source_app.is_some() {
        return entry.source_app.clone();
    }
    window_lookup::find_window_for_pid(entry.pid, adapter)
        .ok()
        .map(|window| window.app)
}

pub(crate) fn apply_post_action_wait(
    result: Value,
    entry: &RefEntry,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let Some(wait) = context.wait_selector() else {
        return Ok(result);
    };
    match wait_selector::execute(
        WaitSelectorInput {
            query_raw: wait.query_raw.clone(),
            gone: wait.gone,
            app: probe_app_name(adapter, entry),
            window_id: entry.source_window_id.clone(),
            opts: TreeOptions::default(),
            timeout_ms: wait.timeout_ms,
        },
        adapter,
        context,
    ) {
        Ok(mut snapshot) => {
            if let Some(body) = snapshot.as_object_mut() {
                body.insert("after_action".into(), result);
            }
            Ok(snapshot)
        }
        Err(AppError::Adapter(mut adapter_err)) => {
            let mut details = adapter_err.details.take().unwrap_or_else(|| json!({}));
            if let Some(obj) = details.as_object_mut() {
                obj.insert("after_action".into(), result);
            }
            Err(AppError::Adapter(adapter_err.with_details(details)))
        }
        Err(err) => Err(err),
    }
}

pub(crate) fn execute_ref_action_result_with_context(
    ref_id: &str,
    snapshot_id: Option<&str>,
    adapter: &dyn PlatformAdapter,
    request: ActionRequest,
    context: &CommandContext,
) -> Result<(RefEntry, ActionResult), AppError> {
    let (entry, handle) = resolve_ref_with_context(ref_id, snapshot_id, adapter, context)?;
    let result = crate::ref_action::execute_resolved(
        crate::ref_action::ResolvedRefAction {
            adapter,
            entry: &entry,
            handle: handle.handle(),
            ref_id,
            context,
        },
        request,
    )?;
    Ok((entry, result))
}

pub(crate) fn window_op_command(
    args: AppArgs,
    adapter: &dyn PlatformAdapter,
    op: WindowOp,
    response_key: &'static str,
) -> Result<Value, AppError> {
    let pid = resolve_app_pid(args.app.as_deref(), adapter)?;
    let win = match window_lookup::find_window_for_pid(pid, adapter) {
        Ok(win) => win,
        Err(_) if matches!(op, WindowOp::Restore) => WindowInfo {
            id: String::new(),
            title: String::new(),
            app: args.app.unwrap_or_default(),
            pid,
            bounds: None,
            is_focused: false,
        },
        Err(err) => return Err(err),
    };
    adapter.window_op(&win, op)?;
    Ok(json!({ response_key: true }))
}

pub(crate) fn resolve_window_for_app(
    app: Option<&str>,
    adapter: &dyn PlatformAdapter,
) -> Result<WindowInfo, AppError> {
    let pid = resolve_app_pid(app, adapter)?;
    window_lookup::find_window_for_pid(pid, adapter)
}

#[cfg(test)]
#[path = "helpers_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "helpers_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "helpers_ref_action_tests.rs"]
mod ref_action_tests;
