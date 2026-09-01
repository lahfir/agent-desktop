//! Activation-chain engine for UIA pattern dispatch.
//!
//! Rung closures return a delivery outcome or `Err`. A genuine `Err` always
//! aborts the chain and never falls through to a later rung. Its disposition,
//! however, is not always the classifier's: chains that continue after an
//! unverified delivery (`VALUE_WRITE_CHAIN`) can hard-error on a later rung
//! after an earlier one already mutated the control, so an `Err` raised once
//! `delivery_occurred` on the recorded steps is upgraded to
//! `delivered_unverified` before it leaves this function — the code,
//! message, suggestion and platform detail are left untouched. Only a clean
//! not-delivered outcome advances to the next rung. Policy-disallowed rungs
//! are skipped with no step recorded (the hook later physical rungs will
//! use). Mechanism is always `semantic_api` for every rung this adapter
//! ships.

use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy,
    StepMechanism,
};
use std::time::{Duration, Instant};

use crate::actions::post_state::delivery_occurred;
use crate::system::permissions::ensure_budget;

pub(crate) const INVOKE_LABEL: &str = "InvokePattern.Invoke";
pub(crate) const ALREADY_LABEL: &str = "AlreadyInState";

/// Caps a verification poll window by the remaining action budget.
pub(crate) fn capped_verification_end(
    deadline: Deadline,
    cap: Duration,
) -> Result<Instant, AdapterError> {
    let remaining = deadline.remaining();
    if remaining.is_zero() {
        return Err(deadline.timeout_error());
    }
    let local = Instant::now() + cap;
    Ok(Instant::now()
        .checked_add(remaining)
        .map_or(local, |end| end.min(local)))
}

/// Per-rung delivery judgment recorded into an [`ActionStep`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeliveryOutcome {
    NotDelivered,
    /// Already-at-target arms (toggle / disclosure) construct this outcome.
    SatisfiedNoDelivery,
    DeliveredUnverified,
    DeliveredVerified,
    /// Delivered, but verification was withheld (secure / unreadable IsPassword).
    DeliveredUnobserved,
}

impl DeliveryOutcome {
    pub(crate) fn from_delivery(delivered: bool, verified: bool) -> Self {
        match (delivered, verified) {
            (false, _) => Self::NotDelivered,
            (true, false) => Self::DeliveredUnverified,
            (true, true) => Self::DeliveredVerified,
        }
    }

    /// Maps a delivered write's observation onto a chain outcome.
    ///
    /// `None` is the secure-field shape: the write landed and no value was
    /// re-read, so the step must not claim `verified: true` or `false`.
    pub(crate) fn from_observation(verified: Option<bool>) -> Self {
        match verified {
            Some(true) => Self::DeliveredVerified,
            Some(false) => Self::DeliveredUnverified,
            None => Self::DeliveredUnobserved,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn was_delivered(self) -> bool {
        matches!(
            self,
            Self::DeliveredUnverified | Self::DeliveredVerified | Self::DeliveredUnobserved
        )
    }

    #[allow(dead_code)]
    pub(crate) fn was_verified(self) -> bool {
        matches!(self, Self::SatisfiedNoDelivery | Self::DeliveredVerified)
    }

    pub(crate) fn terminates_chain(self) -> bool {
        !matches!(self, Self::NotDelivered)
    }
}

/// Chain-level knobs shared by every action that runs through the engine.
pub(crate) struct ChainDef {
    pub(crate) suggestion: &'static str,
    pub(crate) continue_after_unverified_delivery: bool,
}

/// Click chain: Invoke first, Legacy default action second (A19-6, A2-2).
pub(crate) const CLICK_CHAIN: ChainDef = ChainDef {
    suggestion: "Refresh the snapshot and retry, or target an element that advertises Invoke or a Legacy default action.",
    continue_after_unverified_delivery: false,
};

/// One rung whose runner is injected — the unit-test seam.
pub(crate) struct ChainRung<'a> {
    pub(crate) label: &'static str,
    pub(crate) requires_headed: bool,
    pub(crate) run: &'a mut dyn FnMut() -> Result<DeliveryOutcome, AdapterError>,
}

/// Runs gated rungs in order, recording a step for every attempted rung.
pub(crate) fn execute_chain(
    deadline: Deadline,
    def: &ChainDef,
    policy: InteractionPolicy,
    rungs: &mut [ChainRung<'_>],
) -> Result<Vec<ActionStep>, AdapterError> {
    let mut steps = Vec::new();
    for rung in rungs.iter_mut() {
        ensure_budget(deadline)?;
        if !rung_allowed(rung, policy) {
            continue;
        }
        let outcome = match (rung.run)() {
            Ok(outcome) => outcome,
            Err(error) => {
                return Err(if delivery_occurred(&steps) {
                    error.with_disposition(exhaustion_disposition(&steps))
                } else {
                    error
                });
            }
        };
        if record_step_outcome(
            &mut steps,
            rung.label,
            outcome,
            def.continue_after_unverified_delivery,
        ) {
            return Ok(steps);
        }
    }
    Err(
        AdapterError::new(ErrorCode::ActionFailed, "All chain steps exhausted")
            .with_disposition(exhaustion_disposition(&steps))
            .with_suggestion(def.suggestion),
    )
}

pub(crate) fn rung_allowed(rung: &ChainRung<'_>, policy: InteractionPolicy) -> bool {
    !rung.requires_headed || policy.is_headed()
}

pub(crate) fn exhaustion_disposition(steps: &[ActionStep]) -> DeliverySemantics {
    if delivery_occurred(steps) {
        DeliverySemantics::delivered_unverified()
    } else {
        DeliverySemantics::not_delivered()
    }
}

pub(crate) fn build_step(label: &'static str, outcome: DeliveryOutcome) -> ActionStep {
    let built = match outcome {
        DeliveryOutcome::NotDelivered => ActionStep::skipped(label),
        DeliveryOutcome::SatisfiedNoDelivery => ActionStep::skipped(label).with_verified(true),
        DeliveryOutcome::DeliveredUnobserved => ActionStep::succeeded(label),
        DeliveryOutcome::DeliveredUnverified => ActionStep::succeeded(label).with_verified(false),
        DeliveryOutcome::DeliveredVerified => ActionStep::succeeded(label).with_verified(true),
    };
    built.with_mechanism(StepMechanism::SemanticApi)
}

/// Records `outcome` and returns whether the chain should stop.
pub(crate) fn record_step_outcome(
    steps: &mut Vec<ActionStep>,
    label: &'static str,
    outcome: DeliveryOutcome,
    continue_after_unverified_delivery: bool,
) -> bool {
    steps.push(build_step(label, outcome));
    outcome.terminates_chain()
        && !(continue_after_unverified_delivery && outcome == DeliveryOutcome::DeliveredUnverified)
}

#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;
