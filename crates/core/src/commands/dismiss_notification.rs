use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::{json, Value};

pub struct DismissNotificationArgs {
    pub index: usize,
    pub app: Option<String>,
}

pub fn execute(
    args: DismissNotificationArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let dismissed = adapter.dismiss_notification(args.index, args.app.as_deref())?;
    Ok(json!({
        "dismissed": dismissed,
    }))
}
