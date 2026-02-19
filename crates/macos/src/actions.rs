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
    use core_graphics::{
        event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, ScrollEventUnit},
        event_source::{CGEventSource, CGEventSourceStateID},
        geometry::CGPoint,
    };

    pub fn perform_action(el: &AXElement, action: &Action) -> Result<ActionResult, AdapterError> {
        let label = action_label(action);
        match action {
            Action::Click => {
                let err = ax_press(el);
                if err != kAXErrorSuccess {
                    cg_mouse_click(el, 1, CGEventType::LeftMouseDown, CGEventType::LeftMouseUp, CGMouseButton::Left)?;
                }
            }

            Action::DoubleClick => {
                cg_mouse_click(el, 2, CGEventType::LeftMouseDown, CGEventType::LeftMouseUp, CGMouseButton::Left)?;
            }

            Action::RightClick => {
                let ax_action = CFString::new("AXShowMenu");
                let err = unsafe { AXUIElementPerformAction(el.0, ax_action.as_concrete_TypeRef()) };
                if err != kAXErrorSuccess {
                    cg_mouse_click(el, 1, CGEventType::RightMouseDown, CGEventType::RightMouseUp, CGMouseButton::Right)?;
                }
            }

            Action::Toggle => {
                let err = ax_press(el);
                if err != kAXErrorSuccess {
                    cg_mouse_click(el, 1, CGEventType::LeftMouseDown, CGEventType::LeftMouseUp, CGMouseButton::Left)?;
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
                        format!("SetValue failed (err={err})"),
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
                        format!("AXExpand failed (err={err})"),
                    ));
                }
            }

            Action::Collapse => {
                let ax_action = CFString::new("AXCollapse");
                let err = unsafe { AXUIElementPerformAction(el.0, ax_action.as_concrete_TypeRef()) };
                if err != kAXErrorSuccess {
                    return Err(AdapterError::new(
                        agent_desktop_core::error::ErrorCode::ActionFailed,
                        format!("AXCollapse failed (err={err})"),
                    ));
                }
            }

            Action::Select(_) => {
                let err = ax_press(el);
                if err != kAXErrorSuccess {
                    cg_mouse_click(el, 1, CGEventType::LeftMouseDown, CGEventType::LeftMouseUp, CGMouseButton::Left)?;
                }
            }

            Action::Scroll(direction, amount) => {
                let center = element_center(el).ok_or_else(|| {
                    AdapterError::new(
                        agent_desktop_core::error::ErrorCode::ActionFailed,
                        "Cannot scroll: element has no position/size",
                    )
                })?;

                let (dx, dy): (i32, i32) = match direction {
                    agent_desktop_core::action::Direction::Up    => (0,  *amount as i32),
                    agent_desktop_core::action::Direction::Down  => (0, -(*amount as i32)),
                    agent_desktop_core::action::Direction::Left  => (-(*amount as i32), 0),
                    agent_desktop_core::action::Direction::Right => (*amount as i32, 0),
                };

                let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                    .map_err(|_| AdapterError::internal("CGEventSource failed"))?;
                let event = CGEvent::new_scroll_event(source, ScrollEventUnit::LINE, 2, dy, dx, 0)
                    .map_err(|_| AdapterError::internal("CGEvent scroll failed"))?;
                event.set_location(center);
                event.post(CGEventTapLocation::HID);
            }
        }

        Ok(ActionResult::new(label))
    }

    fn ax_press(el: &AXElement) -> i32 {
        let action = CFString::new(kAXPressAction);
        unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) }
    }

    fn element_center(el: &AXElement) -> Option<CGPoint> {
        let bounds = crate::tree::read_bounds(el)?;
        Some(CGPoint::new(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0))
    }

    fn cg_mouse_click(
        el: &AXElement,
        click_count: i64,
        down: CGEventType,
        up: CGEventType,
        button: CGMouseButton,
    ) -> Result<(), AdapterError> {
        let center = element_center(el).ok_or_else(|| {
            AdapterError::new(
                agent_desktop_core::error::ErrorCode::ActionFailed,
                "Cannot click: element has no accessible position or size",
            )
        })?;

        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| AdapterError::internal("CGEventSource failed"))?;

        let ev_down = CGEvent::new_mouse_event(source.clone(), down, center, button)
            .map_err(|_| AdapterError::internal("CGEvent mouse_down failed"))?;
        let ev_up = CGEvent::new_mouse_event(source, up, center, button)
            .map_err(|_| AdapterError::internal("CGEvent mouse_up failed"))?;

        ev_down.set_integer_value_field(
            core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
            click_count,
        );
        ev_up.set_integer_value_field(
            core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
            click_count,
        );

        ev_down.post(CGEventTapLocation::HID);
        ev_up.post(CGEventTapLocation::HID);
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
