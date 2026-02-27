use agent_desktop_core::{
    error::AdapterError,
    notification::{NotificationFilter, NotificationInfo},
};

pub fn list_notifications(
    _filter: &NotificationFilter,
) -> Result<Vec<NotificationInfo>, AdapterError> {
    Err(AdapterError::not_supported(
        "list_notifications (not yet implemented)",
    ))
}
