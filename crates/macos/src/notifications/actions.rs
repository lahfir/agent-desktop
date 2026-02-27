use agent_desktop_core::{
    action::ActionResult, error::AdapterError, notification::NotificationInfo,
};

pub fn dismiss_notification(
    _index: usize,
    _app_filter: Option<&str>,
) -> Result<NotificationInfo, AdapterError> {
    Err(AdapterError::not_supported(
        "dismiss_notification (not yet implemented)",
    ))
}

pub fn notification_action(
    _index: usize,
    _action_name: &str,
) -> Result<ActionResult, AdapterError> {
    Err(AdapterError::not_supported(
        "notification_action (not yet implemented)",
    ))
}
