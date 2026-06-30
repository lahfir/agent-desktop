use crate::{
    action::Action,
    adapter::{PlatformAdapter, SnapshotSurface, TreeOptions},
    commands::helpers::{
        RefArgs, apply_post_action_wait, execute_ref_action_result_with_context, probe_app_name,
    },
    context::CommandContext,
    error::AppError,
    snapshot,
};
use serde_json::{Value, json};

pub fn execute(
    args: RefArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let request = context.request_base(Action::RightClick);
    let (entry, result) = execute_ref_action_result_with_context(
        &args.ref_id,
        args.snapshot_id.as_deref(),
        adapter,
        request,
        context,
    )?;
    let mut response = serde_json::to_value(&result)?;

    std::thread::sleep(std::time::Duration::from_millis(200));

    let opts = TreeOptions {
        interactive_only: true,
        surface: SnapshotSurface::Menu,
        ..Default::default()
    };
    let probe_app = probe_app_name(adapter, &entry);
    match snapshot::run_with_context(adapter, &opts, probe_app.as_deref(), None, context) {
        Ok(snap) => match serde_json::to_value(&snap.tree) {
            Ok(menu_json) => {
                response["menu"] = menu_json;
                if let Some(snapshot_id) = snap.snapshot_id {
                    response["menu_snapshot_id"] = json!(snapshot_id);
                }
            }
            Err(err) => {
                response["menu_probe"] = json!({
                    "ok": false,
                    "error": {
                        "code": "INTERNAL",
                        "message": err.to_string(),
                    }
                })
            }
        },
        Err(err) => response["menu_probe"] = probe_error_json(&err),
    }

    apply_post_action_wait(response, probe_app.as_deref(), adapter, context)
}

fn probe_error_json(err: &AppError) -> Value {
    if err.code() == "ELEMENT_NOT_FOUND" {
        return json!({
            "ok": false,
            "error": {
                "code": "ELEMENT_NOT_FOUND",
                "message": "Right-click action was accepted, but no menu accessibility tree was exposed for capture.",
                "suggestion": "Use 'snapshot --surface menu' only when the app exposes the context menu through accessibility."
            }
        });
    }

    let mut error = json!({
        "code": err.code(),
        "message": err.to_string(),
    });
    if let Some(suggestion) = err.suggestion() {
        error["suggestion"] = json!(suggestion);
    }
    json!({
        "ok": false,
        "error": error,
    })
}

#[cfg(test)]
#[path = "right_click_tests.rs"]
mod tests;
