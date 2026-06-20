use agent_desktop_core::{
    adapter::PlatformAdapter,
    commands::{
        dismiss_all_notifications, dismiss_notification, list_notifications, notification_action,
    },
    error::{AppError, ErrorCode},
};
use serde_json::Value;

use crate::cli::Commands;

pub(crate) fn dispatch_notification(
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
                index: notification_index(a.index)?,
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
                index: notification_index(a.index)?,
                action: a.action,
                expected_app: a.expected_app,
                expected_title: a.expected_title,
            },
            adapter,
        ),
        _ => Err(AppError::Adapter(
            agent_desktop_core::error::AdapterError::new(
                ErrorCode::InvalidArgs,
                "dispatch_notification received a non-notification command",
            ),
        )),
    }
}

fn notification_index(index: u64) -> Result<usize, AppError> {
    if index == 0 {
        return Err(AppError::invalid_input(
            "Notification index is 1-based and must be greater than zero",
        ));
    }
    usize::try_from(index).map_err(|_| AppError::invalid_input("Notification index is too large"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_args::notifications::{DismissNotificationCliArgs, NotificationActionCliArgs};

    struct NoopAdapter;

    impl PlatformAdapter for NoopAdapter {}

    #[test]
    fn dismiss_notification_rejects_zero_index_before_adapter() {
        let err = dispatch_notification(
            Commands::DismissNotification(DismissNotificationCliArgs {
                index: 0,
                app: None,
            }),
            &NoopAdapter,
        )
        .unwrap_err();

        assert_eq!(err.code(), "INVALID_ARGS");
    }

    #[test]
    fn notification_action_rejects_zero_index_before_adapter() {
        let err = dispatch_notification(
            Commands::NotificationAction(NotificationActionCliArgs {
                index: 0,
                action: "Reply".into(),
                expected_app: None,
                expected_title: None,
            }),
            &NoopAdapter,
        )
        .unwrap_err();

        assert_eq!(err.code(), "INVALID_ARGS");
    }
}
