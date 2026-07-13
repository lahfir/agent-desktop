use agent_desktop_core::step_mechanism::StepMechanism;
use agent_desktop_core::{ActionStep, interaction_policy::InteractionPolicy};
use agent_desktop_core::{AdapterError, ErrorCode};

use crate::actions::chain_delivery::DeliveryOutcome;
use crate::tree::AXElement;

pub(crate) use super::chain_context::ChainContext;
pub(crate) use super::chain_def::ChainDef;
pub(crate) use super::chain_step::ChainStep;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::chain_step_exec::execute_step;

    pub(crate) fn execute_chain(
        el: &AXElement,
        def: &ChainDef,
        ctx: &ChainContext,
        policy: InteractionPolicy,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        let total = def.steps.len();
        let mut steps = Vec::new();
        if let Some(pid) = crate::system::app_ops::pid_from_element(el, ctx.deadline) {
            crate::tree::attributes::set_messaging_timeout(
                &crate::tree::element_for_pid(pid),
                ctx.deadline,
            )?;
        }
        crate::tree::attributes::set_messaging_timeout(el, ctx.deadline)?;

        for (i, step) in def.steps.iter().enumerate() {
            ctx.ensure_budget()?;
            if !step_allowed(step, policy) {
                continue;
            }
            let label = step_label(step);
            let outcome = execute_step(el, step, ctx, policy)?;
            if record_step_outcome(
                &mut steps,
                step,
                outcome,
                def.continue_after_unverified_delivery,
            ) {
                tracing::debug!("chain: [{}/{}] {} -> success", i + 1, total, label);
                return Ok(steps);
            }
            tracing::debug!("chain: [{}/{}] {} -> skip", i + 1, total, label);
        }

        tracing::debug!("chain: all {total} steps exhausted");
        Err(
            AdapterError::new(ErrorCode::ActionFailed, "All chain steps exhausted")
                .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
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

    pub(crate) fn build_step(step: &ChainStep, outcome: DeliveryOutcome) -> ActionStep {
        let label = step_label(step);
        let mut built = match outcome {
            DeliveryOutcome::NotDelivered => ActionStep::skipped(label),
            DeliveryOutcome::SatisfiedNoDelivery => ActionStep::skipped(label).with_verified(true),
            DeliveryOutcome::DeliveredUnverified | DeliveryOutcome::DeliveredVerified => {
                ActionStep::succeeded(label)
            }
        };
        built = built.with_mechanism(step_mechanism(step));
        if outcome.was_delivered() {
            built = built.with_verified(outcome.was_verified());
        }
        built
    }

    pub(crate) fn record_step_outcome(
        steps: &mut Vec<ActionStep>,
        step: &ChainStep,
        outcome: DeliveryOutcome,
        continue_after_unverified_delivery: bool,
    ) -> bool {
        steps.push(build_step(step, outcome));
        outcome.terminates_chain()
            && !(continue_after_unverified_delivery
                && outcome == DeliveryOutcome::DeliveredUnverified)
    }

    fn step_label(step: &ChainStep) -> &'static str {
        match step {
            ChainStep::Action(name) => name,
            ChainStep::SetBool { attr, .. } => attr,
            ChainStep::SetDynamic { attr } => attr,
            ChainStep::IncrementToDynamic => "IncrementToDynamic",
            ChainStep::FocusThenClearByKeyboard => "FocusThenClearByKeyboard",
            ChainStep::CustomWithDeadline { label, .. } => label,
            ChainStep::CGClick { .. } => "CGClick",
        }
    }

    pub(crate) fn step_allowed(step: &ChainStep, policy: InteractionPolicy) -> bool {
        !matches!(
            step,
            ChainStep::CGClick { .. } | ChainStep::FocusThenClearByKeyboard
        ) || policy.is_headed()
    }
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) use imp::{build_step, record_step_outcome, step_allowed, step_mechanism};

#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn execute_chain(
        _el: &AXElement,
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
