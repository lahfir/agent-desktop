#[cfg(target_os = "macos")]
use agent_desktop_core::error::{AdapterError, ErrorCode};

#[cfg(target_os = "macos")]
use crate::tree::AXElement;

#[cfg(target_os = "macos")]
pub(crate) fn select_value(el: &AXElement, value: &str) -> Result<(), AdapterError> {
    use crate::actions::{ax_press_or_fail, ax_set_value, element_role};

    let role = element_role(el);
    match role.as_deref() {
        Some("combobox") => {
            ax_set_value(el, value)?;
        }
        Some("popupbutton") | Some("menubutton") => {
            ax_press_or_fail(el, "select (open popup)")?;
            std::thread::sleep(std::time::Duration::from_millis(200));
            if !find_and_press_menu_item(el, value) {
                press_escape(el);
                return Err(AdapterError::new(
                    ErrorCode::ElementNotFound,
                    format!("No menu item matching '{value}' found"),
                )
                .with_suggestion(
                    "Use 'click' to open the menu, then 'snapshot' to see available options.",
                ));
            }
        }
        Some("list") | Some("table") | Some("outline") => {
            if !select_child_by_name(el, value) {
                return Err(AdapterError::new(
                    ErrorCode::ElementNotFound,
                    format!("No child matching '{value}' found in list"),
                )
                .with_suggestion("Use 'find --role' to discover available items."));
            }
        }
        _ => {
            if ax_set_value(el, value).is_err() {
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
fn find_and_press_menu_item(el: &AXElement, target_value: &str) -> bool {
    use accessibility_sys::{kAXPressAction, AXUIElementPerformAction};
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
    use accessibility_sys::{kAXPressAction, AXUIElementPerformAction};
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

#[cfg(target_os = "macos")]
pub(crate) fn ax_scroll(
    el: &AXElement,
    direction: &agent_desktop_core::action::Direction,
    amount: u32,
) -> Result<(), AdapterError> {
    use accessibility_sys::{kAXErrorSuccess, AXUIElementPerformAction};
    use agent_desktop_core::action::Direction;
    use core_foundation::{base::TCFType, string::CFString};

    let scroll_area = find_scroll_area(el);
    let target = scroll_area.as_ref().unwrap_or(el);

    let (bar_orientation, action_name) = match direction {
        Direction::Down => ("AXVerticalScrollBar", "AXIncrement"),
        Direction::Up => ("AXVerticalScrollBar", "AXDecrement"),
        Direction::Right => ("AXHorizontalScrollBar", "AXIncrement"),
        Direction::Left => ("AXHorizontalScrollBar", "AXDecrement"),
    };

    if let Some(scroll_bar) = get_scroll_bar(target, bar_orientation) {
        let ax_action = CFString::new(action_name);
        for _ in 0..amount {
            let err =
                unsafe { AXUIElementPerformAction(scroll_bar.0, ax_action.as_concrete_TypeRef()) };
            if err != kAXErrorSuccess {
                return Err(AdapterError::new(
                    ErrorCode::ActionFailed,
                    format!("{action_name} on scroll bar failed (err={err})"),
                ));
            }
        }
        return Ok(());
    }

    let scroll_action_name = match direction {
        Direction::Down => "AXScrollDownByPage",
        Direction::Up => "AXScrollUpByPage",
        Direction::Right => "AXScrollRightByPage",
        Direction::Left => "AXScrollLeftByPage",
    };

    if crate::actions::has_ax_action(target, scroll_action_name) {
        let ax_action = CFString::new(scroll_action_name);
        for _ in 0..amount {
            unsafe { AXUIElementPerformAction(target.0, ax_action.as_concrete_TypeRef()) };
        }
        return Ok(());
    }

    Err(AdapterError::new(
        ErrorCode::ActionNotSupported,
        "No scroll bar or scroll action found on this element",
    )
    .with_suggestion("Element may not be scrollable, or try scrolling the parent container."))
}

#[cfg(target_os = "macos")]
fn find_scroll_area(el: &AXElement) -> Option<AXElement> {
    use accessibility_sys::kAXRoleAttribute;

    let role = crate::tree::copy_string_attr(el, kAXRoleAttribute)?;
    if role == "AXScrollArea" {
        return Some(AXElement(el.0));
    }
    let parent = crate::tree::copy_element_attr(el, "AXParent")?;
    let parent_role = crate::tree::copy_string_attr(&parent, kAXRoleAttribute)?;
    if parent_role == "AXScrollArea" {
        return Some(parent);
    }
    None
}

#[cfg(target_os = "macos")]
fn get_scroll_bar(scroll_area: &AXElement, bar_attr: &str) -> Option<AXElement> {
    crate::tree::copy_element_attr(scroll_area, bar_attr)
}
