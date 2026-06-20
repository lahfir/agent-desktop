#[cfg(target_os = "macos")]
use agent_desktop_core::{
    error::{AdapterError, ErrorCode},
    interaction_policy::InteractionPolicy,
};

#[cfg(target_os = "macos")]
use crate::tree::AXElement;

#[cfg(target_os = "macos")]
pub(crate) fn ax_scroll(
    el: &AXElement,
    direction: &agent_desktop_core::action::Direction,
    amount: u32,
    policy: InteractionPolicy,
) -> Result<(), AdapterError> {
    use accessibility_sys::{
        AXUIElementPerformAction, AXUIElementSetAttributeValue, kAXErrorSuccess,
    };
    use agent_desktop_core::action::Direction;
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};

    let scroll_area = find_scroll_area(el);
    let target = scroll_area.as_ref().unwrap_or(el);

    let scroll_visible = CFString::new("AXScrollToVisible");
    unsafe { AXUIElementPerformAction(el.0, scroll_visible.as_concrete_TypeRef()) };

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
    if crate::actions::ax_helpers::has_ax_action(target, page_action) {
        let ax = CFString::new(page_action);
        let mut completed = 0;
        for _ in 0..amount {
            if unsafe { AXUIElementPerformAction(target.0, ax.as_concrete_TypeRef()) }
                != kAXErrorSuccess
            {
                break;
            }
            completed += 1;
        }
        if completed == amount {
            return Ok(());
        }
    }

    if let Some(bar) = get_scroll_bar(target, bar_attr) {
        if try_scroll_bar_value_shift(&bar, direction, amount) {
            return Ok(());
        }
        if try_scroll_bar_sub_elements(&bar, direction) {
            return Ok(());
        }
    }

    if policy.allow_focus_steal && try_focus_child_in_direction(target, direction) {
        return Ok(());
    }
    if policy.allow_focus_steal && try_select_row_in_direction(target, direction) {
        return Ok(());
    }

    if policy.allow_focus_steal {
        if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
            let keycode: u16 = match direction {
                Direction::Down => 121,
                Direction::Up => 116,
                Direction::Right => 124,
                Direction::Left => 123,
            };
            let _ = crate::system::app_ops::ensure_app_focused(pid);
            let cf_focused = CFString::new("AXFocused");
            let focus_err = unsafe {
                AXUIElementSetAttributeValue(
                    target.0,
                    cf_focused.as_concrete_TypeRef(),
                    CFBoolean::true_value().as_CFTypeRef(),
                )
            };
            if focus_err == kAXErrorSuccess {
                std::thread::sleep(std::time::Duration::from_millis(50));
                crate::input::keyboard::synthesize_keycode(keycode, amount)?;
                return Ok(());
            }
        }
    }

    if policy.allow_focus_steal && policy.allow_cursor_move {
        if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
            let _ = crate::system::app_ops::ensure_app_focused(pid);
            if let Some(b) = crate::tree::read_bounds(target) {
                let (dy, dx) = scroll_wheel_delta(direction, amount);
                return crate::input::mouse::synthesize_scroll_at(
                    b.x + b.width / 2.0,
                    b.y + b.height / 2.0,
                    dy,
                    dx,
                );
            }
        }
    }

    if policy.allow_focus_steal && !policy.allow_cursor_move {
        return Err(AdapterError::policy_denied_for_policy(
            "Cursor-moving scroll fallback is disabled by the current interaction policy",
            policy,
        ));
    }

    Err(AdapterError::new(
        ErrorCode::ActionNotSupported,
        "No scroll mechanism found on element",
    )
    .with_suggestion("Element may not be scrollable, or try the parent container."))
}

#[cfg(target_os = "macos")]
fn scroll_wheel_delta(
    direction: &agent_desktop_core::action::Direction,
    amount: u32,
) -> (i32, i32) {
    use agent_desktop_core::action::Direction;
    match direction {
        Direction::Down => (-(amount as i32) * 5, 0),
        Direction::Up => (amount as i32 * 5, 0),
        Direction::Right => (0, amount as i32 * 5),
        Direction::Left => (0, -(amount as i32) * 5),
    }
}

#[cfg(target_os = "macos")]
fn try_scroll_bar_value_shift(
    bar: &AXElement,
    direction: &agent_desktop_core::action::Direction,
    amount: u32,
) -> bool {
    use accessibility_sys::{AXUIElementSetAttributeValue, kAXErrorSuccess};
    use agent_desktop_core::action::Direction;
    use core_foundation::{base::TCFType, number::CFNumber, string::CFString};

    if !crate::actions::ax_helpers::is_attr_settable(bar, "AXValue") {
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
    use accessibility_sys::{AXUIElementCopyAttributeValue, kAXErrorSuccess};
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
    use accessibility_sys::{AXUIElementPerformAction, kAXErrorSuccess};
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
    use accessibility_sys::{AXUIElementSetAttributeValue, kAXErrorSuccess};
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
    use accessibility_sys::{AXUIElementSetAttributeValue, kAXErrorSuccess, kAXRoleAttribute};
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
        if !crate::actions::ax_helpers::is_attr_settable(child, "AXSelectedRows") {
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

#[cfg(test)]
mod tests {
    use agent_desktop_core::action::Direction;

    #[test]
    fn horizontal_wheel_delta_matches_direction() {
        assert_eq!(super::scroll_wheel_delta(&Direction::Right, 2), (0, 10));
        assert_eq!(super::scroll_wheel_delta(&Direction::Left, 2), (0, -10));
    }
}
