use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, WindowInfo};

#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
const VERIFY_LIMIT: Duration = Duration::from_millis(500);

#[cfg(target_os = "macos")]
pub(crate) fn focused_app(
    deadline: Deadline,
) -> Result<Option<agent_desktop_core::AppInfo>, AdapterError> {
    let Some(owner) = crate::system::app::workspace_apps::focused_owner_until(
        crate::tree::locator_deadline::from_operation(deadline)?,
    )?
    else {
        return Ok(None);
    };
    let identity = capture_identity(owner.pid)?;
    if owner
        .launch_time
        .is_some_and(|time| identity.conflicts_with_launch_time(time))
    {
        return Err(AdapterError::stale_ref_because(
            "Frontmost application changed process instance",
        ));
    }
    if !is_frontmost(identity, deadline)? {
        return Err(AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Frontmost application changed during observation",
        ));
    }
    Ok(Some(agent_desktop_core::AppInfo {
        name: owner.name,
        pid: crate::system::process::process_identity::from_pid_t(owner.pid)?,
        bundle_id: owner.bundle_id,
        process_instance: Some(identity.token()),
        presentation: Some(match owner.activation_policy {
            crate::system::app::workspace_apps::ActivationPolicy::Regular => {
                agent_desktop_core::AppPresentation::Foreground
            }
            _ => agent_desktop_core::AppPresentation::Background,
        }),
    }))
}

#[cfg(target_os = "macos")]
pub(crate) fn focus_process(
    process: &agent_desktop_core::ProcessIdentity,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let identity = crate::system::process::process_identity::require_core(process)
        .map_err(before_activation)?;
    focus_identity(identity, deadline).map(|_| ())
}

#[cfg(target_os = "macos")]
pub(crate) fn ensure_app_focused(pid: i32, deadline: Deadline) -> Result<(), AdapterError> {
    focus_identity(capture_identity(pid)?, deadline).map(|_| ())
}

#[cfg(target_os = "macos")]
fn capture_identity(
    pid: i32,
) -> Result<crate::system::process::process_identity::ProcessIdentity, AdapterError> {
    crate::system::process::process_identity::ProcessIdentity::capture(pid)?
        .ok_or_else(|| AdapterError::stale_ref_because("Target application process exited"))
}

#[cfg(target_os = "macos")]
fn is_frontmost(
    identity: crate::system::process::process_identity::ProcessIdentity,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    deadline.ensure_remaining()?;
    require_same_process(identity)?;
    let app = crate::tree::properties::element::element_for_pid(identity.pid());
    let focused = read_boolean(&app, "AXFrontmost", deadline)?;
    require_same_process(identity)?;
    Ok(focused)
}

#[cfg(target_os = "macos")]
fn require_same_process(
    identity: crate::system::process::process_identity::ProcessIdentity,
) -> Result<(), AdapterError> {
    if identity.still_matches()? {
        Ok(())
    } else {
        Err(AdapterError::stale_ref_because(
            "Target process instance changed during focus verification",
        ))
    }
}

#[cfg(target_os = "macos")]
fn focus_identity(
    identity: crate::system::process::process_identity::ProcessIdentity,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    if is_frontmost(identity, deadline).map_err(before_activation)? {
        return Ok(false);
    }
    crate::system::cocoa_runtime::ensure_cocoa_multithreaded().map_err(before_activation)?;
    deadline.ensure_remaining().map_err(before_activation)?;
    let delivered = crate::system::focus_bridge::activate(identity)?;
    let local_deadline = Instant::now() + VERIFY_LIMIT;
    loop {
        match is_frontmost(identity, deadline) {
            Ok(true) => return Ok(delivered),
            Ok(false) => {}
            Err(error) => {
                return Err(if delivered {
                    after_delivery(error)
                } else {
                    error
                });
            }
        }
        if Instant::now() >= local_deadline || deadline.is_expired() {
            let error = AdapterError::timeout("Application did not become frontmost")
                .with_details(serde_json::json!({ "kind": "focus_verification", "physical_delivery_started": false }));
            return Err(if delivered {
                after_delivery(error)
            } else {
                error
            });
        }
        std::thread::sleep(deadline.remaining_slice(Duration::from_millis(5)).map_err(
            |error| {
                if delivered {
                    after_delivery(error)
                } else {
                    error
                }
            },
        )?);
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn verify_app_focused(pid: i32, deadline: Deadline) -> Result<(), AdapterError> {
    verify_identity_focused(capture_identity(pid)?, deadline)
}

#[cfg(target_os = "macos")]
fn verify_identity_focused(
    identity: crate::system::process::process_identity::ProcessIdentity,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let pid = identity.pid();
    if is_frontmost(identity, deadline)? {
        Ok(())
    } else {
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Target application lost focus before physical input delivery",
        )
        .with_details(serde_json::json!({ "pid": pid, "physical_delivery_started": false }))
        .with_suggestion("Retry after ensuring the target application remains frontmost"))
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn focus_window_impl(
    window: &WindowInfo,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let instance = window.process_instance.as_deref().ok_or_else(|| {
        AdapterError::stale_ref_because("Window has no process-instance identity")
    })?;
    let identity = crate::system::process::process_identity::require_core(
        &agent_desktop_core::ProcessIdentity::new(window.pid, instance),
    )?;
    let element = crate::system::window::window_resolve::window_element_for_info(window, deadline)?;
    let focused = focus_identity(identity, deadline)?;
    crate::system::window::window_ops::raise_window(&element, deadline).map_err(|error| {
        crate::actions::DeliveryTracker::from_delivered_units(usize::from(focused)).annotate(error)
    })?;
    if !identity.still_matches().map_err(after_delivery)? {
        return Err(after_delivery(AdapterError::stale_ref_because(
            "Target window process instance changed while focusing",
        )));
    }
    verify_identity_focused(identity, deadline).map_err(after_delivery)
}

#[cfg(target_os = "macos")]
pub(crate) fn wait_until_main(
    window: &crate::tree::AXElement,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    wait_until(window, "AXMain", deadline, "window did not become main")
}

#[cfg(target_os = "macos")]
pub(crate) fn verify_window_main(
    window: &crate::tree::AXElement,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    if read_boolean(window, "AXMain", deadline)? {
        Ok(())
    } else {
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Target window lost main-window status before physical input delivery",
        )
        .with_details(serde_json::json!({ "physical_delivery_started": false }))
        .with_suggestion("Retry after bringing the target window to the front"))
    }
}

#[cfg(target_os = "macos")]
fn read_boolean(
    element: &crate::tree::AXElement,
    attribute: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    prepare(element, deadline)?;
    let result =
        crate::tree::properties::attributes::copy_bool_attr_result(element, attribute, deadline);
    deadline.ensure_remaining()?;
    match result {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Ok(false),
        Err(error) => Err(map_ax_read_error(error, "verify focus")),
    }
}

#[cfg(target_os = "macos")]
fn wait_until(
    element: &crate::tree::AXElement,
    attribute: &str,
    deadline: Deadline,
    timeout_message: &str,
) -> Result<(), AdapterError> {
    let local_deadline = Instant::now() + VERIFY_LIMIT;
    loop {
        if read_boolean(element, attribute, deadline)? {
            return Ok(());
        }
        deadline.ensure_remaining()?;
        if Instant::now() >= local_deadline {
            return Err(
                AdapterError::timeout(timeout_message).with_details(serde_json::json!({
                    "kind": "focus_verification",
                    "physical_delivery_started": false,
                })),
            );
        }
        let pause = deadline.remaining_slice(Duration::from_millis(5))?;
        std::thread::sleep(pause.min(Duration::from_millis(5)));
    }
}

#[cfg(target_os = "macos")]
fn prepare(element: &crate::tree::AXElement, deadline: Deadline) -> Result<(), AdapterError> {
    crate::tree::properties::attributes::set_messaging_timeout(element, deadline)
}

#[cfg(target_os = "macos")]
pub(crate) fn finish_mutation(
    element: &crate::tree::AXElement,
    error: i32,
    operation: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let delivered = crate::actions::ax_mutation::classify_result(
        element,
        operation,
        "AXUIElement mutation",
        error,
    )?;
    if !delivered {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            format!("Accessibility does not support {operation}"),
        )
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    if deadline.is_expired() {
        return Err(deadline
            .timeout_error()
            .with_details(serde_json::json!({
                "kind": "mutation_completed_after_deadline",
                "operation": operation,
            }))
            .with_disposition(DeliverySemantics::delivered_unverified()));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn map_ax_read_error(error: i32, operation: &str) -> AdapterError {
    use accessibility_sys::{
        kAXErrorAPIDisabled, kAXErrorCannotComplete, kAXErrorInvalidUIElement,
    };

    let code = if error == kAXErrorAPIDisabled {
        ErrorCode::PermDenied
    } else if error == kAXErrorCannotComplete {
        ErrorCode::Timeout
    } else if error == kAXErrorInvalidUIElement {
        ErrorCode::StaleRef
    } else {
        ErrorCode::ActionFailed
    };
    AdapterError::new(code, format!("Could not {operation} through accessibility"))
        .with_details(serde_json::json!({ "ax_error": error }))
        .with_suggestion("Refresh the target and retry")
        .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(target_os = "macos")]
fn before_activation(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(target_os = "macos")]
fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn ensure_app_focused(_pid: i32, _deadline: Deadline) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("focus_app"))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn focus_window_impl(
    _window: &WindowInfo,
    _deadline: Deadline,
) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("focus_window"))
}
