use super::{
    evaluate::check_with_stability, evidence::ActionabilityEvidence, report::ActionabilityReport,
    stability::StabilityExpectation,
};
use crate::{
    AdapterError, DeliverySemantics, ErrorCode, IdentityMatch, LocatorField,
    action_request::ActionRequest,
    adapter::{LiveElement, NativeHandle, PlatformAdapter},
    ref_identity::{has_meaningful_identity, identity_match},
    refs::RefEntry,
};

#[cfg(test)]
pub(crate) fn check_live(
    entry: &RefEntry,
    handle: &NativeHandle,
    adapter: &dyn PlatformAdapter,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    let deadline = crate::Deadline::standard()?;
    check_live_with_stability(
        entry,
        handle,
        adapter,
        request,
        StabilityExpectation::permissive(entry.geometry.bounds_hash),
        deadline,
    )
}

pub(crate) fn check_live_with_stability(
    entry: &RefEntry,
    handle: &NativeHandle,
    adapter: &dyn PlatformAdapter,
    request: &ActionRequest,
    stability: StabilityExpectation,
    deadline: crate::Deadline,
) -> Result<ActionabilityReport, AdapterError> {
    let evidence = observe(entry, adapter.get_live_element(handle, deadline))?;
    check_with_stability(
        stability,
        &evidence,
        request,
        Some((handle, adapter)),
        deadline,
    )
}

fn observe(
    entry: &RefEntry,
    live: Result<LiveElement, AdapterError>,
) -> Result<ActionabilityEvidence, AdapterError> {
    let live = match live {
        Ok(live) => live,
        Err(err)
            if matches!(
                err.code,
                ErrorCode::PlatformNotSupported | ErrorCode::ActionNotSupported
            ) =>
        {
            return Err(AdapterError::new(
                ErrorCode::ActionNotSupported,
                "Live element evidence is required for actionability checks",
            )
            .with_details(serde_json::json!({ "source_code": err.code.as_str() }))
            .with_disposition(DeliverySemantics::not_delivered()));
        }
        Err(err) => return Err(err),
    };
    validate_live_identity(entry, &live)?;
    Ok(ActionabilityEvidence {
        state: live.state,
        states_complete: live.states_complete,
        bounds: live.bounds,
        available_actions: live.available_actions,
    })
}

fn validate_live_identity(entry: &RefEntry, live: &LiveElement) -> Result<(), AdapterError> {
    if live.state.role == "unknown" || live.state.role != entry.identity.role {
        return Err(stale_live_element(
            "Resolved element changed role before dispatch",
        ));
    }
    if !has_meaningful_identity(entry) {
        return Ok(());
    }
    let value = live
        .state
        .value
        .clone()
        .map(LocatorField::Known)
        .unwrap_or(LocatorField::Absent);
    match identity_match(
        entry,
        &live.identity.name,
        &value,
        &live.identity.description,
        &live.identity.identifiers,
    ) {
        IdentityMatch::Match => Ok(()),
        IdentityMatch::NoMatch => Err(stale_live_element(
            "Resolved element changed identity before dispatch",
        )),
        IdentityMatch::Unknown => Err(AdapterError::app_unresponsive(
            entry
                .source
                .source_app
                .as_deref()
                .unwrap_or("target application"),
        )
        .with_details(serde_json::json!({
            "kind": "live_identity_evidence",
            "complete": false,
            "retryable": true,
        }))),
    }
}

fn stale_live_element(message: &'static str) -> AdapterError {
    AdapterError::new(ErrorCode::StaleRef, message)
        .with_suggestion(
            "Re-run a snapshot to obtain fresh refs, then retry with the new ref (CLI: snapshot; FFI: ad_snapshot then ad_execute_by_ref with the returned snapshot_id)",
        )
        .with_details(serde_json::json!({ "retryable": true }))
        .with_disposition(DeliverySemantics::not_delivered())
}
