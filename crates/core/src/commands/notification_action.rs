use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::{json, Value};

pub struct NotificationActionArgs {
    pub index: usize,
    pub action: String,
}

pub fn execute(
    args: NotificationActionArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let result = adapter.notification_action(args.index, &args.action)?;
    Ok(json!({
        "action": result.action,
    }))
}
