use crate::{
    adapter::{PlatformAdapter, TreeOptions},
    commands::{query, snapshot as snapshot_cmd, wait_timeout},
    context::CommandContext,
    error::{AppError, ErrorCode},
    refs_store::RefStore,
    snapshot::{self, emit_snapshot_saved},
    trace_artifacts,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub struct WaitSelectorInput {
    pub query_raw: String,
    pub gone: bool,
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub opts: TreeOptions,
    pub timeout_ms: u64,
}

pub fn execute(
    input: WaitSelectorInput,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let query = query::validate_selector(&input.query_raw)?;

    let start = Instant::now();
    let timeout = Duration::from_millis(input.timeout_ms);
    let mut interval = Duration::from_millis(200);
    let mut last_error = None;
    let mut last_built = None;

    loop {
        match snapshot::build(
            adapter,
            &input.opts,
            input.app.as_deref(),
            input.window_id.as_deref(),
        ) {
            Ok(mut result) => {
                let present = query::tree_has_match(&result.tree, &query);
                let matched = if input.gone { !present } else { present };
                if matched {
                    let store = RefStore::for_session(context.session_id())?;
                    let snapshot_id = store.save_new_snapshot(&result.refmap)?;
                    trace_artifacts::copy_refmap_if_full(context, &store, &snapshot_id)?;
                    result.snapshot_id = Some(snapshot_id.clone());
                    emit_snapshot_saved(context, &result)?;
                    let elapsed = start.elapsed().as_millis();
                    return snapshot_cmd::format_snapshot_fields(
                        &result,
                        Some(elapsed),
                        Some(&input.query_raw),
                    );
                }
                last_built = Some(result);
            }
            Err(err) if is_target_gone_error(&err) => {
                if input.gone {
                    return Ok(target_absent_response(
                        &input.query_raw,
                        start.elapsed().as_millis(),
                    ));
                }
                last_error = Some(poll_error_json(&err));
            }
            Err(err) if is_transient_poll_error(&err) => {
                last_error = Some(poll_error_json(&err));
            }
            Err(err) => return Err(err),
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            let last_snapshot_id = persist_last_built(context, last_built.as_ref())?;
            return wait_timeout::selector(
                &input.query_raw,
                input.gone,
                input.timeout_ms,
                last_error,
                last_snapshot_id,
            );
        }

        std::thread::sleep(remaining.min(interval));
        interval = (interval * 2).min(Duration::from_millis(1000));
    }
}

fn target_absent_response(query_raw: &str, elapsed_ms: u128) -> Value {
    json!({
        "matched_selector": query_raw,
        "gone": true,
        "target_absent": true,
        "elapsed_ms": elapsed_ms,
    })
}

fn poll_error_json(err: &AppError) -> Value {
    json!({ "code": err.code(), "message": err.to_string() })
}

fn persist_last_built(
    context: &CommandContext,
    last_built: Option<&snapshot::SnapshotResult>,
) -> Result<Option<String>, AppError> {
    let Some(result) = last_built else {
        return Ok(None);
    };
    let store = RefStore::for_session(context.session_id())?;
    let snapshot_id = store.save_new_snapshot(&result.refmap)?;
    trace_artifacts::copy_refmap_if_full(context, &store, &snapshot_id)?;
    Ok(Some(snapshot_id))
}

fn is_target_gone_error(err: &AppError) -> bool {
    matches!(
        err,
        AppError::Adapter(e)
            if matches!(e.code, ErrorCode::AppNotFound | ErrorCode::WindowNotFound)
    )
}

fn is_transient_poll_error(err: &AppError) -> bool {
    matches!(
        err,
        AppError::Adapter(e)
            if matches!(e.code, ErrorCode::Timeout | ErrorCode::ElementNotFound)
    )
}

#[cfg(test)]
#[path = "wait_selector_tests.rs"]
mod tests;
