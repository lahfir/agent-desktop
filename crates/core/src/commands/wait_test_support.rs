use super::{ElementWaitInput, wait_for_element, wait_predicate};
use crate::{adapter::PlatformAdapter, context::CommandContext, error::AppError};
use serde_json::Value;

pub(super) fn wait_for_element_test(
    ref_id: String,
    snapshot_id: Option<String>,
    predicate: wait_predicate::ElementPredicate,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    wait_for_element(
        ElementWaitInput {
            ref_id,
            snapshot_id,
            predicate,
            timeout_ms,
        },
        adapter,
        context,
    )
}
