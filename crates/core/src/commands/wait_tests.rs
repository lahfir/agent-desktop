use super::test_support::wait_args;
use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    AdapterError, ErrorCode, NotificationFilter, NotificationInfo, WindowInfo,
    adapter::WindowFilter,
};

struct NoopAdapter;

impl ObservationOps for NoopAdapter {}

impl ActionOps for NoopAdapter {}

impl InputOps for NoopAdapter {}

impl SystemOps for NoopAdapter {}

struct NotificationErrorAdapter;

impl ObservationOps for NotificationErrorAdapter {}

impl ActionOps for NotificationErrorAdapter {}

impl InputOps for NotificationErrorAdapter {}

impl SystemOps for NotificationErrorAdapter {
    fn list_notifications(
        &self,
        _filter: &NotificationFilter,
        _deadline: crate::Deadline,
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

impl ObservationOps for FlakyNotificationAdapter {}

impl ActionOps for FlakyNotificationAdapter {}

impl InputOps for FlakyNotificationAdapter {}

impl SystemOps for FlakyNotificationAdapter {
    fn list_notifications(
        &self,
        _filter: &NotificationFilter,
        _deadline: crate::Deadline,
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

impl ObservationOps for WindowErrorAdapter {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Err(AdapterError::permission_denied())
    }
}

impl ActionOps for WindowErrorAdapter {}

impl InputOps for WindowErrorAdapter {}

impl SystemOps for WindowErrorAdapter {}

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
fn expired_notification_wait_does_not_start_an_adapter_read() {
    let adapter = FlakyNotificationAdapter::with_responses(vec![Ok(Vec::new())]);

    let error = execute(
        notification_wait_args(0),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(error.code(), "TIMEOUT");
    assert_eq!(adapter.responses.lock().unwrap().len(), 1);
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
