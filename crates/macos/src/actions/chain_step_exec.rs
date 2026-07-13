#[cfg(target_os = "macos")]
mod imp {
    use agent_desktop_core::AdapterError;
    use agent_desktop_core::interaction_policy::InteractionPolicy;

    use crate::actions::ax_helpers;
    use crate::actions::chain_context::ChainContext;
    use crate::actions::chain_delivery::DeliveryOutcome;
    use crate::actions::chain_step::ChainStep;
    use crate::actions::chain_value_write::{
        increment_to_value, set_bool_verified, set_dynamic_verified,
    };
    use crate::tree::AXElement;

    pub(crate) fn execute_step(
        el: &AXElement,
        step: &ChainStep,
        ctx: &ChainContext,
        policy: InteractionPolicy,
    ) -> Result<DeliveryOutcome, AdapterError> {
        ctx.ensure_budget()?;
        match step {
            ChainStep::Action(name) => {
                prepare(el, ctx.deadline)?;
                Ok(DeliveryOutcome::from_delivery(
                    ax_helpers::try_ax_action_or_err(el, name, ctx.deadline)?,
                    false,
                ))
            }

            ChainStep::SetBool { attr, value } => {
                prepare(el, ctx.deadline)?;
                let settable = ax_helpers::is_attr_settable(el, attr, ctx.deadline)?;
                if settable {
                    set_bool_verified(el, attr, *value, ctx.deadline)
                } else {
                    Ok(DeliveryOutcome::NotDelivered)
                }
            }

            ChainStep::SetDynamic { attr } => {
                let value = match ctx.dynamic_value {
                    Some(v) => v,
                    None => return Ok(DeliveryOutcome::NotDelivered),
                };
                prepare(el, ctx.deadline)?;
                if !ax_helpers::is_attr_settable(el, attr, ctx.deadline)? {
                    return Ok(DeliveryOutcome::NotDelivered);
                }
                set_dynamic_verified(el, attr, value, ctx.deadline)
            }

            ChainStep::IncrementToDynamic => match ctx.dynamic_value {
                Some(value) => increment_to_value(el, value, ctx.deadline),
                None => Ok(DeliveryOutcome::NotDelivered),
            },

            ChainStep::FocusThenClearByKeyboard => {
                if !policy.allow_focus_steal {
                    return Ok(DeliveryOutcome::NotDelivered);
                }
                crate::actions::physical_keyboard::press_sequence(
                    el,
                    &[
                        agent_desktop_core::KeyCombo {
                            key: "a".into(),
                            modifiers: vec![agent_desktop_core::Modifier::Meta],
                        },
                        agent_desktop_core::KeyCombo {
                            key: "delete".into(),
                            modifiers: vec![],
                        },
                    ],
                    policy,
                    ctx.deadline,
                )?;
                Ok(DeliveryOutcome::DeliveredUnverified)
            }

            ChainStep::CustomWithDeadline { label: _, func } => func(el, ctx.deadline),

            ChainStep::CGClick { button, count } => {
                physical_click(el, button.clone(), *count, ctx, policy)?;
                Ok(DeliveryOutcome::DeliveredUnverified)
            }
            ChainStep::CGDisclosureClick { expanded } => {
                if !crate::actions::chain_disclosure_steps::physical_fallback_allowed(
                    el,
                    *expanded,
                    ctx.deadline,
                )? {
                    return Ok(DeliveryOutcome::NotDelivered);
                }
                physical_click(el, agent_desktop_core::MouseButton::Left, 1, ctx, policy)?;
                crate::actions::chain_disclosure_steps::verify_physical_delivery(
                    el,
                    *expanded,
                    ctx.deadline,
                )
            }
        }
    }

    fn physical_click(
        element: &AXElement,
        button: agent_desktop_core::MouseButton,
        count: u32,
        context: &ChainContext<'_>,
        policy: InteractionPolicy,
    ) -> Result<(), AdapterError> {
        context.ensure_budget()?;
        crate::actions::physical_click::click_via_bounds(
            element,
            crate::actions::physical_click::PhysicalClick {
                button,
                count,
                verified_point: context.verified_point.cloned(),
            },
            policy,
            context.deadline,
        )
    }

    fn prepare(
        element: &AXElement,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<(), AdapterError> {
        crate::tree::attributes::set_messaging_timeout(element, deadline)
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::execute_step;
