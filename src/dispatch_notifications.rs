use agent_desktop_core::{
    adapter::PlatformAdapter,
    commands::{
        dismiss_all_notifications, dismiss_notification, list_notifications, notification_action,
    },
    error::AppError,
};
use serde_json::Value;

use crate::cli::Commands;

pub fn dispatch_notification(
    cmd: Commands,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    match cmd {
        Commands::ListNotifications(a) => list_notifications::execute(
            list_notifications::ListNotificationsArgs {
                app: a.app,
                text: a.text,
                limit: a.limit,
            },
            adapter,
        ),
        Commands::DismissNotification(a) => dismiss_notification::execute(
            dismiss_notification::DismissNotificationArgs {
                index: a.index as usize,
                app: a.app,
            },
            adapter,
        ),
        Commands::DismissAllNotifications(a) => dismiss_all_notifications::execute(
            dismiss_all_notifications::DismissAllNotificationsArgs { app: a.app },
            adapter,
        ),
        Commands::NotificationAction(a) => notification_action::execute(
            notification_action::NotificationActionArgs {
                index: a.index as usize,
                action: a.action,
            },
            adapter,
        ),
        _ => unreachable!(),
    }
}
