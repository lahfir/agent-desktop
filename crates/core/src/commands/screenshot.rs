use crate::{
    adapter::{PlatformAdapter, ScreenshotTarget, WindowFilter},
    error::AppError,
};
use base64::Engine;
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct ScreenshotArgs {
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub output_path: Option<PathBuf>,
}

pub fn execute(args: ScreenshotArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let target = resolve_target(&args, adapter)?;
    let buf = adapter.screenshot(target)?;

    if let Some(path) = args.output_path {
        std::fs::write(&path, &buf.data)?;
        Ok(json!({ "path": path.to_string_lossy() }))
    } else {
        let encoded = base64::engine::general_purpose::STANDARD.encode(&buf.data);
        Ok(json!({
            "data": encoded,
            "format": buf.format.as_str(),
            "width": buf.width,
            "height": buf.height
        }))
    }
}

fn resolve_target(
    args: &ScreenshotArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<ScreenshotTarget, AppError> {
    if let Some(window_id) = &args.window_id {
        let filter = WindowFilter {
            focused_only: false,
            app: args.app.clone(),
        };
        let windows = adapter.list_windows(&filter)?;
        let win = windows
            .into_iter()
            .find(|w| &w.id == window_id)
            .ok_or_else(|| AppError::invalid_input(format!("Window '{window_id}' not found")))?;
        return Ok(ScreenshotTarget::Window(win.pid));
    }

    if let Some(app_name) = &args.app {
        let filter = WindowFilter {
            focused_only: false,
            app: Some(app_name.clone()),
        };
        let windows = adapter.list_windows(&filter)?;
        let win = windows.into_iter().next().ok_or_else(|| {
            AppError::invalid_input(format!("No windows found for app '{app_name}'"))
        })?;
        return Ok(ScreenshotTarget::Window(win.pid));
    }

    Ok(ScreenshotTarget::FullScreen)
}
