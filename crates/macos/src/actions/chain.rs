use agent_desktop_core::action_step_outcome::ActionStepOutcome;
use agent_desktop_core::error::{AdapterError, ErrorCode};
use agent_desktop_core::step_mechanism::StepMechanism;
use agent_desktop_core::{action_step::ActionStep, interaction_policy::InteractionPolicy};

use crate::actions::discovery::ElementCaps;
use crate::tree::AXElement;

pub(crate) use super::chain_context::ChainContext;
pub(crate) use super::chain_def::ChainDef;
pub(crate) use super::chain_step::ChainStep;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::ax_helpers;
    use crate::actions::chain_step_exec::execute_step;
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
        let ctx = ChainContext {
            dynamic_value: ctx.dynamic_value,
            deadline: Some(deadline),
        };
        let total = def.steps.len();
        let mut steps = Vec::new();

        if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
            ax_helpers::set_messaging_timeout(&crate::tree::element_for_pid(pid), 1.0);
        }
        ax_helpers::set_messaging_timeout(el, 1.0);

        if def.pre_scroll {
            tracing::debug!("chain: pre-scroll AXScrollToVisible");
            ax_helpers::ensure_visible(el);
            steps.push(
                ActionStep::attempted("AXScrollToVisible")
                    .with_mechanism(StepMechanism::SemanticApi),
            );
        }

        for (i, step) in def.steps.iter().enumerate() {
            if Instant::now() > deadline {
                tracing::debug!("chain: timeout after {i}/{total} steps, trying CGClick fallback");
                if let Some(cg) = def
                    .steps
                    .iter()
                    .find(|s| matches!(s, ChainStep::CGClick { .. }))
                {
                    if physical_click_permitted(policy) && execute_step(el, caps, cg, &ctx, policy)?
                    {
                        tracing::debug!("chain: CGClick fallback succeeded");
                        steps.push(build_step(cg, ActionStepOutcome::Succeeded));
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
                return Err(AdapterError::policy_denied_for_policy(
                    "Physical click fallback is disabled by the current interaction policy",
                    policy,
                ));
            }
            let label = step_label(step);
            if execute_step(el, caps, step, &ctx, policy)? {
                tracing::debug!("chain: [{}/{}] {} -> success", i + 1, total, label);
                steps.push(build_step(step, ActionStepOutcome::Succeeded));
                return Ok(steps);
            }
            tracing::debug!("chain: [{}/{}] {} -> skip", i + 1, total, label);
            steps.push(build_step(step, ActionStepOutcome::Skipped));
        }

        tracing::debug!("chain: all {total} steps exhausted");
        Err(
            AdapterError::new(ErrorCode::ActionFailed, "All chain steps exhausted")
                .with_suggestion(def.suggestion),
        )
    }

    pub(crate) fn step_mechanism(step: &ChainStep) -> StepMechanism {
        match step {
            ChainStep::CGClick { .. } | ChainStep::FocusThenClearByKeyboard => {
                StepMechanism::PhysicalSynthetic
            }
            _ => StepMechanism::SemanticApi,
        }
    }

    pub(crate) fn step_verifies_effect(step: &ChainStep) -> bool {
        match step {
            ChainStep::SetBool { .. }
            | ChainStep::SetDynamic { .. }
            | ChainStep::FocusThenSetDynamic { .. }
            | ChainStep::IncrementToDynamic => true,
            ChainStep::Custom { label, .. } => matches!(
                *label,
                "verified_press" | "value_relay" | "visible_in_scroll_context"
            ),
            ChainStep::CustomWithDeadline { label, .. } => {
                matches!(*label, "expand_verified" | "collapse_verified")
            }
            _ => false,
        }
    }

    pub(crate) fn build_step(step: &ChainStep, outcome: ActionStepOutcome) -> ActionStep {
        let label = step_label(step);
        let mut built = match outcome {
            ActionStepOutcome::Attempted => ActionStep::attempted(label),
            ActionStepOutcome::Skipped => ActionStep::skipped(label),
            ActionStepOutcome::Succeeded => ActionStep::succeeded(label),
        };
        built = built.with_mechanism(step_mechanism(step));
        if matches!(outcome, ActionStepOutcome::Succeeded) && step_verifies_effect(step) {
            built = built.with_verified(true);
        }
        built
    }

    fn step_label(step: &ChainStep) -> &'static str {
        match step {
            ChainStep::Action(name) => name,
            ChainStep::SetBool { attr, .. } => attr,
            ChainStep::SetDynamic { attr } => attr,
            ChainStep::FocusThenSetDynamic { attr } => attr,
            ChainStep::IncrementToDynamic => "IncrementToDynamic",
            ChainStep::FocusThenClearByKeyboard => "FocusThenClearByKeyboard",
            ChainStep::ChildActions { .. } => "ChildActions",
            ChainStep::AncestorActions { .. } => "AncestorActions",
            ChainStep::Custom { label, .. } => label,
            ChainStep::CustomWithDeadline { label, .. } => label,
            ChainStep::CGClick { .. } => "CGClick",
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
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) use imp::{build_step, step_mechanism, step_verifies_effect};

#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn execute_chain(
        _el: &AXElement,
        _caps: &ElementCaps,
        def: &ChainDef,
        _ctx: &ChainContext,
        _policy: InteractionPolicy,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Chain execution not supported on this platform",
        )
        .with_suggestion(def.suggestion))
    }
}

pub(crate) use imp::execute_chain;
