use agent_desktop_core::{
    Action, ActionResult, ActionStep, AdapterError, Deadline, ElementState, ErrorCode,
    StepMechanism, action_request::ActionRequest, action_step_outcome::ActionStepOutcome,
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::{
        chain::{ChainContext, ChainDef, execute_chain},
        chain_defs, toggle_state,
    };
    use crate::tree::AXElement;

    pub(crate) fn perform_action(
        el: &AXElement,
        request: &ActionRequest,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let budget = ChainContext {
            dynamic_value: None,
            verified_point: request.verified_point(),
            deadline,
        };
        crate::tree::attributes::set_messaging_timeout(el, deadline)?;
        budget.ensure_budget()?;
        let action = &request.action;
        let label = action.name();
        let mut steps = Vec::new();
        tracing::debug!("action: perform {label}");
        match action {
            Action::Click => {
                steps.extend(run_chain(
                    el,
                    &chain_defs::CLICK_CHAIN,
                    None,
                    request,
                    deadline,
                )?);
            }

            Action::DoubleClick => {
                steps.extend(chain_defs::double_click(el, request, deadline)?);
            }

            Action::RightClick => {
                steps.extend(run_chain(
                    el,
                    &chain_defs::RIGHT_CLICK_CHAIN,
                    None,
                    request,
                    deadline,
                )?);
            }

            Action::Toggle => {
                steps.extend(toggle_state::toggle(el, request.policy, deadline)?);
            }

            Action::SetValue(val) => {
                steps.extend(run_chain(
                    el,
                    &chain_defs::SET_VALUE_CHAIN,
                    Some(val),
                    request,
                    deadline,
                )?);
            }

            Action::SetFocus => {
                steps.extend(run_chain(
                    el,
                    &chain_defs::FOCUS_CHAIN,
                    None,
                    request,
                    deadline,
                )?);
            }

            Action::TypeText(text) => {
                steps.push(crate::actions::type_text::execute_type(
                    el,
                    text.as_str(),
                    request.policy,
                    deadline,
                )?);
            }

            Action::PressKey(combo) => {
                crate::actions::physical_keyboard::press(el, combo, request.policy, deadline)?;
                steps.push(
                    ActionStep::succeeded("PressKey")
                        .with_mechanism(StepMechanism::PhysicalSynthetic)
                        .with_verified(false),
                );
            }

            Action::Expand => {
                steps.extend(run_chain(
                    el,
                    &chain_defs::EXPAND_CHAIN,
                    None,
                    request,
                    deadline,
                )?);
            }

            Action::Collapse => {
                steps.extend(run_chain(
                    el,
                    &chain_defs::COLLAPSE_CHAIN,
                    None,
                    request,
                    deadline,
                )?);
            }

            Action::Select(value) => {
                let verified = crate::actions::extras::select_value(el, value.as_str(), deadline)?;
                steps.push(
                    ActionStep::succeeded("Select")
                        .with_mechanism(StepMechanism::SemanticApi)
                        .with_verified(verified),
                );
            }

            Action::Scroll(direction, amount) => {
                let (mechanism, verified) = crate::actions::scroll::ax_scroll(
                    el,
                    direction,
                    *amount,
                    request.policy,
                    deadline,
                )?;
                steps.push(
                    ActionStep::succeeded("Scroll")
                        .with_mechanism(mechanism)
                        .with_verified(verified),
                );
            }

            Action::Check => {
                steps.extend(toggle_state::check_uncheck(
                    el,
                    true,
                    request.policy,
                    deadline,
                )?);
            }

            Action::Uncheck => {
                steps.extend(toggle_state::check_uncheck(
                    el,
                    false,
                    request.policy,
                    deadline,
                )?);
            }

            Action::TripleClick => {
                steps.extend(chain_defs::triple_click(el, request, deadline)?);
            }

            Action::ScrollTo => {
                steps.extend(run_chain(
                    el,
                    &chain_defs::SCROLL_TO_CHAIN,
                    None,
                    request,
                    deadline,
                )?);
            }

            Action::Clear => {
                steps.extend(run_chain(
                    el,
                    &chain_defs::CLEAR_CHAIN,
                    Some(""),
                    request,
                    deadline,
                )?);
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
        }

        if !delivery_occurred(&steps) {
            return Ok(ActionResult::satisfied_without_delivery(label).with_steps(steps));
        }
        let verified = delivery_was_verified(&steps);
        let mut result = ActionResult::delivered_unverified(label).with_steps(steps);
        if verified {
            result = result.with_verified_delivery();
        }
        if !deadline.is_expired()
            && let Some(state) = crate::actions::post_state::read_post_state(el, action, deadline)
                .map_err(after_delivery)?
        {
            verify_post_state(action, &state).map_err(after_delivery)?;
            result = result.with_state(state);
        }
        Ok(result)
    }

    fn run_chain(
        element: &AXElement,
        definition: &ChainDef,
        dynamic_value: Option<&str>,
        request: &ActionRequest,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        execute_chain(
            element,
            definition,
            &ChainContext {
                dynamic_value,
                verified_point: request.verified_point(),
                deadline,
            },
            request.policy,
        )
    }

    fn delivery_was_verified(steps: &[ActionStep]) -> bool {
        let delivered = steps
            .iter()
            .filter(|step| matches!(step.outcome, ActionStepOutcome::Succeeded))
            .filter_map(ActionStep::verified)
            .collect::<Vec<_>>();
        !delivered.is_empty() && delivered.into_iter().all(|verified| verified)
    }

    fn delivery_occurred(steps: &[ActionStep]) -> bool {
        steps
            .iter()
            .any(|step| matches!(step.outcome, ActionStepOutcome::Succeeded))
    }

    fn after_delivery(error: AdapterError) -> AdapterError {
        error.with_disposition(agent_desktop_core::DeliverySemantics::delivered_unverified())
    }

    fn verify_post_state(action: &Action, state: &ElementState) -> Result<(), AdapterError> {
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

    #[cfg(test)]
    mod tests {
        use super::*;
        use agent_desktop_core::element_state::ElementState;

        #[test]
        fn clear_post_state_fails_when_value_remains() {
            let err = verify_post_state(
                &Action::Clear,
                &ElementState {
                    role: "textfield".into(),
                    states: vec![],
                    value: Some("still here".into()),
                    enabled: None,
                    hidden: None,
                    offscreen: None,
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
                    enabled: None,
                    hidden: None,
                    offscreen: None,
                },
            )
            .unwrap();
        }

        #[test]
        fn result_delivery_is_derived_from_verified_steps() {
            let verified = ActionStep::succeeded("AXValue")
                .with_mechanism(StepMechanism::SemanticApi)
                .with_verified(true);
            let unverified = ActionStep::succeeded("AXPress")
                .with_mechanism(StepMechanism::SemanticApi)
                .with_verified(false);

            assert!(delivery_was_verified(&[verified]));
            assert!(!delivery_was_verified(&[unverified]));
        }

        #[test]
        fn skipped_verified_step_does_not_claim_delivery() {
            let skipped = ActionStep::skipped("AlreadyInState").with_verified(true);

            assert!(!delivery_occurred(std::slice::from_ref(&skipped)));
            assert!(!delivery_was_verified(&[skipped]));
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
        _deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("perform_action"))
    }
}

pub(crate) use imp::perform_action;
