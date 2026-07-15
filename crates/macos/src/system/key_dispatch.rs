use agent_desktop_core::{
    ActionResult, AdapterError, Deadline, DeliverySemantics, ErrorCode, KeyCombo, Modifier,
    ProcessIdentity,
};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::tree::AXElement;

const MAX_MENU_NODES: usize = 2_048;
const MAX_MENU_CHILDREN: usize = 128;
const MAX_MENU_DEPTH: u8 = 8;
const AX_MENU_MODIFIER_SHIFT: u32 = 1 << 0;
const AX_MENU_MODIFIER_OPTION: u32 = 1 << 1;
const AX_MENU_MODIFIER_CONTROL: u32 = 1 << 2;
const AX_MENU_MODIFIER_NO_COMMAND: u32 = 1 << 3;

pub(crate) fn press_for_app_impl(
    process: ProcessIdentity,
    combo: &KeyCombo,
    policy: agent_desktop_core::InteractionPolicy,
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
    prepare(&app, deadline)?;

    if !combo.modifiers.is_empty() {
        match try_menu_bar_shortcut(&app, combo, deadline) {
            Ok(true) => return post_delivery_process_result(&process),
            Ok(false) => {}
            Err(error) if is_bounded_menu_incomplete(&error) => {
                tracing::debug!(details = ?error.details, "bounded menu lookup fell back to exact PID-targeted key delivery");
            }
            Err(error) => return Err(error),
        }
    }
    if try_simple_key_action(&app, combo, deadline)? {
        return post_delivery_process_result(&process);
    }

    if policy.allow_focus_steal {
        crate::system::process_identity::require_core(&process)?;
        crate::system::focus::verify_app_focused(pid, deadline)?;
    }
    crate::system::process_identity::require_core(&process)?;
    require_focused_element(&app, deadline)?;
    crate::system::process_identity::require_core(&process)?;
    crate::input::keyboard::synthesize_key(combo, Some(pid), deadline)?;
    post_delivery_process_result(&process)
}

fn post_delivery_process_result(process: &ProcessIdentity) -> Result<ActionResult, AdapterError> {
    crate::system::process_identity::require_core(process)
        .map_err(|error| error.with_disposition(DeliverySemantics::delivered_unverified()))?;
    Ok(ActionResult::delivered_unverified("press_key"))
}

fn try_simple_key_action(
    app: &AXElement,
    combo: &KeyCombo,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    if !combo.modifiers.is_empty() {
        return Ok(false);
    }
    let action = match combo.key.as_str() {
        "return" | "enter" => "AXConfirm",
        "escape" | "esc" => "AXCancel",
        "space" => "AXPress",
        _ => return Ok(false),
    };
    let Some(focused) = focused_element(app, deadline)? else {
        return Ok(false);
    };
    prepare(&focused, deadline)?;
    crate::actions::ax_helpers::try_ax_action_or_err(&focused, action, deadline)
}

fn try_menu_bar_shortcut(
    app: &AXElement,
    combo: &KeyCombo,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let Some(target_char) = single_uppercase_character(&combo.key) else {
        return Ok(false);
    };
    let Some(menu_bar) = element_attribute(app, "AXMenuBar", deadline)? else {
        return Ok(false);
    };
    let target_modifiers = combo_to_ax_modifiers(combo);
    let mut queue = VecDeque::from([(menu_bar, 0_u8)]);
    let mut visited = 0_usize;

    while let Some((element, depth)) = queue.pop_front() {
        ensure_budget(deadline)?;
        visited += 1;
        if visited > MAX_MENU_NODES {
            return Err(incomplete_menu_error("node_limit"));
        }
        if read_string(&element, "AXMenuItemCmdChar", deadline)?
            .is_some_and(|value| value.to_uppercase() == target_char)
            && read_menu_item_modifiers(&element, deadline)? == Some(target_modifiers)
        {
            prepare(&element, deadline)?;
            return crate::actions::ax_helpers::try_ax_action_or_err(&element, "AXPress", deadline);
        }
        if depth >= MAX_MENU_DEPTH {
            if has_children(&element, deadline)? {
                return Err(incomplete_menu_error("depth_limit"));
            }
            continue;
        }
        let children = children(&element, deadline)?;
        queue.extend(
            children
                .into_iter()
                .map(|child| (child, depth.saturating_add(1))),
        );
    }
    Ok(false)
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
    prepare(element, deadline)?;
    let result = crate::tree::attributes::copy_element_attr_result(element, attribute, deadline);
    ensure_budget(deadline)?;
    result.map_err(|error| read_error(attribute, error))
}

fn children(element: &AXElement, deadline: Deadline) -> Result<Vec<AXElement>, AdapterError> {
    let read = crate::tree::query::child_read::read_attribute_children(
        element,
        "AXChildren",
        MAX_MENU_CHILDREN,
        instant(deadline)?,
    );
    ensure_budget(deadline)?;
    validate_menu_children(read)
}

fn has_children(element: &AXElement, deadline: Deadline) -> Result<bool, AdapterError> {
    let read = crate::tree::query::child_read::read_attribute_children(
        element,
        "AXChildren",
        0,
        instant(deadline)?,
    );
    ensure_budget(deadline)?;
    validate_child_status(&read)?;
    Ok(read.total_count > 0)
}

fn validate_menu_children(
    read: crate::tree::query::child_read::ChildRead,
) -> Result<Vec<AXElement>, AdapterError> {
    validate_child_status(&read)?;
    if !read.complete || read.truncated() {
        return Err(
            incomplete_menu_error("child_limit").with_details(serde_json::json!({
                "kind": "child_limit",
                "complete": false,
                "total_count": read.total_count,
                "loaded_count": read.elements.len(),
            })),
        );
    }
    Ok(read.elements)
}

fn validate_child_status(
    read: &crate::tree::query::child_read::ChildRead,
) -> Result<(), AdapterError> {
    if read.status.api_disabled {
        return Err(read_error(
            "AXChildren",
            accessibility_sys::kAXErrorAPIDisabled,
        ));
    }
    if read.status.invalid_element {
        return Err(read_error(
            "AXChildren",
            accessibility_sys::kAXErrorInvalidUIElement,
        ));
    }
    if read.status.health.cannot_complete > 0 || read.status.health.deadline_exhausted > 0 {
        return Err(read_error(
            "AXChildren",
            accessibility_sys::kAXErrorCannotComplete,
        ));
    }
    Ok(())
}

fn read_string(
    element: &AXElement,
    attribute: &str,
    deadline: Deadline,
) -> Result<Option<String>, AdapterError> {
    prepare(element, deadline)?;
    let result = crate::tree::attributes::copy_string_attr_result(element, attribute, deadline);
    ensure_budget(deadline)?;
    result.map_err(|error| read_error(attribute, error))
}

fn read_menu_item_modifiers(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<u32>, AdapterError> {
    use accessibility_sys::kAXErrorSuccess;
    use core_foundation::{base::TCFType, string::CFString};

    prepare(element, deadline)?;
    let attribute = CFString::new("AXMenuItemCmdModifiers");
    let (error, value) = crate::tree::ax_ipc::copy_attribute_value(
        element,
        attribute.as_concrete_TypeRef(),
        deadline,
    );
    ensure_budget(deadline)?;
    if error != kAXErrorSuccess {
        return if error == accessibility_sys::kAXErrorNoValue
            || error == accessibility_sys::kAXErrorAttributeUnsupported
        {
            Ok(None)
        } else {
            Err(read_error("AXMenuItemCmdModifiers", error))
        };
    }
    if value.is_null() {
        return Err(malformed_menu_modifiers());
    }
    let value = unsafe { core_foundation::base::CFType::wrap_under_create_rule(value) };
    let number = crate::cf_type::borrowed_cf_number(value.as_concrete_TypeRef())
        .and_then(|number| number.to_i64())
        .ok_or_else(malformed_menu_modifiers)?;
    normalize_menu_modifiers(number).map(Some)
}

fn single_uppercase_character(key: &str) -> Option<String> {
    (key.chars().count() == 1).then(|| key.to_uppercase())
}

fn combo_to_ax_modifiers(combo: &KeyCombo) -> u32 {
    let mut encoded = if combo.modifiers.contains(&Modifier::Meta) {
        0
    } else {
        AX_MENU_MODIFIER_NO_COMMAND
    };
    encoded |= combo.modifiers.iter().fold(0, |modifiers, modifier| {
        modifiers
            | match modifier {
                Modifier::Shift => AX_MENU_MODIFIER_SHIFT,
                Modifier::Alt => AX_MENU_MODIFIER_OPTION,
                Modifier::Ctrl => AX_MENU_MODIFIER_CONTROL,
                Modifier::Meta => 0,
            }
    });
    encoded
}

fn normalize_menu_modifiers(number: i64) -> Result<u32, AdapterError> {
    let value = u32::try_from(number).map_err(|_| malformed_menu_modifiers())?;
    let supported = AX_MENU_MODIFIER_SHIFT
        | AX_MENU_MODIFIER_OPTION
        | AX_MENU_MODIFIER_CONTROL
        | AX_MENU_MODIFIER_NO_COMMAND;
    if value & !supported != 0 {
        return Err(malformed_menu_modifiers());
    }
    Ok(value)
}

fn malformed_menu_modifiers() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "AXMenuItemCmdModifiers had an invalid value",
    )
    .with_details(serde_json::json!({
        "kind": "menu_modifier_value_invalid",
        "complete": false,
    }))
}

fn instant(deadline: Deadline) -> Result<Instant, AdapterError> {
    Instant::now()
        .checked_add(deadline.remaining())
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Deadline is out of range"))
}

fn is_bounded_menu_incomplete(error: &AdapterError) -> bool {
    error.code == ErrorCode::AppUnresponsive
        && error
            .details
            .as_ref()
            .and_then(|details| details.get("kind"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kind| matches!(kind, "node_limit" | "child_limit" | "depth_limit"))
}

fn prepare(element: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
    crate::tree::attributes::set_messaging_timeout(element, deadline)
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

fn incomplete_menu_error(kind: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Menu shortcut lookup exceeded its bounded accessibility search",
    )
    .with_details(serde_json::json!({ "kind": kind, "complete": false }))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn press_for_app_impl(
    _process: ProcessIdentity,
    _combo: &KeyCombo,
    _policy: agent_desktop_core::InteractionPolicy,
    _deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    Err(AdapterError::not_supported("press_for_app"))
}

#[cfg(test)]
#[path = "key_dispatch_tests.rs"]
mod tests;
