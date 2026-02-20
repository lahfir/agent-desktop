use crate::{
    adapter::{PlatformAdapter, WindowFilter},
    error::AppError,
};
use serde_json::{json, Value};

pub struct FocusWindowArgs {
    pub window_id: Option<String>,
    pub app: Option<String>,
    pub title: Option<String>,
}

pub fn execute(args: FocusWindowArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let filter = WindowFilter {
        focused_only: false,
        app: args.app.clone(),
    };
    let windows = adapter.list_windows(&filter)?;

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
        AppError::Adapter(crate::error::AdapterError::new(
            crate::error::ErrorCode::WindowNotFound,
            "No matching window found",
        ))
    })?;

    adapter.focus_window(&window)?;
    Ok(json!({ "focused": window }))
}
