use crate::{adapter::PlatformAdapter, error::AppError, notification::NotificationFilter};
use serde_json::{json, Value};

pub struct DismissAllNotificationsArgs {
    pub app: Option<String>,
}

pub fn execute(
    args: DismissAllNotificationsArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let filter = NotificationFilter {
        app: args.app.clone(),
        ..Default::default()
    };
    let notifications = adapter.list_notifications(&filter)?;
    let mut dismissed = 0;
    for notif in notifications.iter().rev() {
        if adapter
            .dismiss_notification(notif.index, args.app.as_deref())
            .is_ok()
        {
            dismissed += 1;
        }
    }
    Ok(json!({
        "dismissed_count": dismissed,
    }))
}
