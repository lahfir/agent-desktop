use crate::{
    AppError,
    adapter::{PlatformAdapter, WindowFilter},
};
use serde_json::{Value, json};
pub struct FocusWindowArgs {
    pub window_id: Option<String>,
    pub app: Option<String>,
    pub title: Option<String>,
}

pub fn execute(args: FocusWindowArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let deadline = crate::Deadline::standard()?;
    let filter = WindowFilter {
        focused_only: false,
        app: args.app.clone(),
    };
    let windows = adapter.list_windows(&filter, deadline)?;

    let window = if let Some(id) = &args.window_id {
        windows.into_iter().find(|w| &w.id == id)
    } else if let Some(title) = &args.title {
        windows
            .into_iter()
            .find(|w| w.title.contains(title.as_str()))
    } else if let Some(app) = &args.app {
        windows
            .into_iter()
            .find(|w| w.app.eq_ignore_ascii_case(app))
    } else {
        return Err(AppError::invalid_input(
            "Provide --window-id, --app, or --title to identify the window",
        ));
    };

    let window = window.ok_or_else(|| {
        AppError::Adapter(
            crate::AdapterError::new(crate::ErrorCode::WindowNotFound, "No matching window found")
                .with_suggestion("Run 'list-windows' to see available windows and their IDs."),
        )
    })?;

    let window_id = window.id.clone();
    let lease = adapter.acquire_interaction_lease(deadline)?;
    let focused = crate::window_focus::focus_and_confirm(adapter, &window, &lease)?;
    debug_assert_eq!(focused.id, window_id);
    Ok(json!({ "focused": focused }))
}

#[cfg(test)]
#[path = "focus_window_tests.rs"]
mod tests;
