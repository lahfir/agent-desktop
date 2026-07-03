#[cfg(target_os = "macos")]
mod imp {
    use std::time::Duration;

    use agent_desktop_core::error::AdapterError;
    use agent_desktop_core::interaction_policy::InteractionPolicy;

    use crate::actions::ax_helpers;
    use crate::actions::chain_context::ChainContext;
    use crate::actions::chain_step::ChainStep;
    use crate::actions::chain_value_write::{
        increment_to_value, set_bool_verified, set_dynamic_verified,
    };
    use crate::actions::discovery::ElementCaps;
    use crate::tree::AXElement;

    /// Executes a single [`ChainStep`] against `el`, returning whether the
    /// step succeeded (`Ok(true)`), was skipped because it does not apply to
    /// this element/context (`Ok(false)`), or failed outright (`Err`). The
    /// caller (`chain::execute_chain`) owns retry/fallback policy across the
    /// whole chain definition.
    pub(crate) fn execute_step(
        el: &AXElement,
        caps: &ElementCaps,
        step: &ChainStep,
        ctx: &ChainContext,
        policy: InteractionPolicy,
    ) -> Result<bool, AdapterError> {
        match step {
            ChainStep::Action(name) => ax_helpers::try_ax_action_retried_or_err(el, name),

            ChainStep::SetBool { attr, value } => {
                let settable = match *attr {
                    "AXSelected" => caps.settable_selected,
                    "AXDisclosing" => caps.settable_disclosing,
                    "AXFocused" => caps.settable_focus,
                    _ => ax_helpers::is_attr_settable(el, attr),
                };
                Ok(settable && set_bool_verified(el, attr, *value)?)
            }

            ChainStep::SetDynamic { attr } => {
                let value = match ctx.dynamic_value {
                    Some(v) => v,
                    None => return Ok(false),
                };
                set_dynamic_verified(el, attr, value)
            }

            ChainStep::FocusThenSetDynamic { attr } => {
                if !policy.allow_focus_steal {
                    return Ok(false);
                }
                let value = match ctx.dynamic_value {
                    Some(v) => v,
                    None => return Ok(false),
                };
                if !ax_helpers::ax_focus_or_err(el)? {
                    return Ok(false);
                }
                std::thread::sleep(Duration::from_millis(50));
                set_dynamic_verified(el, attr, value)
            }

            ChainStep::IncrementToDynamic => match ctx.dynamic_value {
                Some(value) => increment_to_value(el, value, ctx.deadline),
                None => Ok(false),
            },

            ChainStep::FocusThenClearByKeyboard => {
                if !policy.allow_focus_steal {
                    return Ok(false);
                }
                if !ax_helpers::ax_focus_or_err(el)? {
                    return Ok(false);
                }
                std::thread::sleep(Duration::from_millis(20));
                Ok(crate::input::keyboard::synthesize_key_for_element(
                    el,
                    &agent_desktop_core::action::KeyCombo {
                        key: "a".into(),
                        modifiers: vec![agent_desktop_core::action::Modifier::Cmd],
                    },
                )
                .and_then(|_| {
                    crate::input::keyboard::synthesize_key_for_element(
                        el,
                        &agent_desktop_core::action::KeyCombo {
                            key: "delete".into(),
                            modifiers: vec![],
                        },
                    )
                })
                .is_ok())
            }

            ChainStep::ChildActions { actions, limit } => Ok(ax_helpers::try_each_child(
                el,
                |child| {
                    let child_actions = ax_helpers::list_ax_actions(child);
                    ax_helpers::try_action_from_list(child, &child_actions, actions)
                },
                *limit,
            )),

            ChainStep::AncestorActions { actions, limit } => Ok(ax_helpers::try_each_ancestor(
                el,
                |ancestor| {
                    let al = ax_helpers::list_ax_actions(ancestor);
                    ax_helpers::try_action_from_list(ancestor, &al, actions)
                },
                *limit,
            )),

            ChainStep::Custom { label: _, func } => func(el),

            ChainStep::CustomWithDeadline { label: _, func } => func(el, ctx.deadline),

            ChainStep::CGClick { button, count } => {
                Ok(
                    crate::actions::dispatch::click_via_bounds(el, button.clone(), *count, policy)
                        .is_ok(),
                )
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::execute_step;
