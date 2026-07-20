use agent_desktop_core::{
    AdapterError, AppInfo, Deadline, DeliverySemantics, ErrorCode, WindowInfo,
    launch_options::LaunchOptions,
};
use std::time::{Duration, Instant};

const MAX_ARGUMENT_COUNT: usize = 256;
const MAX_ENVIRONMENT_COUNT: usize = 256;
const MAX_LAUNCH_TEXT_BYTES: usize = 1024 * 1024;

enum LaunchTarget {
    Existing { pid: i32, process_instance: String },
    New { baseline_instances: Vec<String> },
}

#[cfg(target_os = "macos")]
pub(crate) fn launch_app_impl(
    id: &str,
    options: &LaunchOptions,
    parent_deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    validate_app_identifier(id).map_err(before_launch)?;
    validate_launch_options(options).map_err(before_launch)?;
    let deadline = if options.timeout_ms == 0 {
        parent_deadline
    } else {
        parent_deadline.capped(Duration::from_millis(options.timeout_ms))
    };
    ensure_launch_budget(deadline, id).map_err(before_launch)?;
    let timeout_ms = options.timeout_ms;
    let initial = matching_apps(id, deadline).map_err(before_launch)?;
    if options.attach_if_running && initial.len() == 1 {
        let app = &initial[0];
        let instance = required_instance(app).map_err(before_launch)?;
        let pid = crate::system::process_identity::to_pid_t(app.pid).map_err(before_launch)?;
        if let Some(window) = exact_window(pid, &instance, deadline).map_err(before_launch)? {
            return Ok(window);
        }
    }
    let target = launch_target(options, initial).map_err(before_launch)?;

    let launched = crate::system::launch_workspace::open(id, options, deadline)?;
    validate_launched_target(&target, &launched).map_err(after_launch)?;
    let mut poll_interval = Duration::from_millis(50);
    loop {
        if let Some(window) =
            exact_window(launched.0, &launched.1, deadline).map_err(after_launch)?
        {
            return Ok(window);
        }
        if !should_poll_after_first_observation(options.timeout_ms) {
            return Err(launch_no_window_error(id, timeout_ms, &launched));
        }
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            return Err(launch_no_window_error(id, timeout_ms, &launched));
        }
        std::thread::sleep(poll_interval.min(remaining));
        poll_interval = (poll_interval * 3 / 2).min(Duration::from_millis(250));
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn launch_app_impl(
    _id: &str,
    _options: &LaunchOptions,
    _deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    Err(AdapterError::not_supported("launch_app"))
}

#[cfg(target_os = "macos")]
fn launch_target(
    options: &LaunchOptions,
    mut initial: Vec<AppInfo>,
) -> Result<LaunchTarget, AdapterError> {
    if options.attach_if_running {
        if initial.len() > 1 {
            return Err(ambiguous_apps(&initial));
        }
        if let Some(app) = initial.pop() {
            let process_instance = required_instance(&app)?;
            return Ok(LaunchTarget::Existing {
                pid: crate::system::process_identity::to_pid_t(app.pid)?,
                process_instance,
            });
        }
    }
    let baseline_instances = initial
        .iter()
        .map(required_instance)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LaunchTarget::New { baseline_instances })
}

#[cfg(target_os = "macos")]
fn validate_launched_target(
    target: &LaunchTarget,
    launched: &(i32, String),
) -> Result<(), AdapterError> {
    match target {
        LaunchTarget::Existing {
            pid,
            process_instance,
        } => {
            if launched.0 == *pid && launched.1 == *process_instance {
                Ok(())
            } else {
                Err(AdapterError::new(
                    ErrorCode::AppUnresponsive,
                    "NSWorkspace returned a different application while attaching",
                )
                .with_details(serde_json::json!({
                    "expected_pid": pid,
                    "returned_pid": launched.0,
                    "complete": false,
                })))
            }
        }
        LaunchTarget::New { baseline_instances } => {
            if baseline_instances.contains(&launched.1) {
                Err(AdapterError::new(
                    ErrorCode::AppUnresponsive,
                    "NSWorkspace reused an existing application during an exact fresh launch",
                )
                .with_details(serde_json::json!({
                    "returned_pid": launched.0,
                    "complete": false,
                })))
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn matching_apps(id: &str, deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
    ensure_launch_budget(deadline, id)?;
    Ok(
        crate::system::workspace_apps::list_apps_until(deadline_instant(deadline)?)?
            .into_iter()
            .filter(|app| {
                app.name.eq_ignore_ascii_case(id)
                    || app
                        .bundle_id
                        .as_deref()
                        .is_some_and(|bundle_id| bundle_id.eq_ignore_ascii_case(id))
            })
            .collect(),
    )
}

#[cfg(target_os = "macos")]
fn required_instance(app: &AppInfo) -> Result<String, AdapterError> {
    app.process_instance.clone().ok_or_else(|| {
        AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Running application has no exact process instance token",
        )
        .with_details(serde_json::json!({ "pid": app.pid, "complete": false }))
    })
}

#[cfg(target_os = "macos")]
fn ambiguous_apps(apps: &[AppInfo]) -> AdapterError {
    AdapterError::ambiguous_target("More than one application instance matches the launch target")
        .with_details(serde_json::json!({
            "candidate_pids": apps.iter().map(|app| app.pid).collect::<Vec<_>>(),
        }))
}

#[cfg(target_os = "macos")]
fn exact_window(
    pid: i32,
    process_instance: &str,
    deadline: Deadline,
) -> Result<Option<WindowInfo>, AdapterError> {
    let window = match crate::system::window_inventory::exact_window_for_pid_until(
        pid,
        deadline_instant(deadline)?,
    ) {
        Ok(window) => window,
        Err(error) if error.code == ErrorCode::WindowNotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if window.process_instance.as_deref() != Some(process_instance) {
        return Err(AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Application process instance changed while waiting for its window",
        )
        .with_details(serde_json::json!({ "pid": pid, "complete": false })));
    }
    Ok(Some(window))
}

#[cfg(target_os = "macos")]
fn ensure_launch_budget(deadline: Deadline, id: &str) -> Result<(), AdapterError> {
    if deadline.is_expired() {
        return Err(deadline
            .timeout_error()
            .with_details(serde_json::json!({ "app_name": id })));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn before_launch(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(target_os = "macos")]
fn should_poll_after_first_observation(timeout_ms: u64) -> bool {
    timeout_ms > 0
}

#[cfg(target_os = "macos")]
fn validate_app_identifier(id: &str) -> Result<(), AdapterError> {
    let safe_bundle_id = !looks_like_bundle_id(id)
        || id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        });
    if id.is_empty()
        || id.contains("..")
        || id.contains('/')
        || id.chars().any(char::is_control)
        || !safe_bundle_id
    {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Invalid app identifier: use a bare app name or bundle identifier",
        )
        .with_details(serde_json::json!({ "app_name": id }))
        .with_suggestion("Use an app name like 'Safari' or bundle ID like 'com.apple.Safari'."));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn validate_launch_options(options: &LaunchOptions) -> Result<(), AdapterError> {
    if options.cwd.is_some() {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "macOS Launch Services does not support an exact launch working directory",
        )
        .with_suggestion("Remove --cwd, or start the app through a dedicated launcher script"));
    }
    if options.args.len() > MAX_ARGUMENT_COUNT || options.env.len() > MAX_ENVIRONMENT_COUNT {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Launch argument or environment entry count exceeds the supported limit",
        ));
    }
    let text_bytes = options
        .args
        .iter()
        .map(String::len)
        .chain(
            options
                .env
                .iter()
                .map(|(key, value)| key.len() + value.len()),
        )
        .try_fold(0_usize, usize::checked_add)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Launch options are too large"))?;
    if text_bytes > MAX_LAUNCH_TEXT_BYTES {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Launch argument and environment data exceeds one MiB",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn looks_like_bundle_id(id: &str) -> bool {
    id.contains('.') && !id.ends_with(".app") && !id.contains(' ')
}

#[cfg(target_os = "macos")]
fn launch_no_window_error(id: &str, timeout_ms: u64, launched: &(i32, String)) -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        format!(
            "Application started, but no exact accessible window appeared within {timeout_ms} ms"
        ),
    )
    .with_details(serde_json::json!({
        "app_name": id,
        "pid": launched.0,
        "process_instance": launched.1,
        "retry_safe": false,
    }))
    .with_disposition(DeliverySemantics::delivered_unverified())
    .with_suggestion(
        "Inspect list-apps and list-windows for the returned process; do not repeat the launch blindly.",
    )
}

#[cfg(target_os = "macos")]
fn after_launch(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(target_os = "macos")]
fn deadline_instant(deadline: Deadline) -> Result<Instant, AdapterError> {
    Instant::now()
        .checked_add(deadline.remaining())
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Launch deadline is out of range"))
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
