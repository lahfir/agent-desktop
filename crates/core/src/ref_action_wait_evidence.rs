use crate::{ActionRequest, AdapterError};

pub(crate) fn should_scroll_after_preflight(request: &ActionRequest, error: &AdapterError) -> bool {
    request.action.requires_scroll_into_view() && failed_check(error, "visible")
}

pub(crate) fn failed_check(error: &AdapterError, check_name: &str) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("checks"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|checks| {
            checks.iter().any(|check| {
                check.get("check").and_then(serde_json::Value::as_str) == Some(check_name)
                    && check.get("status").and_then(serde_json::Value::as_str) == Some("fail")
            })
        })
}

pub(crate) fn observed_bounds_hash(error: &AdapterError) -> Option<u64> {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("observed_bounds_hash"))
        .and_then(serde_json::Value::as_u64)
}
