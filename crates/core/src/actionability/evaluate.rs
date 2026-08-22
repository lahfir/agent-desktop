use super::{
    evidence::ActionabilityEvidence, gates, receives_events::receives_events_check,
    report::ActionabilityReport, requirements::ActionabilityRequirements,
    stability::StabilityExpectation,
};
use crate::{
    AdapterError, ErrorCode,
    action_request::ActionRequest,
    adapter::{NativeHandle, PlatformAdapter},
};
use serde_json::json;

#[cfg(test)]
use crate::refs::RefEntry;

#[cfg(test)]
pub(crate) fn check(
    entry: &RefEntry,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    use crate::{
        adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
        hit_test::HitTestResult,
    };

    struct TestHitAdapter;

    impl ObservationOps for TestHitAdapter {
        fn hit_test(
            &self,
            _handle: &NativeHandle,
            _point: crate::Point,
            _deadline: crate::Deadline,
        ) -> Result<HitTestResult, AdapterError> {
            Ok(HitTestResult::ReachesTarget)
        }
    }

    impl ActionOps for TestHitAdapter {}
    impl InputOps for TestHitAdapter {}
    impl SystemOps for TestHitAdapter {}

    let evidence = ActionabilityEvidence {
        state: crate::ElementState {
            role: entry.identity.role.clone(),
            states: entry.capabilities.states.clone(),
            value: entry.identity.value.clone(),
            enabled: Some(!crate::state::has_state(
                &entry.capabilities.states,
                crate::state::DISABLED,
            )),
            hidden: Some(crate::state::has_state(
                &entry.capabilities.states,
                crate::state::HIDDEN,
            )),
            offscreen: Some(crate::state::has_state(
                &entry.capabilities.states,
                crate::state::OFFSCREEN,
            )),
        },
        states_complete: true,
        bounds: entry.geometry.bounds,
        available_actions: entry.capabilities.available_actions.clone(),
    };
    let handle = NativeHandle::null();
    let adapter = TestHitAdapter;
    check_with_stability(
        StabilityExpectation::permissive(entry.geometry.bounds_hash),
        &evidence,
        request,
        Some((&handle, &adapter)),
        crate::Deadline::standard()?,
    )
}

pub(super) fn check_with_stability(
    stability: StabilityExpectation,
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
    hit_test: Option<(&NativeHandle, &dyn PlatformAdapter)>,
    deadline: crate::Deadline,
) -> Result<ActionabilityReport, AdapterError> {
    let requirements = ActionabilityRequirements::for_action(&request.action);
    let pointer_delivery =
        requirements.pointer_delivery(&request.action, &evidence.available_actions, request.policy);
    let mut checks = Vec::new();
    let mut verified_point = None;
    let presentation_point = evidence.bounds.and_then(center_point);
    if requirements.visible {
        checks.push(gates::visibility(evidence));
    }
    if requirements.requires_stability(pointer_delivery) {
        checks.push(gates::stability(stability, evidence.bounds));
    }
    if requirements.enabled {
        checks.push(gates::enabled(evidence));
    }
    if requirements.editable {
        checks.push(gates::editable(evidence, &request.action));
    }
    checks.push(gates::action_supported(evidence, request));
    checks.push(gates::policy(request));
    if checks
        .iter()
        .any(|check| !matches!(check.status, super::status::ActionabilityStatus::Pass))
    {
        return finish(
            evidence,
            ActionabilityReport::from_checks(
                checks,
                verified_point,
                presentation_point,
                pointer_delivery,
            ),
        );
    }
    if matches!(pointer_delivery, super::PointerDelivery::Physical) {
        match hit_test {
            Some((handle, adapter)) => {
                let (check, point) =
                    receives_events_check(evidence.bounds, handle, adapter, request, deadline)?;
                checks.push(check);
                verified_point = point;
            }
            None => checks.push(super::check_result::unknown(
                "receives_events",
                "live hit-test context unavailable",
            )),
        }
    }
    finish(
        evidence,
        ActionabilityReport::from_checks(
            checks,
            verified_point,
            presentation_point,
            pointer_delivery,
        ),
    )
}

fn center_point(bounds: crate::Rect) -> Option<crate::Point> {
    bounds.validate().ok().and_then(|bounds| {
        (bounds.width > 0.0 && bounds.height > 0.0).then_some(crate::Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        })
    })
}

fn finish(
    evidence: &ActionabilityEvidence,
    report: ActionabilityReport,
) -> Result<ActionabilityReport, AdapterError> {
    if report.actionable {
        return Ok(report);
    }
    let code = report.terminal_code().unwrap_or(ErrorCode::ActionFailed);
    let suggestion = if code == ErrorCode::ActionFailed {
        "Wait for the target to become actionable, refresh the snapshot, or use an explicit physical/focus command if intended."
    } else {
        "Waiting will not help: this element cannot satisfy the action as targeted. Target an element that advertises the action (check available_actions in a fresh snapshot) or adjust the interaction policy (e.g. pass --headed)."
    };
    let mut details = json!(report);
    if let Some(object) = details.as_object_mut()
        && let Some(bounds) = evidence.bounds
    {
        if let Some(bounds_hash) = bounds.bounds_hash() {
            object.insert("observed_bounds_hash".into(), json!(bounds_hash));
        }
    }
    Err(AdapterError::new(
        code,
        format!("Target is not actionable: {}", report.failure_reasons()),
    )
    .with_details(details)
    .with_suggestion(suggestion)
    .with_disposition(crate::DeliverySemantics::not_delivered()))
}
