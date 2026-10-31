use super::*;
use crate::{Action, ActionOps, HeadedRequirement, InputOps, ObservationOps, SystemOps};

struct GeometryAdapter(HeadedRequirement);

impl ActionOps for GeometryAdapter {
    fn headed_requirement(
        &self,
        _: &NativeHandle,
        _: &ActionRequest,
        _: crate::Deadline,
    ) -> Result<HeadedRequirement, AdapterError> {
        Ok(self.0)
    }
}
impl ObservationOps for GeometryAdapter {}
impl InputOps for GeometryAdapter {}
impl SystemOps for GeometryAdapter {}

fn evidence() -> ActionabilityEvidence {
    ActionabilityEvidence {
        nonspatial: false,
        state: crate::ElementState {
            role: "menuitem".into(),
            states: vec![],
            value: None,
            enabled: Some(true),
            hidden: Some(false),
            offscreen: Some(false),
        },
        states_complete: true,
        bounds: None,
        available_actions: vec!["Click".into()],
    }
}

fn check_gap(
    evidence: &ActionabilityEvidence,
    timeout: u64,
    requirement: HeadedRequirement,
) -> Result<ActionabilityReport, AdapterError> {
    let adapter = GeometryAdapter(requirement);
    check_with_stability_or_gap(
        StabilityExpectation::permissive(None),
        evidence,
        &ActionRequest::headed(Action::DoubleClick).with_timeout_ms(Some(timeout)),
        Some((&NativeHandle::null(), &adapter)),
        crate::Deadline::standard().unwrap(),
    )
}

#[test]
fn only_positive_timeout_application_pointers_preserve_a_geometry_gap() {
    let mut evidence = evidence();
    for bounds in [
        None,
        Some(crate::Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }),
    ] {
        evidence.bounds = bounds;
        let report = check_gap(
            &evidence,
            500,
            HeadedRequirement::FocusedApplicationAndCursor,
        )
        .unwrap();
        assert!(!report.actionable);
        assert!(report.verified_point.is_none());
        assert_eq!(
            report.pointer_delivery,
            super::super::PointerDelivery::Physical
        );
        assert!(check_gap(&evidence, 0, HeadedRequirement::FocusedApplicationAndCursor).is_err());
        assert!(check_gap(&evidence, 500, HeadedRequirement::FocusedWindowAndCursor).is_err());
        let adapter = GeometryAdapter(HeadedRequirement::FocusedApplicationAndCursor);
        assert!(
            check_with_stability(
                StabilityExpectation::permissive(None),
                &evidence,
                &ActionRequest::headed(Action::DoubleClick).with_timeout_ms(Some(500)),
                Some((&NativeHandle::null(), &adapter)),
                crate::Deadline::standard().unwrap()
            )
            .is_err()
        );
    }
}

#[test]
fn geometry_gap_never_masks_hidden_disabled_or_incomplete_state() {
    for variant in 0..5 {
        let mut evidence = evidence();
        match variant {
            0 => evidence.state.hidden = Some(true),
            1 => evidence.state.offscreen = Some(true),
            2 => evidence.state.enabled = Some(false),
            3 => evidence.states_complete = false,
            _ => evidence
                .state
                .states
                .push(crate::tree::state::OFFSCREEN.into()),
        }
        assert!(
            check_gap(
                &evidence,
                500,
                HeadedRequirement::FocusedApplicationAndCursor
            )
            .is_err(),
            "variant {variant}"
        );
    }
}

#[test]
fn geometry_gap_preserves_permanent_action_and_policy_refusals() {
    let adapter = GeometryAdapter(HeadedRequirement::FocusedApplicationAndCursor);
    for (request, code) in [
        (
            ActionRequest::headed(Action::SetValue("text".into())),
            ErrorCode::ActionNotSupported,
        ),
        (
            ActionRequest::headless(Action::DoubleClick),
            ErrorCode::PolicyDenied,
        ),
    ] {
        let error = check_with_stability_or_gap(
            StabilityExpectation::permissive(None),
            &evidence(),
            &request.with_timeout_ms(Some(500)),
            Some((&NativeHandle::null(), &adapter)),
            crate::Deadline::standard().unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.code, code);
    }
}
