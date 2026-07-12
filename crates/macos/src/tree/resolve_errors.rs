use agent_desktop_core::{AdapterError, ErrorCode, RefEntry};

pub(super) fn native_identifier_role_reuse(entry: &RefEntry) -> AdapterError {
    AdapterError::stale_ref(
        "Saved native identifier was found only on a different accessibility role",
    )
    .with_details(serde_json::json!({
        "kind": "native_identifier_role_reuse",
        "expected_role": entry.identity.role,
        "complete": true,
        "retryable": false,
    }))
}

pub(super) fn identity_unknown(entry: &RefEntry) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Strict resolution could not determine candidate identity from the live accessibility evidence",
    )
    .with_suggestion("Retry after the target application finishes updating its accessibility tree")
    .with_details(serde_json::json!({
        "kind": "resolution_identity_unknown",
        "role": entry.identity.role,
        "complete": false,
        "retryable": true,
    }))
}

pub(super) fn incomplete_traversal(phase: &str, depth: u8) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        format!(
            "Strict element resolution observed an incomplete accessibility tree during {phase}"
        ),
    )
    .with_suggestion("Retry after the target application finishes updating its accessibility tree")
    .with_details(serde_json::json!({
        "kind": "resolution_traversal_incomplete",
        "phase": phase,
        "depth": depth,
        "complete": false,
        "retryable": true,
    }))
}
