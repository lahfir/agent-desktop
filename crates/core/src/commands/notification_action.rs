use crate::{adapter::PlatformAdapter, error::AppError, notification::NotificationIdentity};
use serde_json::{json, Value};

pub struct NotificationActionArgs {
    pub index: usize,
    pub action: String,
    pub expected_app: Option<String>,
    pub expected_title: Option<String>,
}

pub fn execute(
    args: NotificationActionArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let identity = if args.expected_app.is_some() || args.expected_title.is_some() {
        Some(NotificationIdentity {
            expected_app: args.expected_app,
            expected_title: args.expected_title,
        })
    } else {
        None
    };
    let result = adapter.notification_action(args.index, identity.as_ref(), &args.action)?;
    Ok(json!({
        "action": result.action,
    }))
}
