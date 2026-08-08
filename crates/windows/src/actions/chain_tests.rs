use super::{
    ChainDef, ChainRung, DeliveryOutcome, build_step, execute_chain, exhaustion_disposition,
    record_step_outcome, rung_allowed,
};
use agent_desktop_core::{
    ActionStepOutcome, AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy,
    StepMechanism,
};
use std::cell::Cell;

fn deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

#[test]
fn delivery_and_verification_are_independent() {
    assert_eq!(
        DeliveryOutcome::from_delivery(false, true),
        DeliveryOutcome::NotDelivered
    );
    assert_eq!(
        DeliveryOutcome::from_delivery(true, false),
        DeliveryOutcome::DeliveredUnverified
    );
    assert_eq!(
        DeliveryOutcome::from_delivery(true, true),
        DeliveryOutcome::DeliveredVerified
    );
    assert!(DeliveryOutcome::SatisfiedNoDelivery.terminates_chain());
    assert!(!DeliveryOutcome::SatisfiedNoDelivery.was_delivered());
    assert!(DeliveryOutcome::SatisfiedNoDelivery.was_verified());
}

#[test]
fn build_step_tags_mechanism_and_verified_on_success() {
    let built = build_step("InvokePattern.Invoke", DeliveryOutcome::DeliveredVerified);
    assert_eq!(built.mechanism(), Some(StepMechanism::SemanticApi));
    assert_eq!(built.verified(), Some(true));
}

#[test]
fn build_step_skipped_does_not_tag_verified() {
    let built = build_step("InvokePattern.Invoke", DeliveryOutcome::NotDelivered);
    assert_eq!(built.mechanism(), Some(StepMechanism::SemanticApi));
    assert!(built.verified().is_none());
}

#[test]
fn build_step_marks_delivered_unverified_explicitly() {
    let built = build_step("InvokePattern.Invoke", DeliveryOutcome::DeliveredUnverified);
    assert_eq!(built.mechanism(), Some(StepMechanism::SemanticApi));
    assert_eq!(built.verified(), Some(false));
}

#[test]
fn build_step_leaves_verified_absent_when_observation_withheld() {
    let built = build_step(
        "ValuePattern.SetValue",
        DeliveryOutcome::from_observation(None),
    );
    assert_eq!(built.mechanism(), Some(StepMechanism::SemanticApi));
    assert!(matches!(built.outcome, ActionStepOutcome::Succeeded));
    assert!(built.verified().is_none());
    assert!(DeliveryOutcome::DeliveredUnobserved.was_delivered());
    assert!(!DeliveryOutcome::DeliveredUnobserved.was_verified());
}

#[test]
fn satisfied_without_delivery_stops_fallback_and_is_skipped_verified() {
    let mut steps = Vec::new();
    assert!(record_step_outcome(
        &mut steps,
        "AlreadyInState",
        DeliveryOutcome::SatisfiedNoDelivery,
        false,
    ));
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Skipped));
    assert_eq!(steps[0].verified(), Some(true));
}

#[test]
fn not_delivered_falls_through_with_skipped_step() {
    let first = Cell::new(0u8);
    let second = Cell::new(0u8);
    let mut first_run = || {
        first.set(first.get() + 1);
        Ok(DeliveryOutcome::NotDelivered)
    };
    let mut second_run = || {
        second.set(second.get() + 1);
        Ok(DeliveryOutcome::DeliveredUnverified)
    };
    let def = ChainDef {
        suggestion: "retry",
        continue_after_unverified_delivery: false,
    };
    let steps = execute_chain(
        deadline(),
        &def,
        InteractionPolicy::headless(),
        &mut [
            ChainRung {
                label: "InvokePattern.Invoke",
                requires_headed: false,
                run: &mut first_run,
            },
            ChainRung {
                label: "LegacyIAccessible.DoDefaultAction",
                requires_headed: false,
                run: &mut second_run,
            },
        ],
    )
    .expect("second rung delivers");
    assert_eq!(first.get(), 1);
    assert_eq!(second.get(), 1);
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Skipped));
    assert!(matches!(steps[1].outcome, ActionStepOutcome::Succeeded));
}

#[test]
fn genuine_err_aborts_with_no_later_rung() {
    let later = Cell::new(0u8);
    let mut first_run = || {
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "classified write failure",
        ))
    };
    let mut second_run = || {
        later.set(later.get() + 1);
        Ok(DeliveryOutcome::DeliveredUnverified)
    };
    let def = ChainDef {
        suggestion: "retry",
        continue_after_unverified_delivery: false,
    };
    let error = execute_chain(
        deadline(),
        &def,
        InteractionPolicy::headless(),
        &mut [
            ChainRung {
                label: "InvokePattern.Invoke",
                requires_headed: false,
                run: &mut first_run,
            },
            ChainRung {
                label: "LegacyIAccessible.DoDefaultAction",
                requires_headed: false,
                run: &mut second_run,
            },
        ],
    )
    .expect_err("Err aborts");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(later.get(), 0);
}

#[test]
fn policy_disallowed_rung_is_silently_skipped() {
    let physical = Cell::new(0u8);
    let semantic = Cell::new(0u8);
    let mut physical_run = || {
        physical.set(physical.get() + 1);
        Ok(DeliveryOutcome::DeliveredUnverified)
    };
    let mut semantic_run = || {
        semantic.set(semantic.get() + 1);
        Ok(DeliveryOutcome::DeliveredUnverified)
    };
    let def = ChainDef {
        suggestion: "retry",
        continue_after_unverified_delivery: false,
    };
    let steps = execute_chain(
        deadline(),
        &def,
        InteractionPolicy::headless(),
        &mut [
            ChainRung {
                label: "PhysicalClick",
                requires_headed: true,
                run: &mut physical_run,
            },
            ChainRung {
                label: "InvokePattern.Invoke",
                requires_headed: false,
                run: &mut semantic_run,
            },
        ],
    )
    .expect("semantic rung delivers");
    assert_eq!(physical.get(), 0);
    assert_eq!(semantic.get(), 1);
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].label(), "InvokePattern.Invoke");
}

#[test]
fn headed_policy_allows_physical_rung() {
    let rung = ChainRung {
        label: "PhysicalClick",
        requires_headed: true,
        run: &mut || Ok(DeliveryOutcome::NotDelivered),
    };
    assert!(!rung_allowed(&rung, InteractionPolicy::headless()));
    assert!(rung_allowed(&rung, InteractionPolicy::headed()));
}

#[test]
fn continue_after_unverified_delivery_runs_next_rung() {
    let mut steps = Vec::new();
    assert!(!record_step_outcome(
        &mut steps,
        "ValuePattern.SetValue",
        DeliveryOutcome::DeliveredUnverified,
        true,
    ));
    assert!(!record_step_outcome(
        &mut steps,
        "RangeValuePattern.SetValue",
        DeliveryOutcome::NotDelivered,
        true,
    ));
    assert_eq!(steps.len(), 2);
}

#[test]
fn non_idempotent_chain_stops_after_unverified_delivery() {
    let mut steps = Vec::new();
    assert!(record_step_outcome(
        &mut steps,
        "InvokePattern.Invoke",
        DeliveryOutcome::DeliveredUnverified,
        false,
    ));
}

#[test]
fn exhaustion_after_unverified_delivery_reports_delivered_unverified() {
    let mut steps = Vec::new();
    assert!(!record_step_outcome(
        &mut steps,
        "ValuePattern.SetValue",
        DeliveryOutcome::DeliveredUnverified,
        true,
    ));
    assert!(!record_step_outcome(
        &mut steps,
        "RangeValuePattern.SetValue",
        DeliveryOutcome::NotDelivered,
        true,
    ));
    assert_eq!(
        exhaustion_disposition(&steps),
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn exhaustion_without_any_delivery_reports_not_delivered() {
    let mut steps = Vec::new();
    assert!(!record_step_outcome(
        &mut steps,
        "InvokePattern.Invoke",
        DeliveryOutcome::NotDelivered,
        false,
    ));
    assert_eq!(
        exhaustion_disposition(&steps),
        DeliverySemantics::not_delivered()
    );
    assert_eq!(
        exhaustion_disposition(&[]),
        DeliverySemantics::not_delivered()
    );
}

#[test]
fn exhausted_chain_carries_suggestion_and_disposition() {
    let mut first = || Ok(DeliveryOutcome::NotDelivered);
    let mut second = || Ok(DeliveryOutcome::NotDelivered);
    let def = ChainDef {
        suggestion: "target an Invoke-capable control",
        continue_after_unverified_delivery: false,
    };
    let error = execute_chain(
        deadline(),
        &def,
        InteractionPolicy::headless(),
        &mut [
            ChainRung {
                label: "InvokePattern.Invoke",
                requires_headed: false,
                run: &mut first,
            },
            ChainRung {
                label: "LegacyIAccessible.DoDefaultAction",
                requires_headed: false,
                run: &mut second,
            },
        ],
    )
    .expect_err("exhausted");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        agent_desktop_core::DeliveryDisposition::NotDelivered
    );
    assert_eq!(
        error.suggestion.as_deref(),
        Some("target an Invoke-capable control")
    );
}
