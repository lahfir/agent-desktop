use super::*;
use crate::{
    adapter::PlatformAdapter,
    error::{AdapterError, ErrorCode},
    notification::{NotificationFilter, NotificationInfo},
};

struct NoopAdapter;

impl PlatformAdapter for NoopAdapter {}

struct NotificationErrorAdapter;

impl PlatformAdapter for NotificationErrorAdapter {
    fn list_notifications(
        &self,
        _filter: &NotificationFilter,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        Err(AdapterError::new(
            ErrorCode::PlatformNotSupported,
            "notifications unavailable",
        ))
    }
}

#[test]
fn notification_wait_propagates_adapter_error() {
    let err = execute(
        WaitArgs {
            ms: None,
            element: None,
            snapshot_id: None,
            predicate: None,
            value: None,
            count: None,
            window: None,
            text: None,
            timeout_ms: 1,
            menu: false,
            menu_closed: false,
            notification: true,
            app: None,
        },
        &NotificationErrorAdapter,
    )
    .unwrap_err();

    assert_eq!(err.code(), "PLATFORM_NOT_SUPPORTED");
}

#[test]
fn rejects_multiple_wait_modes() {
    let err = execute(
        WaitArgs {
            ms: Some(1),
            element: Some("@e1".into()),
            snapshot_id: None,
            predicate: None,
            value: None,
            count: None,
            window: None,
            text: None,
            timeout_ms: 1,
            menu: false,
            menu_closed: false,
            notification: false,
            app: None,
        },
        &NoopAdapter,
    )
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(err.suggestion().is_some());
}

#[test]
fn notification_wait_allows_text_filter() {
    let result = validate_wait_mode(&WaitArgs {
        ms: None,
        element: None,
        snapshot_id: None,
        predicate: None,
        value: None,
        count: None,
        window: None,
        text: Some("done".into()),
        timeout_ms: 1,
        menu: false,
        menu_closed: false,
        notification: true,
        app: None,
    });

    assert!(result.is_ok());
}

#[test]
fn predicate_requires_element_mode() {
    let err = validate_wait_mode(&WaitArgs {
        ms: None,
        element: None,
        snapshot_id: None,
        predicate: Some("enabled".into()),
        value: None,
        count: None,
        window: None,
        text: None,
        timeout_ms: 1,
        menu: false,
        menu_closed: false,
        notification: false,
        app: None,
    })
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
}
