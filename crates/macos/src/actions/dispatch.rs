use agent_desktop_core::{
    action::{
        Action, ActionRequest, ActionResult, InteractionPolicy, MouseButton, MouseEvent,
        MouseEventKind, Point,
    },
    error::{AdapterError, ErrorCode},
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::{
        ax_helpers,
        chain::{ChainContext, execute_chain},
        chain_defs, discovery, toggle_state,
    };
    use crate::tree::AXElement;

    pub(crate) fn click_via_bounds(
        el: &AXElement,
        button: MouseButton,
        count: u32,
        policy: InteractionPolicy,
    ) -> Result<(), AdapterError> {
        if !policy.allow_cursor_move || !policy.allow_focus_steal {
            return Err(AdapterError::policy_denied(
                "Physical click fallback is disabled by the current interaction policy",
            ));
        }
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

    pub(crate) fn perform_action(
        el: &AXElement,
        request: &ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        let action = &request.action;
        let label = action_label(action);
        tracing::debug!("action: perform {label}");
        match action {
            Action::Click => {
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: None,
                    deadline: None,
                };
                execute_chain(el, &caps, &chain_defs::CLICK_CHAIN, &ctx, request.policy)?;
            }

            Action::DoubleClick => {
                let caps = discovery::discover(el);
                chain_defs::double_click(el, &caps, request.policy)?;
            }

            Action::RightClick => {
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: None,
                    deadline: None,
                };
                execute_chain(
                    el,
                    &caps,
                    &chain_defs::RIGHT_CLICK_CHAIN,
                    &ctx,
                    request.policy,
                )?;
            }

            Action::Toggle => {
                toggle_state::toggle(el, request.policy)?;
            }

            Action::SetValue(val) => {
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: Some(val),
                    deadline: None,
                };
                execute_chain(
                    el,
                    &caps,
                    &chain_defs::SET_VALUE_CHAIN,
                    &ctx,
                    request.policy,
                )?;
            }

            Action::SetFocus => {
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: None,
                    deadline: None,
                };
                execute_chain(el, &caps, &chain_defs::FOCUS_CHAIN, &ctx, request.policy)?;
            }

            Action::TypeText(text) => {
                crate::actions::type_text::execute_type(el, text, request.policy)?;
            }

            Action::PressKey(combo) => {
                crate::input::keyboard::synthesize_key(combo)?;
            }

            Action::Expand => {
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: None,
                    deadline: None,
                };
                execute_chain(el, &caps, &chain_defs::EXPAND_CHAIN, &ctx, request.policy)?;
            }

            Action::Collapse => {
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: None,
                    deadline: None,
                };
                execute_chain(el, &caps, &chain_defs::COLLAPSE_CHAIN, &ctx, request.policy)?;
            }

            Action::Select(value) => {
                crate::actions::extras::select_value(el, value)?;
            }

            Action::Scroll(direction, amount) => {
                crate::actions::scroll::ax_scroll(el, direction, *amount, request.policy)?;
            }

            Action::Check => {
                toggle_state::check_uncheck(el, true, request.policy)?;
            }

            Action::Uncheck => {
                toggle_state::check_uncheck(el, false, request.policy)?;
            }

            Action::TripleClick => {
                let caps = discovery::discover(el);
                chain_defs::triple_click(el, &caps, request.policy)?;
            }

            Action::ScrollTo => {
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: None,
                    deadline: None,
                };
                execute_chain(
                    el,
                    &caps,
                    &chain_defs::SCROLL_TO_CHAIN,
                    &ctx,
                    request.policy,
                )?;
            }

            Action::Clear => {
                let caps = discovery::discover(el);
                let ctx = ChainContext {
                    dynamic_value: Some(""),
                    deadline: None,
                };
                execute_chain(el, &caps, &chain_defs::CLEAR_CHAIN, &ctx, request.policy)?;
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

        let mut result = ActionResult::new(label);
        if let Some(state) = crate::actions::post_state::read_post_state(el, action) {
            verify_post_state(action, &state)?;
            result = result.with_state(state);
        }
        Ok(result)
    }

    fn verify_post_state(
        action: &Action,
        state: &agent_desktop_core::action::ElementState,
    ) -> Result<(), AdapterError> {
        if matches!(action, Action::Clear)
            && state
                .value
                .as_deref()
                .is_some_and(|value| !value.is_empty())
        {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "Clear reported success but element value is still non-empty",
            )
            .with_suggestion("Retry 'clear', or use 'press cmd+a' then 'press delete'."));
        }
        Ok(())
    }

    pub(crate) fn ax_press_or_fail(el: &AXElement, context: &str) -> Result<(), AdapterError> {
        if !ax_helpers::ax_press(el) {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                format!("{context}: AXPress failed"),
            )
            .with_suggestion("Element may not be pressable. Try 'click' instead."));
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use agent_desktop_core::action::ElementState;

        #[test]
        fn clear_post_state_fails_when_value_remains() {
            let err = verify_post_state(
                &Action::Clear,
                &ElementState {
                    role: "textfield".into(),
                    states: vec![],
                    value: Some("still here".into()),
                },
            )
            .unwrap_err();

            assert_eq!(err.code, ErrorCode::ActionFailed);
        }

        #[test]
        fn clear_post_state_accepts_empty_value() {
            verify_post_state(
                &Action::Clear,
                &ElementState {
                    role: "textfield".into(),
                    states: vec![],
                    value: Some(String::new()),
                },
            )
            .unwrap();
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;
    use crate::tree::AXElement;

    pub fn perform_action(
        _el: &AXElement,
        _request: &ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("perform_action"))
    }
}

pub(crate) use imp::perform_action;

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
