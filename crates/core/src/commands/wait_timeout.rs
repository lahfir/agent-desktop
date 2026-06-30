use crate::{
    commands::wait_predicate,
    error::{AdapterError, AppError},
};
use serde_json::{Value, json};

/// Builds the wait-loop TIMEOUT error. Every payload carries
/// `kind: "wait_timeout"` so agents can discriminate it from the chain
/// deadline TIMEOUT schema (`kind: "chain_deadline"`) without sniffing
/// field names.
fn timeout_err(message: String, mut details: Value) -> Result<Value, AppError> {
    details["kind"] = json!("wait_timeout");
    Err(AppError::Adapter(
        AdapterError::timeout(message).with_details(details),
    ))
}

pub(crate) fn element(
    ref_id: String,
    predicate: wait_predicate::ElementPredicate,
    timeout_ms: u64,
    last_observed: Value,
) -> Result<Value, AppError> {
    timeout_err(
        format!(
            "Element {ref_id} did not satisfy predicate '{}' within {timeout_ms}ms; last_observed={last_observed}",
            predicate.name()
        ),
        json!({
            "ref": ref_id,
            "predicate": predicate.name(),
            "timeout_ms": timeout_ms,
            "last_observed": last_observed
        }),
    )
}

pub(crate) fn window(
    title: &str,
    timeout_ms: u64,
    last_error: Option<Value>,
) -> Result<Value, AppError> {
    timeout_err(
        format!("Window with title '{title}' not found within {timeout_ms}ms"),
        json!({
            "predicate": "window",
            "title": title,
            "timeout_ms": timeout_ms,
            "last_error": last_error
        }),
    )
}

pub(crate) fn text(
    text: &str,
    timeout_ms: u64,
    expected_count: Option<usize>,
    last_error: Option<Value>,
) -> Result<Value, AppError> {
    timeout_err(
        format!("Text '{text}' did not match within {timeout_ms}ms"),
        json!({
            "predicate": "text",
            "text_chars": text.chars().count(),
            "timeout_ms": timeout_ms,
            "expected_count": expected_count,
            "last_error": last_error
        }),
    )
}

pub(crate) fn notification(
    app: Option<&String>,
    text: Option<&String>,
    timeout_ms: u64,
    last_error: Option<Value>,
) -> Result<Value, AppError> {
    timeout_err(
        format!("No new notification within {timeout_ms}ms"),
        json!({
            "predicate": "notification",
            "timeout_ms": timeout_ms,
            "app": app,
            "text_chars": text.map(|text| text.chars().count()),
            "last_error": last_error
        }),
    )
}

pub(crate) fn selector(
    selector: &str,
    gone: bool,
    timeout_ms: u64,
    last_error: Option<Value>,
    last_snapshot_id: Option<String>,
) -> Result<Value, AppError> {
    let mut details = json!({
        "predicate": "selector",
        "selector": selector,
        "gone": gone,
        "timeout_ms": timeout_ms,
    });
    if let Some(err) = last_error {
        details["last_error"] = err;
    }
    if let Some(snapshot_id) = last_snapshot_id {
        details["snapshot_id"] = json!(snapshot_id);
    }
    timeout_err(
        format!(
            "Selector '{selector}' did not {} within {timeout_ms}ms",
            if gone { "disappear" } else { "appear" }
        ),
        details,
    )
}
