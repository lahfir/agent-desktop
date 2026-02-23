use agent_desktop_core::{
    action::{Action, ActionResult, MouseButton, MouseEvent, MouseEventKind, Point},
    error::{AdapterError, ErrorCode},
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::{
        ax_helpers,
        chain::{execute_chain, ChainContext},
        chain_defs, discovery,
    };
    use crate::tree::AXElement;

    pub fn click_via_bounds(
        el: &AXElement,
        button: MouseButton,
        count: u32,
    ) -> Result<(), AdapterError> {
        if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
            let _ = crate::system::app_ops::ensure_app_focused(pid);
        }
        let bounds = crate::tree::read_bounds(el).ok_or_else(|| {
            AdapterError::new(ErrorCode::ActionFailed, "Element has no readable bounds")
                .with_suggestion("AX action failed and CGEvent fallback unavailable")
        })?;
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return Err(
                AdapterError::new(ErrorCode::ActionFailed, "Element has zero-size bounds")
                    .with_suggestion("Element may be hidden or off-screen. Try 'scroll-to' first."),
            );
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
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: None,
                };
                execute_chain(el, &caps, &chain_defs::CLICK_CHAIN, &ctx)?;
            }

            Action::DoubleClick => {
                let caps = discovery::discover(el);
                chain_defs::double_click(el, &caps)?;
            }

            Action::RightClick => {
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: None,
                };
                execute_chain(el, &caps, &chain_defs::RIGHT_CLICK_CHAIN, &ctx)?;
            }

            Action::Toggle => {
                let role = ax_helpers::element_role(el);
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
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: None,
                };
                execute_chain(el, &caps, &chain_defs::CLICK_CHAIN, &ctx)?;
            }

            Action::SetValue(val) => {
                ax_helpers::ax_set_value(el, val)?;
            }

            Action::SetFocus => {
                if !ax_helpers::ax_focus(el) {
                    return Err(
                        AdapterError::new(ErrorCode::ActionFailed, "SetFocus failed")
                            .with_suggestion("Element may not support focus. Try 'click' instead."),
                    );
                }
            }

            Action::TypeText(text) => {
                ax_helpers::ax_focus(el);
                crate::input::keyboard::synthesize_text(text)?;
            }

            Action::PressKey(combo) => {
                crate::input::keyboard::synthesize_key(combo)?;
            }

            Action::Expand => {
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: None,
                };
                execute_chain(el, &caps, &chain_defs::EXPAND_CHAIN, &ctx)?;
            }

            Action::Collapse => {
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: None,
                };
                execute_chain(el, &caps, &chain_defs::COLLAPSE_CHAIN, &ctx)?;
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
                let caps = discovery::discover(el);
                chain_defs::triple_click(el, &caps)?;
            }

            Action::ScrollTo => {
                if !ax_helpers::try_ax_action(el, "AXScrollToVisible") {
                    return Err(AdapterError::new(
                        ErrorCode::ActionFailed,
                        "AXScrollToVisible failed",
                    )
                    .with_suggestion("Element may not be inside a scrollable area"));
                }
            }

            Action::Clear => {
                ax_helpers::ax_set_value(el, "")?;
            }

            Action::KeyDown(_) | Action::KeyUp(_) | Action::Hover | Action::Drag(_) => {
                return Err(AdapterError::new(
                    ErrorCode::ActionNotSupported,
                    format!(
                        "{} requires adapter-level handling, not element action",
                        label
                    ),
                )
                .with_suggestion("Use the top-level command (e.g. 'hover', 'drag', 'key-down') instead of targeting an element."));
            }

            _ => {
                return Err(AdapterError::not_supported(&label));
            }
        }

        Ok(ActionResult::new(label))
    }

    pub fn ax_press_or_fail(el: &AXElement, context: &str) -> Result<(), AdapterError> {
        if !ax_helpers::ax_press(el) {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                format!("{context}: AXPress failed"),
            )
            .with_suggestion("Element may not be pressable. Try 'click' instead."));
        }
        Ok(())
    }

    fn check_uncheck(el: &AXElement, want_checked: bool) -> Result<(), AdapterError> {
        let role = ax_helpers::element_role(el);
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
        let caps = discovery::discover(el);
        let ctx = ChainContext {
            dynamic_value: None,
        };
        execute_chain(el, &caps, &chain_defs::CLICK_CHAIN, &ctx)
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
pub(crate) use imp::{ax_press_or_fail, click_via_bounds};

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
