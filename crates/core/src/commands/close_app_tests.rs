use super::*;
use crate::adapter::PlatformAdapter;
use crate::error::{AdapterError, ErrorCode};

struct ProtectiveAdapter;

impl PlatformAdapter for ProtectiveAdapter {
    fn is_protected_process(&self, identifier: &str) -> bool {
        identifier.eq_ignore_ascii_case("CriticalThing")
    }

    fn close_app(&self, _id: &str, _force: bool) -> Result<(), crate::error::AdapterError> {
        Ok(())
    }
}

struct FailingAdapter;

impl PlatformAdapter for FailingAdapter {
    fn close_app(&self, _id: &str, _force: bool) -> Result<(), AdapterError> {
        Err(AdapterError::new(ErrorCode::AppNotFound, "no such app"))
    }
}

#[test]
fn close_app_blocks_adapter_protected_process() {
    let err = execute(
        CloseAppArgs {
            app: "CriticalThing".into(),
            force: false,
        },
        &ProtectiveAdapter,
    )
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(err.to_string().contains("protected"));
}

#[test]
fn graceful_close_reports_request_without_claiming_termination() {
    let value = execute(
        CloseAppArgs {
            app: "TextEdit".into(),
            force: false,
        },
        &ProtectiveAdapter,
    )
    .unwrap();

    assert_eq!(value["app"], "TextEdit");
    assert_eq!(value["method"], "graceful");
    assert_eq!(value["requested"], true);
    assert_eq!(
        value["closed"], false,
        "graceful close must not claim a termination it cannot confirm"
    );
}

#[test]
fn close_app_propagates_adapter_errors() {
    let err = execute(
        CloseAppArgs {
            app: "Ghost".into(),
            force: true,
        },
        &FailingAdapter,
    )
    .unwrap_err();

    assert_eq!(err.code(), "APP_NOT_FOUND");
}

#[test]
fn forced_close_confirms_termination() {
    let value = execute(
        CloseAppArgs {
            app: "TextEdit".into(),
            force: true,
        },
        &ProtectiveAdapter,
    )
    .unwrap();

    assert_eq!(value["app"], "TextEdit");
    assert_eq!(value["method"], "force");
    assert_eq!(value["requested"], true);
    assert_eq!(value["closed"], true);
}
