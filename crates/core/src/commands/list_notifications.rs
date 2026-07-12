use crate::{AppError, NotificationFilter, adapter::PlatformAdapter};
use serde_json::{Value, json};

pub struct ListNotificationsArgs {
    pub app: Option<String>,
    pub text: Option<String>,
    pub limit: Option<usize>,
}

pub fn execute(
    args: ListNotificationsArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let filter = NotificationFilter {
        app: args.app,
        text: args.text,
        limit: args.limit,
    };
    let notifications = adapter.list_notifications(&filter, crate::Deadline::standard()?)?;
    Ok(json!({
        "count": notifications.len(),
        "notifications": notifications,
    }))
}
