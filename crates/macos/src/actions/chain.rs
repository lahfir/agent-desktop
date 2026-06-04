use agent_desktop_core::error::{AdapterError, ErrorCode};
use agent_desktop_core::{action_request::InteractionPolicy, action_step::ActionStep};

use crate::actions::discovery::ElementCaps;
use crate::tree::AXElement;

pub(crate) use super::chain_context::ChainContext;
pub(crate) use super::chain_def::ChainDef;
pub(crate) use super::chain_step::ChainStep;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::ax_helpers;
    use std::time::{Duration, Instant};

    const DEFAULT_CHAIN_TIMEOUT: Duration = Duration::from_secs(10);
    const MAX_CHAIN_TIMEOUT_MS: u64 = 300_000;

    pub(crate) fn execute_chain(
        el: &AXElement,
        caps: &ElementCaps,
        def: &ChainDef,
        ctx: &ChainContext,
        policy: InteractionPolicy,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        let deadline = ctx
            .deadline
            .unwrap_or_else(|| Instant::now() + chain_timeout());
        let total = def.steps.len();
        let mut steps = Vec::new();

        if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
            ax_helpers::set_messaging_timeout(&crate::tree::element_for_pid(pid), 1.0);
        }
        ax_helpers::set_messaging_timeout(el, 1.0);

        if def.pre_scroll {
            tracing::debug!("chain: pre-scroll AXScrollToVisible");
            ax_helpers::ensure_visible(el);
            steps.push(ActionStep::attempted("AXScrollToVisible"));
        }

        for (i, step) in def.steps.iter().enumerate() {
            if Instant::now() > deadline {
                tracing::debug!("chain: timeout after {i}/{total} steps, trying CGClick fallback");
                if let Some(cg) = def
                    .steps
                    .iter()
                    .find(|s| matches!(s, ChainStep::CGClick { .. }))
                {
                    let label = step_label(cg);
                    if physical_click_permitted(policy) && execute_step(el, caps, cg, ctx, policy)?
                    {
                        tracing::debug!("chain: CGClick fallback succeeded");
                        steps.push(ActionStep::succeeded(label));
                        return Ok(steps);
                    }
                }
                return Err(
                    AdapterError::timeout("Chain execution deadline exceeded").with_suggestion(
                        "Retry the command, refresh the snapshot, or increase AGENT_DESKTOP_CHAIN_TIMEOUT_MS for slow apps.",
                    ),
                );
            }
            if matches!(step, ChainStep::CGClick { .. }) && !physical_click_permitted(policy) {
                return Err(AdapterError::policy_denied(
                    "Physical click fallback is disabled by the current interaction policy",
                ));
            }
            let label = step_label(step);
            if execute_step(el, caps, step, ctx, policy)? {
                tracing::debug!("chain: [{}/{}] {} -> success", i + 1, total, label);
                steps.push(ActionStep::succeeded(label));
                return Ok(steps);
            }
            tracing::debug!("chain: [{}/{}] {} -> skip", i + 1, total, label);
            steps.push(ActionStep::skipped(label));
        }

        tracing::debug!("chain: all {total} steps exhausted");
        Err(
            AdapterError::new(ErrorCode::ActionFailed, "All chain steps exhausted")
                .with_suggestion(def.suggestion),
        )
    }

    fn step_label(step: &ChainStep) -> &'static str {
        match step {
            ChainStep::Action(name) => name,
            ChainStep::SetBool { attr, .. } => attr,
            ChainStep::SetDynamic { attr } => attr,
            ChainStep::FocusThenSetDynamic { attr } => attr,
            ChainStep::FocusThenClearByKeyboard => "FocusThenClearByKeyboard",
            ChainStep::ChildActions { .. } => "ChildActions",
            ChainStep::AncestorActions { .. } => "AncestorActions",
            ChainStep::Custom { label, .. } => label,
            ChainStep::CGClick { .. } => "CGClick",
        }
    }

    fn execute_step(
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

            ChainStep::Custom { label: _, func } => func(el, caps),

            ChainStep::CGClick { button, count } => {
                Ok(
                    crate::actions::dispatch::click_via_bounds(el, button.clone(), *count, policy)
                        .is_ok(),
                )
            }
        }
    }

    fn chain_timeout() -> Duration {
        std::env::var("AGENT_DESKTOP_CHAIN_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|ms| *ms > 0)
            .map(|ms| ms.min(MAX_CHAIN_TIMEOUT_MS))
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_CHAIN_TIMEOUT)
    }

    fn physical_click_permitted(policy: InteractionPolicy) -> bool {
        policy.allow_focus_steal && policy.allow_cursor_move
    }

    fn set_dynamic_verified(el: &AXElement, attr: &str, value: &str) -> Result<bool, AdapterError> {
        ax_helpers::set_ax_string_or_err(el, attr, value)?;
        Ok(dynamic_write_had_effect(
            attr,
            ax_helpers::element_role(el).as_deref(),
            value,
            crate::tree::copy_value_typed(el).as_deref(),
        ))
    }

    fn set_bool_verified(el: &AXElement, attr: &str, value: bool) -> Result<bool, AdapterError> {
        Ok(ax_helpers::set_ax_bool_or_err(el, attr, value)?
            && bool_write_had_effect(attr, value, crate::tree::copy_bool_attr(el, attr)))
    }

    fn bool_write_had_effect(attr: &str, expected: bool, observed: Option<bool>) -> bool {
        !matches!(
            attr,
            "AXExpanded" | "AXDisclosing" | "AXSelected" | "AXFocused"
        ) || observed == Some(expected)
    }

    fn dynamic_write_had_effect(
        attr: &str,
        role: Option<&str>,
        expected: &str,
        observed: Option<&str>,
    ) -> bool {
        attr != "AXValue" || role == Some("AXSecureTextField") || observed == Some(expected)
    }

    #[cfg(test)]
    mod tests {
        use super::{bool_write_had_effect, dynamic_write_had_effect};

        #[test]
        fn ax_value_write_requires_readback_match() {
            assert!(!dynamic_write_had_effect(
                "AXValue",
                Some("AXTextField"),
                "",
                Some("unchanged")
            ));
            assert!(dynamic_write_had_effect(
                "AXValue",
                Some("AXTextField"),
                "",
                Some("")
            ));
        }

        #[test]
        fn non_value_and_secure_writes_trust_ax_success() {
            assert!(dynamic_write_had_effect(
                "AXSelected",
                Some("AXCheckBox"),
                "true",
                None
            ));
            assert!(dynamic_write_had_effect(
                "AXValue",
                Some("AXSecureTextField"),
                "secret",
                None
            ));
        }

        #[test]
        fn bool_state_writes_require_readback_match_for_stateful_attrs() {
            assert!(bool_write_had_effect("AXExpanded", true, Some(true)));
            assert!(!bool_write_had_effect("AXExpanded", true, Some(false)));
            assert!(!bool_write_had_effect("AXExpanded", false, None));
            assert!(bool_write_had_effect("AXFoo", true, None));
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn execute_chain(
        _el: &AXElement,
        _caps: &ElementCaps,
        def: &ChainDef,
        _ctx: &ChainContext,
        _policy: InteractionPolicy,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Chain execution not supported on this platform",
        )
        .with_suggestion(def.suggestion))
    }
}

pub(crate) use imp::execute_chain;
