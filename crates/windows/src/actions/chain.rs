//! Activation-chain engine for UIA pattern dispatch.
//!
//! Rung closures return a delivery outcome or `Err`. A genuine `Err` aborts
//! the chain via `?` and never falls through. Only a clean not-delivered
//! outcome advances to the next rung. Policy-disallowed rungs are skipped
//! with no step recorded (the hook later physical rungs will use). Mechanism
//! is always `semantic_api` for every rung this adapter ships.

use agent_desktop_core::{
    ActionStep, ActionStepOutcome, AdapterError, Deadline, DeliverySemantics, ErrorCode,
    InteractionPolicy, StepMechanism,
};

use crate::system::permissions::ensure_budget;

/// Per-rung delivery judgment recorded into an [`ActionStep`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeliveryOutcome {
    NotDelivered,
    /// Already-at-target arms (toggle / disclosure) construct this outcome.
    #[allow(dead_code)]
    SatisfiedNoDelivery,
    DeliveredUnverified,
    DeliveredVerified,
}

impl DeliveryOutcome {
    pub(crate) fn from_delivery(delivered: bool, verified: bool) -> Self {
        match (delivered, verified) {
            (false, _) => Self::NotDelivered,
            (true, false) => Self::DeliveredUnverified,
            (true, true) => Self::DeliveredVerified,
        }
    }

    pub(crate) fn was_delivered(self) -> bool {
        matches!(self, Self::DeliveredUnverified | Self::DeliveredVerified)
    }

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
        let outcome = (rung.run)()?;
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
    let delivered = steps
        .iter()
        .any(|step| matches!(step.outcome, ActionStepOutcome::Succeeded));
    if delivered {
        DeliverySemantics::delivered_unverified()
    } else {
        DeliverySemantics::not_delivered()
    }
}

pub(crate) fn build_step(label: &'static str, outcome: DeliveryOutcome) -> ActionStep {
    let mut built = match outcome {
        DeliveryOutcome::NotDelivered => ActionStep::skipped(label),
        DeliveryOutcome::SatisfiedNoDelivery => ActionStep::skipped(label).with_verified(true),
        DeliveryOutcome::DeliveredUnverified | DeliveryOutcome::DeliveredVerified => {
            ActionStep::succeeded(label)
        }
    };
    built = built.with_mechanism(StepMechanism::SemanticApi);
    if outcome.was_delivered() {
        built = built.with_verified(outcome.was_verified());
    }
    built
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
        && !(continue_after_unverified_delivery
            && outcome == DeliveryOutcome::DeliveredUnverified)
}

#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;
