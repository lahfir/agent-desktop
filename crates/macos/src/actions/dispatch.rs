use agent_desktop_core::{
    action::{Action, ActionResult, MouseButton, MouseEvent, MouseEventKind, Point},
    error::{AdapterError, ErrorCode},
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::tree::AXElement;
    use accessibility_sys::{
        kAXErrorSuccess, kAXFocusedAttribute, kAXPressAction, kAXValueAttribute,
        AXUIElementCopyActionNames, AXUIElementPerformAction, AXUIElementSetAttributeValue,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFType, TCFType},
        boolean::CFBoolean,
        string::CFString,
    };

    pub fn try_ax_action(el: &AXElement, name: &str) -> bool {
        let action = CFString::new(name);
        let err = unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
        err == kAXErrorSuccess
    }

    pub fn click_via_bounds(
        el: &AXElement,
        button: MouseButton,
        count: u32,
    ) -> Result<(), AdapterError> {
        let bounds = crate::tree::read_bounds(el).ok_or_else(|| {
            AdapterError::new(ErrorCode::ActionFailed, "Element has no readable bounds")
                .with_suggestion("AX action failed and CGEvent fallback unavailable")
        })?;
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "Element has zero-size bounds",
            ));
        }
        let center = Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        };
        tracing::debug!(
            ?button,
            count,
            x = center.x,
            y = center.y,
            "AX action failed, falling back to CGEvent click"
        );
        crate::input::mouse::synthesize_mouse(MouseEvent {
            kind: MouseEventKind::Click { count },
            point: center,
            button,
        })
    }

    const TOGGLEABLE_ROLES: &[&str] = &[
        "checkbox",
        "switch",
        "radiobutton",
        "togglebutton",
        "menuitemcheckbox",
        "menuitemradio",
    ];

    pub fn perform_action(el: &AXElement, action: &Action) -> Result<ActionResult, AdapterError> {
        let label = action_label(action);
        match action {
            Action::Click => {
                crate::actions::activate::smart_activate(el)?;
            }

            Action::DoubleClick => {
                crate::actions::activate::smart_double_activate(el)?;
            }

            Action::RightClick => {
                crate::actions::activate::smart_right_activate(el)?;
            }

            Action::Toggle => {
                let role = element_role(el);
                if !TOGGLEABLE_ROLES.iter().any(|r| role.as_deref() == Some(*r)) {
                    return Err(AdapterError::new(
                        ErrorCode::ActionNotSupported,
                        format!(
                            "Toggle not supported on role '{}'",
                            role.as_deref().unwrap_or("unknown")
                        ),
                    )
                    .with_suggestion(
                        "Toggle works on checkboxes, switches, and radio buttons. Use 'click' for other elements.",
                    ));
                }
                crate::actions::activate::smart_activate(el)?;
            }

            Action::SetValue(val) => {
                ax_set_value(el, val)?;
            }

            Action::SetFocus => {
                let cf_attr = CFString::new(kAXFocusedAttribute);
                let err = unsafe {
                    AXUIElementSetAttributeValue(
                        el.0,
                        cf_attr.as_concrete_TypeRef(),
                        CFBoolean::true_value().as_CFTypeRef(),
                    )
                };
                if err != kAXErrorSuccess {
                    return Err(AdapterError::new(
                        ErrorCode::ActionFailed,
                        format!("SetFocus failed (err={err})"),
                    ));
                }
            }

            Action::TypeText(text) => {
                let cf_attr = CFString::new(kAXFocusedAttribute);
                unsafe {
                    AXUIElementSetAttributeValue(
                        el.0,
                        cf_attr.as_concrete_TypeRef(),
                        CFBoolean::true_value().as_CFTypeRef(),
                    )
                };
                crate::input::keyboard::synthesize_text(text)?;
            }

            Action::PressKey(combo) => {
                crate::input::keyboard::synthesize_key(combo)?;
            }

            Action::Expand => {
                if !try_ax_action(el, "AXExpand") {
                    if crate::actions::activate::is_attr_settable(el, "AXDisclosing") {
                        let cf_attr = CFString::new("AXDisclosing");
                        let err = unsafe {
                            AXUIElementSetAttributeValue(
                                el.0,
                                cf_attr.as_concrete_TypeRef(),
                                CFBoolean::true_value().as_CFTypeRef(),
                            )
                        };
                        if err != kAXErrorSuccess {
                            return Err(AdapterError::new(
                                ErrorCode::ActionFailed,
                                format!("AXDisclosing set to true failed (err={err})"),
                            ));
                        }
                    } else {
                        return Err(AdapterError::new(
                            ErrorCode::ActionNotSupported,
                            "AXExpand failed and AXDisclosing not settable",
                        )
                        .with_suggestion("Try 'click' to open it instead."));
                    }
                }
            }

            Action::Collapse => {
                if !try_ax_action(el, "AXCollapse") {
                    if crate::actions::activate::is_attr_settable(el, "AXDisclosing") {
                        let cf_attr = CFString::new("AXDisclosing");
                        let err = unsafe {
                            AXUIElementSetAttributeValue(
                                el.0,
                                cf_attr.as_concrete_TypeRef(),
                                CFBoolean::false_value().as_CFTypeRef(),
                            )
                        };
                        if err != kAXErrorSuccess {
                            return Err(AdapterError::new(
                                ErrorCode::ActionFailed,
                                format!("AXDisclosing set to false failed (err={err})"),
                            ));
                        }
                    } else {
                        return Err(AdapterError::new(
                            ErrorCode::ActionNotSupported,
                            "AXCollapse failed and AXDisclosing not settable",
                        )
                        .with_suggestion("Try 'click' to close it instead."));
                    }
                }
            }

            Action::Select(value) => {
                crate::actions::extras::select_value(el, value)?;
            }

            Action::Scroll(direction, amount) => {
                crate::actions::extras::ax_scroll(el, direction, *amount)?;
            }

            Action::Check => {
                check_uncheck(el, true)?;
            }

            Action::Uncheck => {
                check_uncheck(el, false)?;
            }

            Action::TripleClick => {
                crate::actions::activate::smart_triple_activate(el)?;
            }

            Action::ScrollTo => {
                let ax_action = CFString::new("AXScrollToVisible");
                let err =
                    unsafe { AXUIElementPerformAction(el.0, ax_action.as_concrete_TypeRef()) };
                if err != kAXErrorSuccess {
                    return Err(AdapterError::new(
                        ErrorCode::ActionFailed,
                        format!("AXScrollToVisible failed (err={err})"),
                    )
                    .with_suggestion("Element may not be inside a scrollable area"));
                }
            }

            Action::Clear => {
                ax_set_value(el, "")?;
            }

            Action::KeyDown(_) | Action::KeyUp(_) | Action::Hover | Action::Drag(_) => {
                return Err(AdapterError::new(
                    ErrorCode::ActionNotSupported,
                    format!(
                        "{} requires adapter-level handling, not element action",
                        label
                    ),
                ));
            }

            _ => {
                return Err(AdapterError::not_supported(&label));
            }
        }

        Ok(ActionResult::new(label))
    }

    pub fn ax_press_or_fail(el: &AXElement, context: &str) -> Result<(), AdapterError> {
        let action = CFString::new(kAXPressAction);
        let err = unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
        if err != kAXErrorSuccess {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                format!("{context}: AXPress failed (err={err})"),
            ));
        }
        Ok(())
    }

    pub fn ax_set_value(el: &AXElement, val: &str) -> Result<(), AdapterError> {
        let cf_attr = CFString::new(kAXValueAttribute);
        let cf_val = CFString::new(val);
        let err = unsafe {
            AXUIElementSetAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), cf_val.as_CFTypeRef())
        };
        if err != kAXErrorSuccess {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                format!("SetValue failed (err={err})"),
            ));
        }
        Ok(())
    }

    pub fn element_role(el: &AXElement) -> Option<String> {
        use accessibility_sys::kAXRoleAttribute;
        crate::tree::copy_string_attr(el, kAXRoleAttribute)
            .map(|r| crate::tree::roles::ax_role_to_str(&r).to_string())
    }

    pub fn has_ax_action(el: &AXElement, action_name: &str) -> bool {
        let mut actions_ref: core_foundation_sys::array::CFArrayRef = std::ptr::null();
        let err = unsafe { AXUIElementCopyActionNames(el.0, &mut actions_ref) };
        if err != kAXErrorSuccess || actions_ref.is_null() {
            return false;
        }
        let actions: CFArray<CFType> = unsafe { TCFType::wrap_under_create_rule(actions_ref) };
        let target = CFString::new(action_name);
        for i in 0..actions.len() {
            if let Some(name) = actions.get(i).and_then(|v| v.downcast::<CFString>()) {
                if name == target {
                    return true;
                }
            }
        }
        false
    }

    fn check_uncheck(el: &AXElement, want_checked: bool) -> Result<(), AdapterError> {
        let role = element_role(el);
        if !TOGGLEABLE_ROLES.iter().any(|r| role.as_deref() == Some(*r)) {
            return Err(AdapterError::new(
                ErrorCode::ActionNotSupported,
                format!(
                    "check/uncheck not supported on role '{}'",
                    role.as_deref().unwrap_or("unknown")
                ),
            )
            .with_suggestion("Only works on checkboxes, switches, and radio buttons."));
        }
        let current = crate::tree::copy_string_attr(el, "AXValue");
        let is_checked = current.as_deref() == Some("1");
        if is_checked == want_checked {
            return Ok(());
        }
        crate::actions::activate::smart_activate(el)?;
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;
    use crate::tree::AXElement;

    pub fn perform_action(_el: &AXElement, _action: &Action) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("perform_action"))
    }
}

pub use imp::perform_action;

#[cfg(target_os = "macos")]
pub(crate) use imp::{
    ax_press_or_fail, ax_set_value, click_via_bounds, element_role, has_ax_action,
};

fn action_label(action: &Action) -> String {
    match action {
        Action::Click => "click",
        Action::DoubleClick => "double_click",
        Action::RightClick => "right_click",
        Action::TripleClick => "triple_click",
        Action::SetValue(_) => "set_value",
        Action::SetFocus => "set_focus",
        Action::Expand => "expand",
        Action::Collapse => "collapse",
        Action::Select(_) => "select",
        Action::Toggle => "toggle",
        Action::Check => "check",
        Action::Uncheck => "uncheck",
        Action::Scroll(_, _) => "scroll",
        Action::ScrollTo => "scroll_to",
        Action::PressKey(_) => "press_key",
        Action::KeyDown(_) => "key_down",
        Action::KeyUp(_) => "key_up",
        Action::TypeText(_) => "type_text",
        Action::Clear => "clear",
        Action::Hover => "hover",
        Action::Drag(_) => "drag",
        _ => "unknown",
    }
    .to_string()
}
