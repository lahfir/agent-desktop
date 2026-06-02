use crate::{commands::wait_predicate, error::AppError};
use serde_json::{Value, json};

pub(crate) fn element(
    ref_id: String,
    predicate: wait_predicate::ElementPredicate,
    timeout_ms: u64,
    last_observed: Value,
) -> Result<Value, AppError> {
    Err(AppError::Adapter(crate::error::AdapterError::timeout(
        format!(
            "Element {ref_id} did not satisfy predicate '{}' within {timeout_ms}ms; last_observed={last_observed}",
            predicate.name()
        ),
    )
    .with_details(json!({
        "ref": ref_id,
        "predicate": predicate.name(),
        "timeout_ms": timeout_ms,
        "last_observed": last_observed
    }))))
}
