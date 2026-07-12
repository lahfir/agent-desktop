use crate::{AppError, adapter::PlatformAdapter, search_text};
use serde_json::{Value, json};

pub struct ListAppsArgs {
    pub app: Option<String>,
}

pub fn execute(args: ListAppsArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let mut apps = adapter.list_apps(crate::Deadline::standard()?)?;
    if let Some(app) = args.app {
        let needle = search_text::normalize(&app);
        apps.retain(|candidate| search_text::contains(&candidate.name, &needle));
    }
    Ok(json!({ "apps": apps }))
}

#[cfg(test)]
#[path = "list_apps_tests.rs"]
mod tests;
