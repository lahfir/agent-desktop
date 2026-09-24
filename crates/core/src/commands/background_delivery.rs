use serde_json::{Value, json};

use crate::{
    AdapterError, AppError, BackgroundDeliveryReport, BackgroundFocusGuard, DeliverySemantics,
    ErrorCode, RefEntry, WindowInfo, WindowState, context::CommandContext,
};

/// Rules shared by every opt-in background delivery (`--background` pointer
/// and keyboard input): events go to the process that owns one exact window,
/// never through the shared cursor or the system key window, so they belong to
/// neither the headless semantic path nor headed physical input.
pub(crate) fn reject_headed(context: &CommandContext) -> Result<(), AppError> {
    if !context.is_headed() {
        return Ok(());
    }
    Err(AppError::invalid_input_with_suggestion(
        "--background cannot be combined with --headed",
        "Drop --headed to post to the background window, or drop --background to use real input.",
    ))
}

/// The exact window a ref was captured from. The title is left empty because
/// titles are mutable and the pid, process instance, and window number
/// already pin the identity.
pub(crate) fn ref_window(entry: &RefEntry) -> Result<WindowInfo, AppError> {
    let process_instance = entry
        .process
        .process_instance
        .clone()
        .filter(|instance| !instance.is_empty());
    let window_id = entry
        .source
        .source_window_id
        .clone()
        .filter(|id| !id.is_empty());
    let (Some(process_instance), Some(window_id)) = (process_instance, window_id) else {
        return Err(not_delivered(
            ErrorCode::ActionNotSupported,
            "Background delivery needs a ref captured from one exact window",
        )
        .with_suggestion("Snapshot the target window again and retry with the new ref.")
        .into());
    };
    Ok(WindowInfo {
        id: window_id,
        title: String::new(),
        app: entry.source.source_app.clone().unwrap_or_default(),
        pid: entry.process.pid,
        process_instance: Some(process_instance),
        bounds: None,
        state: WindowState::default(),
    })
}

/// Adds `background`, `disposition`, and any focus `warning` to a command's
/// response. Success is always `delivered_unverified` (`retry: unsafe`): the
/// target decides what to do with posted events, so the caller must observe
/// the effect with a fresh snapshot instead of retrying blindly.
pub(crate) fn attach_report(
    mut response: Value,
    window: &WindowInfo,
    report: &BackgroundDeliveryReport,
) -> Value {
    let focus_change = report.focus_change();
    let mut background = json!({
        "pid": window.pid,
        "window_id": window.id,
        "focus_change": focus_change,
        "layers": report.layers,
    });
    if let Some(pid) = report.frontmost_pid_before {
        background["frontmost_pid_before"] = json!(pid);
    }
    if let Some(pid) = report.frontmost_pid_after {
        background["frontmost_pid_after"] = json!(pid);
    }
    if !report.degradations.is_empty() {
        background["degraded"] = json!(report.degradations);
    }
    if let Some(guard) = report.focus_guard {
        background["focus_guard"] = json!(guard);
    }
    response["background"] = background;
    response["disposition"] = json!(DeliverySemantics::delivered_unverified());

    if let Some(warning) = focus_warning(focus_change, report.focus_guard) {
        response["warning"] = json!(warning);
    }
    response
}

fn focus_warning(focus_change: &str, guard: Option<BackgroundFocusGuard>) -> Option<String> {
    let steal_ms = guard.map_or(0, |guard| guard.max_steal_ms);
    let yielded = guard.is_some_and(|guard| guard.yielded);
    match focus_change {
        "changed" if yielded => Some(
            "Another application became frontmost during background delivery; the focus guard treated it as a deliberate switch and left it alone"
                .to_string(),
        ),
        "changed" => Some(
            "The frontmost application changed during background delivery and was not restored; the target may have activated itself"
                .to_string(),
        ),
        "restored" => Some(format!(
            "The target briefly became frontmost (up to {steal_ms} ms) during background delivery before the previous application was frontmost again"
        )),
        _ => None,
    }
}

pub(crate) fn not_delivered(code: ErrorCode, message: impl Into<String>) -> AdapterError {
    AdapterError::new(code, message).with_disposition(DeliverySemantics::not_delivered())
}
