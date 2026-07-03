use crate::{error::AppError, signals::DesktopSignal};
use serde_json::{Value, json};

pub(crate) fn wait_for_event(
    event: &str,
    app: Option<String>,
    window_id: Option<String>,
    window_title: Option<String>,
    timeout_ms: u64,
    adapter: &dyn crate::adapter::PlatformAdapter,
) -> Result<Value, AppError> {
    let signal = parse_event_signal(event, app, window_id, window_title)?;
    let baseline = adapter.capture_signal_baseline()?;
    let observed = adapter.wait_for_signal(&baseline, &signal, timeout_ms)?;
    Ok(json!({ "signal": serde_json::to_value(observed)? }))
}

pub(crate) fn parse_event_signal(
    event: &str,
    app: Option<String>,
    window_id: Option<String>,
    window_title: Option<String>,
) -> Result<DesktopSignal, AppError> {
    match event {
        "app_activated" => {
            let app = app.ok_or_else(|| {
                AppError::invalid_input_with_suggestion(
                    "--event app_activated requires --app",
                    "Use --event app_activated --app <name>.",
                )
            })?;
            Ok(DesktopSignal::AppActivated { app })
        }
        "window_focused" => {
            let window_id = window_id.ok_or_else(|| {
                AppError::invalid_input_with_suggestion(
                    "--event window_focused requires --window-id",
                    "Use --event window_focused --window-id <id> [--window <title>].",
                )
            })?;
            Ok(DesktopSignal::WindowFocused {
                window_id,
                title: window_title.unwrap_or_default(),
            })
        }
        "window_closed" => {
            let window_id = window_id.ok_or_else(|| {
                AppError::invalid_input_with_suggestion(
                    "--event window_closed requires --window-id",
                    "Use --event window_closed --window-id <id>.",
                )
            })?;
            Ok(DesktopSignal::WindowClosed { window_id })
        }
        other => Err(AppError::invalid_input_with_suggestion(
            format!("Unknown --event value: {other}"),
            "Use app_activated, window_focused, or window_closed.",
        )),
    }
}

#[cfg(test)]
#[path = "wait_event_tests.rs"]
mod tests;
