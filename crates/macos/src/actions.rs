use agent_desktop_core::{
    action::{Action, ActionResult},
    error::AdapterError,
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::tree::AXElement;
    use accessibility_sys::{
        kAXErrorSuccess, kAXFocusedAttribute, kAXPressAction, kAXValueAttribute,
        AXUIElementPerformAction, AXUIElementSetAttributeValue,
    };
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};

    pub fn perform_action(el: &AXElement, action: &Action) -> Result<ActionResult, AdapterError> {
        let action_name = action_label(action);
        match action {
            Action::Click | Action::DoubleClick | Action::RightClick | Action::Toggle => {
                let ax_action = CFString::new(kAXPressAction);
                let err = unsafe {
                    AXUIElementPerformAction(el.0, ax_action.as_concrete_TypeRef())
                };
                if err != kAXErrorSuccess {
                    return Err(AdapterError::new(
                        agent_desktop_core::error::ErrorCode::ActionFailed,
                        format!("AXPress failed with error {err}"),
                    ));
                }
            }
            Action::SetValue(val) => {
                let cf_attr = CFString::new(kAXValueAttribute);
                let cf_val = CFString::new(val);
                let err = unsafe {
                    AXUIElementSetAttributeValue(
                        el.0,
                        cf_attr.as_concrete_TypeRef(),
                        cf_val.as_CFTypeRef(),
                    )
                };
                if err != kAXErrorSuccess {
                    return Err(AdapterError::new(
                        agent_desktop_core::error::ErrorCode::ActionFailed,
                        format!("SetValue failed with error {err}"),
                    ));
                }
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
                        agent_desktop_core::error::ErrorCode::ActionFailed,
                        format!("SetFocus failed with error {err}"),
                    ));
                }
            }
            Action::TypeText(text) => {
                crate::input::synthesize_text(text)?;
            }
            Action::PressKey(combo) => {
                crate::input::synthesize_key(combo)?;
            }
            Action::Expand => {
                let ax_action = CFString::new("AXExpand");
                let err = unsafe { AXUIElementPerformAction(el.0, ax_action.as_concrete_TypeRef()) };
                if err != kAXErrorSuccess {
                    return Err(AdapterError::new(
                        agent_desktop_core::error::ErrorCode::ActionFailed,
                        format!("AXExpand failed with error {err}"),
                    ));
                }
            }
            Action::Collapse => {
                let ax_action = CFString::new("AXCollapse");
                let err = unsafe { AXUIElementPerformAction(el.0, ax_action.as_concrete_TypeRef()) };
                if err != kAXErrorSuccess {
                    return Err(AdapterError::new(
                        agent_desktop_core::error::ErrorCode::ActionFailed,
                        format!("AXCollapse failed with error {err}"),
                    ));
                }
            }
            Action::Select(_) => {
                let ax_action = CFString::new(kAXPressAction);
                let err = unsafe { AXUIElementPerformAction(el.0, ax_action.as_concrete_TypeRef()) };
                if err != kAXErrorSuccess {
                    return Err(AdapterError::new(
                        agent_desktop_core::error::ErrorCode::ActionFailed,
                        format!("AXPress (select) failed with error {err}"),
                    ));
                }
            }
            Action::Scroll(direction, amount) => {
                use core_graphics::{
                    event::{CGEvent, CGEventTapLocation, ScrollEventUnit},
                    event_source::{CGEventSource, CGEventSourceStateID},
                };
                let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                    .map_err(|_| AdapterError::internal("CGEventSource failed"))?;
                let (dx, dy) = match direction {
                    agent_desktop_core::action::Direction::Up => (0i32, *amount as i32),
                    agent_desktop_core::action::Direction::Down => (0i32, -(*amount as i32)),
                    agent_desktop_core::action::Direction::Left => (-(*amount as i32), 0i32),
                    agent_desktop_core::action::Direction::Right => (*amount as i32, 0i32),
                };
                let event = CGEvent::new_scroll_event(
                    source,
                    ScrollEventUnit::LINE,
                    2,
                    dy,
                    dx,
                    0,
                )
                .map_err(|_| AdapterError::internal("CGEvent scroll failed"))?;
                event.post(CGEventTapLocation::HID);
            }
        }

        Ok(ActionResult::new(action_name))
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

fn action_label(action: &Action) -> String {
    match action {
        Action::Click => "click",
        Action::DoubleClick => "double_click",
        Action::RightClick => "right_click",
        Action::SetValue(_) => "set_value",
        Action::SetFocus => "set_focus",
        Action::Expand => "expand",
        Action::Collapse => "collapse",
        Action::Select(_) => "select",
        Action::Toggle => "toggle",
        Action::Scroll(_, _) => "scroll",
        Action::PressKey(_) => "press_key",
        Action::TypeText(_) => "type_text",
    }
    .to_string()
}
