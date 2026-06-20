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

struct FlakyNotificationAdapter {
    responses: std::sync::Mutex<Vec<Result<Vec<NotificationInfo>, AdapterError>>>,
}

impl FlakyNotificationAdapter {
    fn with_responses(in_order: Vec<Result<Vec<NotificationInfo>, AdapterError>>) -> Self {
        let mut responses = in_order;
        responses.reverse();
        Self {
            responses: std::sync::Mutex::new(responses),
        }
    }
}

impl PlatformAdapter for FlakyNotificationAdapter {
    fn list_notifications(
        &self,
        _filter: &NotificationFilter,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        self.responses
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| Err(AdapterError::timeout("notification center unavailable")))
    }
}

fn notification(index: usize, title: &str) -> NotificationInfo {
    NotificationInfo {
        index,
        app_name: "Mail".into(),
        title: title.into(),
        body: None,
        actions: vec![],
    }
}

fn notification_wait_args(timeout_ms: u64) -> WaitArgs {
    WaitArgs {
        mode: WaitModeArgs {
            notification: true,
            ..wait_args().mode
        },
        timeout_ms,
        ..wait_args()
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
            action: None,
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
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "PLATFORM_NOT_SUPPORTED");
}

#[test]
fn notification_wait_retries_transient_baseline_errors() {
    let adapter = FlakyNotificationAdapter::with_responses(vec![
        Err(AdapterError::timeout("notification center starting")),
        Ok(vec![notification(0, "old")]),
        Ok(vec![notification(0, "old"), notification(1, "fresh")]),
    ]);

    let value = execute(
        notification_wait_args(5_000),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["matched"], true);
    assert_eq!(value["notification"]["title"], "fresh");
}

#[test]
fn notification_wait_fingerprint_ignores_reindexed_existing_notification() {
    let baseline = notification_counts(&[notification(0, "old")]);
    let current = vec![notification(4, "old")];

    assert!(first_new_notification(&current, &baseline).is_none());
}

#[test]
fn notification_wait_fingerprint_detects_duplicate_new_notification() {
    let baseline = notification_counts(&[notification(0, "same")]);
    let current = vec![notification(4, "same"), notification(5, "same")];

    let found = first_new_notification(&current, &baseline).unwrap();

    assert_eq!(found.index, 5);
}

#[test]
fn notification_wait_times_out_with_last_error_after_transient_failures() {
    let adapter = FlakyNotificationAdapter::with_responses(vec![]);

    let err = execute(
        notification_wait_args(600),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    let AppError::Adapter(adapter_err) = err else {
        panic!("expected adapter error");
    };
    assert_eq!(adapter_err.code, ErrorCode::Timeout);
    let details = adapter_err.details.expect("timeout should carry details");
    assert_eq!(details["last_error"]["code"], "TIMEOUT");
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
        &CommandContext::default(),
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
        &CommandContext::default(),
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
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "PERM_DENIED");
}

#[test]
fn app_retryability_uses_adapter_error_codes() {
    assert!(is_retryable_wait_app_error(&AppError::Adapter(
        AdapterError::timeout("busy")
    )));
    assert!(!is_retryable_wait_app_error(&AppError::Adapter(
        AdapterError::permission_denied()
    )));
    assert!(!is_retryable_wait_app_error(&AppError::Internal(
        "internal".into()
    )));
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

struct TextlessTreeAdapter;

impl PlatformAdapter for TextlessTreeAdapter {
    fn list_windows(&self, _filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: "TestApp".into(),
            pid: 1,
            bounds: None,
            is_focused: true,
        }])
    }

    fn get_tree(
        &self,
        _win: &WindowInfo,
        _opts: &crate::adapter::TreeOptions,
    ) -> Result<crate::node::AccessibilityNode, AdapterError> {
        Ok(crate::node::AccessibilityNode {
            ref_id: None,
            role: "window".into(),
            name: Some("Doc".into()),
            value: None,
            description: None,
            hint: None,
            states: vec![],
            available_actions: vec![],
            bounds: None,
            children_count: None,
            children: vec![],
        })
    }
}

#[test]
fn text_wait_with_count_zero_detects_absence() {
    let _guard = crate::refs_test_support::HomeGuard::new();

    let value = execute(
        WaitArgs {
            mode: WaitModeArgs {
                text: Some("Gone".into()),
                ..wait_args().mode
            },
            predicate: WaitPredicateArgs {
                count: Some(0),
                ..wait_args().predicate
            },
            timeout_ms: 1_000,
            app: Some("TestApp".into()),
        },
        &TextlessTreeAdapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["found"], true);
    assert_eq!(value["count"], 0);
}

struct MenuWaitAdapter {
    open_seen: std::sync::Mutex<Option<bool>>,
}

impl PlatformAdapter for MenuWaitAdapter {
    fn list_apps(&self) -> Result<Vec<crate::node::AppInfo>, AdapterError> {
        Ok(vec![crate::node::AppInfo {
            name: "MenuApp".into(),
            pid: 42,
            bundle_id: None,
        }])
    }

    fn wait_for_menu(&self, _pid: i32, open: bool, _timeout_ms: u64) -> Result<(), AdapterError> {
        *self.open_seen.lock().unwrap() = Some(open);
        Ok(())
    }
}

#[test]
fn menu_closed_wait_requests_closed_state_and_reports_found() {
    let adapter = MenuWaitAdapter {
        open_seen: std::sync::Mutex::new(None),
    };
    let value = execute(
        WaitArgs {
            mode: WaitModeArgs {
                menu_closed: true,
                ..wait_args().mode
            },
            app: Some("MenuApp".into()),
            ..wait_args()
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["found"], true);
    assert_eq!(
        *adapter.open_seen.lock().unwrap(),
        Some(false),
        "--menu-closed must wait for the menu to be closed (open=false)"
    );
}
