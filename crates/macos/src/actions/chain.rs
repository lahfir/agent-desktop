use agent_desktop_core::action::MouseButton;
use agent_desktop_core::error::{AdapterError, ErrorCode};

use crate::actions::discovery::ElementCaps;
use crate::tree::AXElement;

#[allow(dead_code)]
pub enum ChainStep {
    Action(&'static str),
    SetBool {
        attr: &'static str,
        value: bool,
    },
    SetDynamic {
        attr: &'static str,
    },
    FocusThenAction(&'static str),
    FocusThenConfirmOrPress,
    ChildActions {
        actions: &'static [&'static str],
        limit: usize,
    },
    AncestorActions {
        actions: &'static [&'static str],
        limit: usize,
    },
    Custom {
        label: &'static str,
        func: fn(&AXElement, &ElementCaps) -> bool,
    },
    CGClick {
        button: MouseButton,
        count: u32,
    },
}

pub struct ChainDef {
    pub pre_scroll: bool,
    pub steps: &'static [ChainStep],
    pub suggestion: &'static str,
}

pub struct ChainContext<'a> {
    pub dynamic_value: Option<&'a str>,
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::ax_helpers;
    use std::time::{Duration, Instant};

    const CHAIN_TIMEOUT: Duration = Duration::from_secs(10);

    pub fn execute_chain(
        el: &AXElement,
        caps: &ElementCaps,
        def: &ChainDef,
        ctx: &ChainContext,
    ) -> Result<(), AdapterError> {
        let deadline = Instant::now() + CHAIN_TIMEOUT;
        let total = def.steps.len();

        ax_helpers::set_messaging_timeout(el, 3.0);
        if def.pre_scroll {
            tracing::debug!("chain: pre-scroll AXScrollToVisible");
            ax_helpers::ensure_visible(el);
        }

        for (i, step) in def.steps.iter().enumerate() {
            if Instant::now() > deadline {
                tracing::debug!("chain: timeout after {i}/{total} steps");
                return Err(AdapterError::timeout("Chain execution exceeded 10s"));
            }
            let label = step_label(step);
            if execute_step(el, caps, step, ctx) {
                tracing::debug!("chain: [{}/{}] {} -> success", i + 1, total, label);
                return Ok(());
            }
            tracing::debug!("chain: [{}/{}] {} -> skip", i + 1, total, label);
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
            ChainStep::FocusThenAction(name) => name,
            ChainStep::FocusThenConfirmOrPress => "FocusThenConfirmOrPress",
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
    ) -> bool {
        match step {
            ChainStep::Action(name) => ax_helpers::try_ax_action_retried(el, name),

            ChainStep::SetBool { attr, value } => {
                let settable = match *attr {
                    "AXSelected" => caps.settable_selected,
                    "AXDisclosing" => caps.settable_disclosing,
                    "AXFocused" => caps.settable_focus,
                    _ => ax_helpers::is_attr_settable(el, attr),
                };
                settable && ax_helpers::set_ax_bool(el, attr, *value)
            }

            ChainStep::SetDynamic { attr } => {
                let value = match ctx.dynamic_value {
                    Some(v) => v,
                    None => return false,
                };
                ax_helpers::set_ax_string_or_err(el, attr, value).is_ok()
            }

            ChainStep::FocusThenAction(name) => {
                if !ax_helpers::ax_focus(el) {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(50));
                ax_helpers::try_ax_action_retried(el, name)
            }

            ChainStep::FocusThenConfirmOrPress => {
                if !ax_helpers::ax_focus(el) {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(50));
                ax_helpers::try_ax_action_retried(el, "AXConfirm")
                    || ax_helpers::try_ax_action_retried(el, "AXPress")
            }

            ChainStep::ChildActions { actions, limit } => ax_helpers::try_each_child(
                el,
                |child| {
                    let child_actions = ax_helpers::list_ax_actions(child);
                    ax_helpers::try_action_from_list(child, &child_actions, actions)
                },
                *limit,
            ),

            ChainStep::AncestorActions { actions, limit } => ax_helpers::try_each_ancestor(
                el,
                |ancestor| {
                    let al = ax_helpers::list_ax_actions(ancestor);
                    ax_helpers::try_action_from_list(ancestor, &al, actions)
                },
                *limit,
            ),

            ChainStep::Custom { label: _, func } => func(el, caps),

            ChainStep::CGClick { button, count } => {
                crate::actions::dispatch::click_via_bounds(el, button.clone(), *count).is_ok()
            }
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
    ) -> Result<(), AdapterError> {
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Chain execution not supported on this platform",
        )
        .with_suggestion(def.suggestion))
    }
}

pub(crate) use imp::execute_chain;
