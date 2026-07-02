#[cfg(target_os = "macos")]
use agent_desktop_core::error::{AdapterError, ErrorCode};

#[cfg(target_os = "macos")]
use crate::tree::AXElement;

#[cfg(target_os = "macos")]
pub(crate) fn select_value(el: &AXElement, value: &str) -> Result<(), AdapterError> {
    use crate::actions::ax_helpers;

    let role = ax_helpers::element_role(el);
    match role.as_deref() {
        Some("combobox") => {
            if set_value_and_verify(el, value) {
                return Ok(());
            }
            let pid = crate::system::app_ops::pid_from_element(el);
            open_menu(el, pid, "select (open combo box)")?;
            if !wait_for_menu_item(el, pid, value) {
                press_escape(el);
                return Err(option_not_found(value));
            }
            wait_for_value(el, value)?;
        }
        Some("popupbutton") | Some("menubutton") => {
            let pid = crate::system::app_ops::pid_from_element(el);
            open_menu(el, pid, "select (open popup)")?;
            if !wait_for_menu_item(el, pid, value) {
                press_escape(el);
                return Err(option_not_found(value));
            }
            if crate::tree::copy_value_typed(el).is_some() {
                wait_for_value(el, value)?;
            }
        }
        Some("list") | Some("table") | Some("outline") => {
            if !select_child_by_name(el, value) {
                return Err(AdapterError::new(
                    ErrorCode::ElementNotFound,
                    format!(
                        "No child matching the requested value ({} chars) found in list",
                        value.chars().count()
                    ),
                )
                .with_suggestion("Use 'find --role' to discover available items."));
            }
        }
        _ => {
            if ax_helpers::ax_set_value(el, value).is_err() {
                return Err(AdapterError::new(
                    ErrorCode::ActionNotSupported,
                    format!(
                        "Select not supported on role '{}'",
                        role.as_deref().unwrap_or("unknown")
                    ),
                )
                .with_suggestion("Use 'click' or 'set-value' instead."));
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_value_and_verify(el: &AXElement, value: &str) -> bool {
    crate::actions::ax_helpers::ax_set_value(el, value).is_ok() && wait_for_value(el, value).is_ok()
}

#[cfg(target_os = "macos")]
fn wait_for_value(el: &AXElement, value: &str) -> Result<(), AdapterError> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(600);
    loop {
        if crate::tree::copy_value_typed(el)
            .as_deref()
            .is_some_and(|current| current.eq_ignore_ascii_case(value))
        {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                format!(
                    "Selection did not change to the requested value ({} chars)",
                    value.chars().count()
                ),
            )
            .with_suggestion("Refresh the snapshot and inspect available values."));
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[cfg(target_os = "macos")]
fn option_not_found(value: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ElementNotFound,
        format!(
            "No menu item matching the requested value ({} chars) found",
            value.chars().count()
        ),
    )
    .with_suggestion("Use 'click' to open the menu, then 'snapshot' to see available options.")
}

#[cfg(target_os = "macos")]
fn find_and_press_open_menu_item(pid: i32, target_value: &str) -> bool {
    crate::tree::surfaces::menu_element_for_pid(pid)
        .as_ref()
        .is_some_and(|menu| find_and_press_menu_item(menu, target_value))
}

#[cfg(target_os = "macos")]
fn open_menu(el: &AXElement, pid: Option<i32>, context: &str) -> Result<(), AdapterError> {
    let was_open = pid.is_some_and(is_menu_open);
    if was_open {
        return Err(menu_already_open());
    }
    if crate::actions::ax_helpers::try_ax_action_retried(el, "AXShowMenu")
        && menu_opened_after_action(was_open, wait_for_open_menu(pid))
    {
        return Ok(());
    }
    crate::actions::dispatch::ax_press_or_fail(el, context)?;
    if pid.is_none() || menu_opened_after_action(was_open, wait_for_open_menu(pid)) {
        return Ok(());
    }
    Err(AdapterError::timeout(format!(
        "No context menu opened within {}ms",
        crate::system::wait::menu_timeout_ms()
    )))
}

#[cfg(target_os = "macos")]
fn wait_for_open_menu(pid: Option<i32>) -> bool {
    pid.is_some_and(|p| {
        crate::system::wait::wait_for_menu(p, true, crate::system::wait::menu_timeout_ms()).is_ok()
    })
}

#[cfg(target_os = "macos")]
fn is_menu_open(pid: i32) -> bool {
    crate::system::wait::wait_for_menu(pid, true, 60).is_ok()
}

#[cfg(target_os = "macos")]
fn wait_for_menu_item(el: &AXElement, pid: Option<i32>, target_value: &str) -> bool {
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_millis(crate::system::wait::menu_timeout_ms());
    loop {
        if find_and_press_menu_item(el, target_value)
            || pid.is_some_and(|p| find_and_press_open_menu_item(p, target_value))
        {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[cfg(target_os = "macos")]
fn menu_already_open() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Refusing to select from a menu while another menu is already open",
    )
    .with_suggestion("Dismiss the open menu and retry the select command.")
}

#[cfg(target_os = "macos")]
fn menu_opened_after_action(was_open: bool, is_open: bool) -> bool {
    !was_open && is_open
}

#[cfg(target_os = "macos")]
fn find_and_press_menu_item(el: &AXElement, target_value: &str) -> bool {
    use accessibility_sys::{AXUIElementPerformAction, kAXPressAction};
    use core_foundation::{base::TCFType, string::CFString};

    let children = crate::tree::copy_ax_array(el, "AXChildren").unwrap_or_default();
    for child in &children {
        let title = crate::tree::copy_string_attr(child, "AXTitle");
        if let Some(t) = &title {
            if t.eq_ignore_ascii_case(target_value) {
                let action = CFString::new(kAXPressAction);
                unsafe { AXUIElementPerformAction(child.0, action.as_concrete_TypeRef()) };
                return true;
            }
        }
        if find_and_press_menu_item(child, target_value) {
            return true;
        }
    }
    false
}

#[cfg(target_os = "macos")]
fn press_escape(el: &AXElement) {
    use accessibility_sys::AXUIElementPerformAction;
    use core_foundation::{base::TCFType, string::CFString};

    let cancel = CFString::new("AXCancel");
    unsafe { AXUIElementPerformAction(el.0, cancel.as_concrete_TypeRef()) };
}

#[cfg(target_os = "macos")]
fn select_child_by_name(el: &AXElement, name: &str) -> bool {
    use accessibility_sys::{AXUIElementPerformAction, kAXPressAction};
    use core_foundation::{base::TCFType, string::CFString};

    let children = crate::tree::copy_ax_array(el, "AXChildren").unwrap_or_default();
    for child in &children {
        let child_name = crate::tree::copy_string_attr(child, "AXTitle")
            .or_else(|| crate::tree::copy_string_attr(child, "AXDescription"));
        if let Some(n) = &child_name {
            if n.eq_ignore_ascii_case(name) {
                let action = CFString::new(kAXPressAction);
                unsafe { AXUIElementPerformAction(child.0, action.as_concrete_TypeRef()) };
                return true;
            }
        }
    }
    false
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::menu_opened_after_action;

    #[test]
    fn menu_guard_rejects_preexisting_menu() {
        assert!(!menu_opened_after_action(true, true));
        assert!(!menu_opened_after_action(true, false));
    }

    #[test]
    fn menu_guard_requires_closed_to_open_transition() {
        assert!(menu_opened_after_action(false, true));
        assert!(!menu_opened_after_action(false, false));
    }
}
