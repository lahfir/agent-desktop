use crate::{AppError, adapter::PlatformAdapter, search_text};
use serde_json::{Value, json};

pub struct ListAppsArgs {
    pub app: Option<String>,
    pub timeout_ms: Option<u64>,
}

pub fn execute(args: ListAppsArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let inventory = adapter.list_apps_inventory(crate::Deadline::after(
        args.timeout_ms
            .unwrap_or(crate::DEFAULT_OPERATION_TIMEOUT_MS),
    )?)?;
    let mut apps = inventory.apps;
    if let Some(app) = args.app {
        let needle = search_text::normalize(&app);
        apps.retain(|candidate| search_text::contains(&candidate.name, &needle));
    }
    let mut output = json!({ "apps": apps });
    if !inventory.skipped.is_empty() {
        output["complete"] = json!(false);
        output["skipped"] = json!(inventory.skipped);
    }
    Ok(output)
}

#[cfg(test)]
#[path = "list_apps_tests.rs"]
mod tests;
