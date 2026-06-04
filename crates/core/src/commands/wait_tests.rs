use super::*;
use crate::{
    adapter::{PlatformAdapter, WindowFilter},
    error::{AdapterError, ErrorCode},
    node::WindowInfo,
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

struct WindowErrorAdapter;

impl PlatformAdapter for WindowErrorAdapter {
    fn list_windows(&self, _filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        Err(AdapterError::permission_denied())
    }
}

fn wait_args() -> WaitArgs {
    WaitArgs {
        mode: WaitModeArgs {
            ms: None,
            element: None,
            window: None,
            text: None,
            menu: false,
            menu_closed: false,
            notification: false,
        },
        predicate: WaitPredicateArgs {
            snapshot_id: None,
            predicate: None,
            value: None,
            count: None,
        },
        timeout_ms: 1,
        app: None,
    }
}

#[test]
fn notification_wait_propagates_adapter_error() {
    let err = execute(
        WaitArgs {
            mode: WaitModeArgs {
                notification: true,
                ..wait_args().mode
            },
            ..wait_args()
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
            mode: WaitModeArgs {
                ms: Some(1),
                element: Some("@e1".into()),
                ..wait_args().mode
            },
            ..wait_args()
        },
        &NoopAdapter,
    )
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(err.suggestion().is_some());
}

#[test]
fn window_wait_propagates_permanent_adapter_error() {
    let err = execute(
        WaitArgs {
            mode: WaitModeArgs {
                window: Some("Document".into()),
                ..wait_args().mode
            },
            ..wait_args()
        },
        &WindowErrorAdapter,
    )
    .unwrap_err();

    assert_eq!(err.code(), "PERM_DENIED");
}

#[test]
fn text_wait_propagates_permanent_snapshot_error() {
    let err = execute(
        WaitArgs {
            mode: WaitModeArgs {
                text: Some("hello".into()),
                ..wait_args().mode
            },
            ..wait_args()
        },
        &WindowErrorAdapter,
    )
    .unwrap_err();

    assert_eq!(err.code(), "PERM_DENIED");
}

#[test]
fn notification_wait_allows_text_filter() {
    let result = validate_wait_mode(&WaitArgs {
        mode: WaitModeArgs {
            text: Some("done".into()),
            notification: true,
            ..wait_args().mode
        },
        ..wait_args()
    });

    assert!(result.is_ok());
}

#[test]
fn predicate_requires_element_mode() {
    let err = validate_wait_mode(&WaitArgs {
        predicate: WaitPredicateArgs {
            predicate: Some("enabled".into()),
            ..wait_args().predicate
        },
        ..wait_args()
    })
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
}
