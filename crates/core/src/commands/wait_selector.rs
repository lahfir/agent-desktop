use crate::{
    adapter::{PlatformAdapter, TreeOptions},
    commands::{
        query::{self, FindQuery},
        snapshot as snapshot_cmd, wait_timeout,
    },
    context::CommandContext,
    error::{AppError, ErrorCode},
    refs_store::RefStore,
    snapshot,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub struct WaitSelectorInput {
    pub query: FindQuery,
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
    if input.query.is_match_everything() {
        return Err(AppError::invalid_input_with_suggestion(
            "Selector must constrain at least role or text",
            "Use forms like \"button:Submit\", \"button\", or \":Saved!\".",
        ));
    }

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
                last_built = Some(result.clone());
                let present = query::tree_has_match(&result.tree, &input.query);
                let matched = if input.gone { !present } else { present };
                if matched {
                    let snapshot_id = RefStore::for_session(context.session_id())?
                        .save_new_snapshot(&result.refmap)?;
                    result.snapshot_id = Some(snapshot_id);
                    let elapsed = start.elapsed().as_millis();
                    return snapshot_cmd::format_snapshot_fields(
                        &result,
                        Some(elapsed),
                        Some(&input.query_raw),
                    );
                }
            }
            Err(err) if is_retryable_wait_app_error(&err) => {
                last_error = Some(json!({
                    "code": err.code(),
                    "message": err.to_string()
                }));
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

fn persist_last_built(
    context: &CommandContext,
    last_built: Option<&snapshot::SnapshotResult>,
) -> Result<Option<String>, AppError> {
    let Some(result) = last_built else {
        return Ok(None);
    };
    let snapshot_id =
        RefStore::for_session(context.session_id())?.save_new_snapshot(&result.refmap)?;
    Ok(Some(snapshot_id))
}

fn is_retryable_wait_poll_error(code: &ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::Timeout
            | ErrorCode::ElementNotFound
            | ErrorCode::AppNotFound
            | ErrorCode::WindowNotFound
    )
}

fn is_retryable_wait_app_error(err: &AppError) -> bool {
    matches!(err, AppError::Adapter(e) if is_retryable_wait_poll_error(&e.code))
}

#[cfg(test)]
#[path = "wait_selector_tests.rs"]
mod tests;
