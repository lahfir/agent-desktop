use agent_desktop_core::{
    ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
    InteractionPolicy, KeyCombo, ProcessIdentity, StepMechanism,
};
use std::time::{Duration, Instant};

use crate::tree::AXElement;

#[path = "key_dispatch_menu.rs"]
mod menu;

pub(crate) fn press_for_app_impl(
    process: ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    press_for_app(process, combo, policy, deadline)
        .map_err(|error| crate::actions::DeliveryTracker::default().annotate(error))
}

fn press_for_app(
    process: ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    tracing::debug!(
        pid = process.pid.get(),
        key = combo.key,
        "system: press key for app"
    );
    let identity = crate::system::process_identity::require_core(&process)?;
    let pid = identity.pid();
    let app = crate::tree::element_for_pid(pid);

    if uses_menu_shortcut(combo, policy) {
        match menu::try_menu_bar_shortcut(&app, combo, deadline) {
            Ok(true) => {
                return post_delivery_process_result(&process, semantic_step("AXPress menu item"));
            }
            Ok(false) => {}
            Err(error) if menu::is_bounded_menu_incomplete(&error) => {
                tracing::debug!(details = ?error.details, "bounded menu lookup fell back to exact PID-targeted key delivery");
            }
            Err(error) => return Err(error),
        }
    }
    if let Some(action) = simple_key_action(combo)
        && try_simple_key_action(&app, action, deadline)?
    {
        return post_delivery_process_result(&process, semantic_step(action));
    }

    if policy.allow_focus_steal {
        crate::system::process_identity::require_core(&process)?;
        crate::system::focus::verify_app_focused(pid, deadline)?;
    }
    crate::system::process_identity::require_core(&process)?;
    require_focused_element(&app, deadline)?;
    crate::system::process_identity::require_core(&process)?;
    crate::input::keyboard::synthesize_key(combo, Some(pid), deadline)?;
    post_delivery_process_result(
        &process,
        ActionStep::succeeded("CGEventPostToPid")
            .with_mechanism(StepMechanism::PhysicalSynthetic)
            .with_verified(false),
    )
}

fn semantic_step(label: &'static str) -> ActionStep {
    ActionStep::succeeded(label)
        .with_mechanism(StepMechanism::SemanticApi)
        .with_verified(false)
}

fn post_delivery_process_result(
    process: &ProcessIdentity,
    step: ActionStep,
) -> Result<ActionResult, AdapterError> {
    crate::system::process_identity::require_core(process)
        .map_err(|error| error.with_disposition(DeliverySemantics::delivered_unverified()))?;
    Ok(ActionResult::delivered_unverified("press_key").with_steps(vec![step]))
}

fn try_simple_key_action(
    app: &AXElement,
    action: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let Some(focused) = focused_element(app, deadline)? else {
        return Ok(false);
    };
    if crate::actions::ax_helpers::advertises_action(&focused, action, deadline) != Some(true) {
        return Ok(false);
    }
    crate::actions::ax_helpers::try_ax_action_or_err(&focused, action, deadline)
}

/// Performing a menu item runs its action against whichever window the app
/// considers focused. An inactive app has none, so apps such as VS Code answer
/// by opening and activating a new window. Only callers that authorized focus
/// changes, whose target window core has already focused, take this route;
/// headless callers get the keystroke itself.
fn uses_menu_shortcut(combo: &KeyCombo, policy: InteractionPolicy) -> bool {
    policy.allow_focus_steal && !combo.modifiers.is_empty()
}

/// Only keys whose meaning is the focused element's default or cancel action
/// have a semantic equivalent. Printable keys, space included, must arrive as
/// keystrokes: performing `AXPress` on a focused text input drops the character.
fn simple_key_action(combo: &KeyCombo) -> Option<&'static str> {
    if !combo.modifiers.is_empty() {
        return None;
    }
    match combo.key.as_str() {
        "return" | "enter" => Some("AXConfirm"),
        "escape" | "esc" => Some("AXCancel"),
        _ => None,
    }
}

fn require_focused_element(app: &AXElement, deadline: Deadline) -> Result<AXElement, AdapterError> {
    let local_deadline = local_deadline(deadline, Duration::from_millis(500))?;
    loop {
        if let Some(focused) = focused_element(app, deadline)? {
            return Ok(focused);
        }
        ensure_budget(deadline)?;
        if Instant::now() >= local_deadline {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "Application has no verified focused element for keyboard delivery",
            )
            .with_details(serde_json::json!({ "physical_delivery_started": false })));
        }
        let pause = deadline.remaining_slice(Duration::from_millis(5))?;
        std::thread::sleep(pause.min(Duration::from_millis(5)));
    }
}

fn focused_element(app: &AXElement, deadline: Deadline) -> Result<Option<AXElement>, AdapterError> {
    element_attribute(app, "AXFocusedUIElement", deadline)
}

fn element_attribute(
    element: &AXElement,
    attribute: &str,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let result = crate::tree::attributes::copy_element_attr_result(element, attribute, deadline);
    ensure_budget(deadline)?;
    result.map_err(|error| read_error(attribute, error))
}

fn ensure_budget(deadline: Deadline) -> Result<(), AdapterError> {
    if deadline.is_expired() {
        Err(deadline.timeout_error())
    } else {
        Ok(())
    }
}

fn local_deadline(deadline: Deadline, maximum: Duration) -> Result<Instant, AdapterError> {
    let remaining = deadline.remaining_slice(maximum)?;
    Instant::now()
        .checked_add(remaining)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Deadline is out of range"))
}

fn read_error(attribute: &str, error: i32) -> AdapterError {
    AdapterError::new(
        if error == accessibility_sys::kAXErrorCannotComplete {
            ErrorCode::Timeout
        } else if error == accessibility_sys::kAXErrorAPIDisabled {
            ErrorCode::PermDenied
        } else if error == accessibility_sys::kAXErrorInvalidUIElement {
            ErrorCode::StaleRef
        } else {
            ErrorCode::ActionFailed
        },
        format!("Accessibility read failed for {attribute} during key dispatch"),
    )
    .with_details(serde_json::json!({ "attribute": attribute, "ax_error": error }))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn press_for_app_impl(
    _process: ProcessIdentity,
    _combo: &KeyCombo,
    _policy: InteractionPolicy,
    _deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    Err(AdapterError::not_supported("press_for_app"))
}

#[cfg(test)]
#[path = "key_dispatch_tests.rs"]
mod tests;
