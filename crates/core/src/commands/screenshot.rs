use crate::{
    adapter::{PlatformAdapter, ScreenshotTarget},
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
    let target = match (&args.window_id, &args.app) {
        (Some(id), _) => ScreenshotTarget::Window(id.clone()),
        (None, Some(_app)) => ScreenshotTarget::FullScreen,
        (None, None) => ScreenshotTarget::FullScreen,
    };

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
