#[cfg(target_os = "macos")]
use agent_desktop_core::error::{AdapterError, ErrorCode};

#[cfg(target_os = "macos")]
use crate::tree::AXElement;

#[cfg(target_os = "macos")]
pub(crate) fn select_value(el: &AXElement, value: &str) -> Result<(), AdapterError> {
    use crate::actions::dispatch::{ax_press_or_fail, ax_set_value, element_role};

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
    use accessibility_sys::{
        kAXErrorSuccess, AXUIElementPerformAction, AXUIElementPostKeyboardEvent,
        AXUIElementSetAttributeValue,
    };
    use agent_desktop_core::action::Direction;
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};

    let scroll_area = find_scroll_area(el);
    let target = scroll_area.as_ref().unwrap_or(el);

    let scroll_visible = CFString::new("AXScrollToVisible");
    if unsafe { AXUIElementPerformAction(el.0, scroll_visible.as_concrete_TypeRef()) }
        == kAXErrorSuccess
    {
        return Ok(());
    }

    let (bar_attr, inc_action) = match direction {
        Direction::Down => ("AXVerticalScrollBar", "AXIncrement"),
        Direction::Up => ("AXVerticalScrollBar", "AXDecrement"),
        Direction::Right => ("AXHorizontalScrollBar", "AXIncrement"),
        Direction::Left => ("AXHorizontalScrollBar", "AXDecrement"),
    };

    if let Some(bar) = get_scroll_bar(target, bar_attr) {
        let ax_action = CFString::new(inc_action);
        let mut ok = true;
        for _ in 0..amount {
            if unsafe { AXUIElementPerformAction(bar.0, ax_action.as_concrete_TypeRef()) }
                != kAXErrorSuccess
            {
                ok = false;
                break;
            }
        }
        if ok {
            return Ok(());
        }
    }

    let page_action = match direction {
        Direction::Down => "AXScrollDownByPage",
        Direction::Up => "AXScrollUpByPage",
        Direction::Right => "AXScrollRightByPage",
        Direction::Left => "AXScrollLeftByPage",
    };
    if crate::actions::dispatch::has_ax_action(target, page_action) {
        let ax = CFString::new(page_action);
        for _ in 0..amount {
            unsafe { AXUIElementPerformAction(target.0, ax.as_concrete_TypeRef()) };
        }
        return Ok(());
    }

    if let Some(bar) = get_scroll_bar(target, bar_attr) {
        if try_scroll_bar_value_shift(&bar, direction, amount) {
            return Ok(());
        }
        if try_scroll_bar_sub_elements(&bar, direction) {
            return Ok(());
        }
    }

    if try_focus_child_in_direction(target, direction) {
        return Ok(());
    }
    if try_select_row_in_direction(target, direction) {
        return Ok(());
    }

    if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
        let keycode: u16 = match direction {
            Direction::Down => 121,
            Direction::Up => 116,
            Direction::Right => 124,
            Direction::Left => 123,
        };
        let cf_focused = CFString::new("AXFocused");
        unsafe {
            AXUIElementSetAttributeValue(
                target.0,
                cf_focused.as_concrete_TypeRef(),
                CFBoolean::true_value().as_CFTypeRef(),
            )
        };
        std::thread::sleep(std::time::Duration::from_millis(50));
        let app = crate::tree::element_for_pid(pid);
        for _ in 0..amount {
            unsafe {
                AXUIElementPostKeyboardEvent(app.0, 0, keycode, true);
                AXUIElementPostKeyboardEvent(app.0, 0, keycode, false);
            };
        }
        return Ok(());
    }

    if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
        let _ = crate::system::app_ops::ensure_app_focused(pid);
        if let Some(b) = crate::tree::read_bounds(target) {
            let (dy, dx) = match direction {
                Direction::Down => (-(amount as i32) * 5, 0),
                Direction::Up => (amount as i32 * 5, 0),
                Direction::Right => (0, -(amount as i32) * 5),
                Direction::Left => (0, amount as i32 * 5),
            };
            return crate::input::mouse::synthesize_scroll_at(
                b.x + b.width / 2.0,
                b.y + b.height / 2.0,
                dy,
                dx,
            );
        }
    }

    Err(AdapterError::new(
        ErrorCode::ActionNotSupported,
        "No scroll mechanism found on element",
    )
    .with_suggestion("Element may not be scrollable, or try the parent container."))
}

#[cfg(target_os = "macos")]
fn try_scroll_bar_value_shift(
    bar: &AXElement,
    direction: &agent_desktop_core::action::Direction,
    amount: u32,
) -> bool {
    use accessibility_sys::{kAXErrorSuccess, AXUIElementSetAttributeValue};
    use agent_desktop_core::action::Direction;
    use core_foundation::{base::TCFType, number::CFNumber, string::CFString};

    if !crate::actions::activate::is_attr_settable(bar, "AXValue") {
        return false;
    }
    let current = read_scroll_bar_value(bar).unwrap_or(0.0);
    let delta = 0.1 * amount as f64;
    let new_val = match direction {
        Direction::Down | Direction::Right => (current + delta).min(1.0),
        Direction::Up | Direction::Left => (current - delta).max(0.0),
    };
    let cf_num = CFNumber::from(new_val as f32);
    let cf_attr = CFString::new("AXValue");
    let err = unsafe {
        AXUIElementSetAttributeValue(bar.0, cf_attr.as_concrete_TypeRef(), cf_num.as_CFTypeRef())
    };
    err == kAXErrorSuccess
}

#[cfg(target_os = "macos")]
fn read_scroll_bar_value(bar: &AXElement) -> Option<f64> {
    use accessibility_sys::{kAXErrorSuccess, AXUIElementCopyAttributeValue};
    use core_foundation::{base::TCFType, number::CFNumber, string::CFString};

    let cf_attr = CFString::new("AXValue");
    let mut value: core_foundation::base::CFTypeRef = std::ptr::null_mut();
    let err =
        unsafe { AXUIElementCopyAttributeValue(bar.0, cf_attr.as_concrete_TypeRef(), &mut value) };
    if err != kAXErrorSuccess || value.is_null() {
        return None;
    }
    let cf = unsafe { core_foundation::base::CFType::wrap_under_create_rule(value) };
    cf.downcast::<CFNumber>().and_then(|n| n.to_f64())
}

#[cfg(target_os = "macos")]
fn try_scroll_bar_sub_elements(
    bar: &AXElement,
    direction: &agent_desktop_core::action::Direction,
) -> bool {
    use accessibility_sys::{kAXErrorSuccess, AXUIElementPerformAction};
    use agent_desktop_core::action::Direction;
    use core_foundation::{base::TCFType, string::CFString};

    let children = crate::tree::copy_ax_array(bar, "AXChildren").unwrap_or_default();
    let target_subroles = match direction {
        Direction::Down | Direction::Right => &["AXIncrementPage", "AXIncrementArrow"],
        Direction::Up | Direction::Left => &["AXDecrementPage", "AXDecrementArrow"],
    };
    let press = CFString::new("AXPress");
    for child in &children {
        let sr = crate::tree::copy_string_attr(child, "AXSubrole").unwrap_or_default();
        if target_subroles.iter().any(|t| *t == sr)
            && unsafe { AXUIElementPerformAction(child.0, press.as_concrete_TypeRef()) }
                == kAXErrorSuccess
        {
            return true;
        }
    }
    false
}

#[cfg(target_os = "macos")]
fn try_focus_child_in_direction(
    scroll_area: &AXElement,
    _direction: &agent_desktop_core::action::Direction,
) -> bool {
    use accessibility_sys::{kAXErrorSuccess, AXUIElementSetAttributeValue};
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};

    let children = crate::tree::copy_ax_array(scroll_area, "AXChildren").unwrap_or_default();
    let child = match children.first() {
        Some(c) => c,
        None => return false,
    };
    let grandchildren = crate::tree::copy_ax_array(child, "AXChildren").unwrap_or_default();
    let target = match grandchildren.last() {
        Some(t) => t,
        None => return false,
    };
    let cf_attr = CFString::new("AXFocused");
    let err = unsafe {
        AXUIElementSetAttributeValue(
            target.0,
            cf_attr.as_concrete_TypeRef(),
            CFBoolean::true_value().as_CFTypeRef(),
        )
    };
    err == kAXErrorSuccess
}

#[cfg(target_os = "macos")]
fn try_select_row_in_direction(
    scroll_area: &AXElement,
    _direction: &agent_desktop_core::action::Direction,
) -> bool {
    use accessibility_sys::{kAXErrorSuccess, kAXRoleAttribute, AXUIElementSetAttributeValue};
    use core_foundation::{
        array::CFArray,
        base::{CFRetain, CFType, CFTypeRef, TCFType},
        string::CFString,
    };

    let children = crate::tree::copy_ax_array(scroll_area, "AXChildren").unwrap_or_default();
    for child in &children {
        let role = crate::tree::copy_string_attr(child, kAXRoleAttribute);
        if !matches!(role.as_deref(), Some("AXTable" | "AXOutline" | "AXList")) {
            continue;
        }
        if !crate::actions::activate::is_attr_settable(child, "AXSelectedRows") {
            continue;
        }
        let rows = crate::tree::copy_ax_array(child, "AXRows").unwrap_or_default();
        if let Some(last) = rows.last() {
            unsafe { CFRetain(last.0 as CFTypeRef) };
            let el_as_cftype = unsafe { CFType::wrap_under_create_rule(last.0 as CFTypeRef) };
            let arr = CFArray::from_CFTypes(&[el_as_cftype]);
            let cf_attr = CFString::new("AXSelectedRows");
            let err = unsafe {
                AXUIElementSetAttributeValue(
                    child.0,
                    cf_attr.as_concrete_TypeRef(),
                    arr.as_CFTypeRef(),
                )
            };
            if err == kAXErrorSuccess {
                return true;
            }
        }
    }
    false
}

#[cfg(target_os = "macos")]
fn find_scroll_area(el: &AXElement) -> Option<AXElement> {
    use accessibility_sys::kAXRoleAttribute;
    let role = crate::tree::copy_string_attr(el, kAXRoleAttribute)?;
    if role == "AXScrollArea" {
        return Some(el.clone());
    }
    let mut current = crate::tree::copy_element_attr(el, "AXParent")?;
    for _ in 0..5 {
        let r = crate::tree::copy_string_attr(&current, kAXRoleAttribute)?;
        if r == "AXScrollArea" {
            return Some(current);
        }
        current = crate::tree::copy_element_attr(&current, "AXParent")?;
    }
    None
}

#[cfg(target_os = "macos")]
fn get_scroll_bar(scroll_area: &AXElement, bar_attr: &str) -> Option<AXElement> {
    crate::tree::copy_element_attr(scroll_area, bar_attr)
}
