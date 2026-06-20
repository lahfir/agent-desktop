use crate::capability;
use crate::{
    action::Action,
    action_request::ActionRequest,
    adapter::{NativeHandle, PlatformAdapter},
    error::{AdapterError, ErrorCode},
    node::Rect,
    refs::RefEntry,
};
use serde_json::json;

mod check;
mod report;
mod status;

pub use check::ActionabilityCheck;
pub use report::ActionabilityReport;
pub use status::ActionabilityStatus;

#[cfg(test)]
pub fn check(
    entry: &RefEntry,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    check_with_stability(entry.bounds_hash, entry, request)
}

pub fn check_live(
    entry: &RefEntry,
    handle: &NativeHandle,
    adapter: &dyn PlatformAdapter,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    let mut observed = entry.clone();
    let live = match adapter.get_live_element(handle) {
        Ok(live) => live,
        Err(err)
            if matches!(
                err.code,
                ErrorCode::PlatformNotSupported | ErrorCode::ActionNotSupported
            ) =>
        {
            return check_with_stability(entry.bounds_hash, &observed, request);
        }
        Err(err) => return Err(err),
    };
    if live_element_is_stale(&live) {
        return Err(AdapterError::new(
            ErrorCode::StaleRef,
            "Resolved element no longer exposes live accessibility state",
        )
        .with_suggestion("Run 'snapshot' again and retry with the refreshed ref"));
    }
    if let Some(state) = live.state {
        observed.role = state.role;
        observed.states = state.states;
        observed.value = state.value.or(observed.value);
    }
    observed.bounds = live.bounds;
    if let Some(actions) = live.available_actions
        && !actions.is_empty()
    {
        observed.available_actions = actions;
    }
    check_with_stability(entry.bounds_hash, &observed, request)
}

fn live_element_is_stale(live: &crate::adapter::LiveElement) -> bool {
    let role_unknown = live
        .state
        .as_ref()
        .is_none_or(|state| state.role == "unknown");
    let actions_empty = live.available_actions.as_ref().is_none_or(Vec::is_empty);
    role_unknown && live.bounds.is_none() && actions_empty
}

fn check_with_stability(
    expected_bounds_hash: Option<u64>,
    entry: &RefEntry,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    let checks = vec![
        visibility_check(entry),
        stability_check(expected_bounds_hash, entry.bounds),
        enabled_check(entry),
        action_supported_check(entry, request),
        policy_check(request),
        editable_check(entry, &request.action),
    ];

    let actionable = checks
        .iter()
        .all(|check| !matches!(check.status, ActionabilityStatus::Fail));
    let report = ActionabilityReport { actionable, checks };
    if report.actionable {
        return Ok(report);
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        format!("Target is not actionable: {}", failure_reasons(&report)),
    )
    .with_details(json!(report))
    .with_suggestion(
        "Wait for the target to become actionable, refresh the snapshot, or use an explicit physical/focus command if intended.",
    ))
}

fn visibility_check(entry: &RefEntry) -> ActionabilityCheck {
    let Some(bounds) = entry.bounds else {
        return unknown("visible", "bounds unavailable");
    };
    if !bounds_are_visible(Some(bounds)) {
        return fail("visible", "bounds are zero-sized");
    }
    pass("visible")
}

fn stability_check(expected_bounds_hash: Option<u64>, bounds: Option<Rect>) -> ActionabilityCheck {
    let Some(expected) = expected_bounds_hash else {
        return unknown("stable", "snapshot bounds hash unavailable");
    };
    let Some(bounds) = bounds else {
        return unknown("stable", "live bounds unavailable");
    };
    if bounds.bounds_hash() != expected {
        return unknown("stable", "bounds changed since snapshot");
    }
    pass("stable")
}

fn enabled_check(entry: &RefEntry) -> ActionabilityCheck {
    if !states_are_enabled(&entry.states) {
        return fail("enabled", "entry state contains disabled");
    }
    pass("enabled")
}

pub fn states_are_enabled(states: &[String]) -> bool {
    !states.iter().any(|state| state == "disabled")
}

pub fn bounds_are_visible(bounds: Option<Rect>) -> bool {
    bounds.is_some_and(|bounds| bounds.width > 0.0 && bounds.height > 0.0)
}

fn action_supported_check(entry: &RefEntry, request: &ActionRequest) -> ActionabilityCheck {
    if request.action.requires_cursor_policy() {
        return pass("supported_action");
    }
    if capability::contains_any(
        &entry.available_actions,
        capability::for_action(&request.action),
    ) {
        return pass("supported_action");
    }
    if may_use_fallback(&request.action, request) {
        return unknown(
            "supported_action",
            "semantic action unavailable but fallback policy allows attempt",
        );
    }
    let expected = capability::for_action(&request.action).join(" or ");
    fail("supported_action", format!("{expected} is not available"))
}

fn policy_check(request: &ActionRequest) -> ActionabilityCheck {
    if request.action.requires_cursor_policy() && !request.policy.allow_cursor_move {
        return fail(
            "policy",
            "action requires cursor movement but policy denies it",
        );
    }
    if request.action.may_use_focus_fallback() && !request.policy.allow_focus_steal {
        return fail("policy", "action requires focus but policy denies it");
    }
    pass("policy")
}

fn editable_check(entry: &RefEntry, action: &Action) -> ActionabilityCheck {
    if !matches!(
        action,
        Action::SetValue(_) | Action::TypeText(_) | Action::Clear
    ) {
        return pass("editable");
    }
    if entry.role == "textfield" || entry.role == "combobox" {
        return pass("editable");
    }
    if capability::contains(&entry.available_actions, capability::SET_VALUE) {
        return pass("editable");
    }
    fail("editable", format!("role {} is not editable", entry.role))
}

fn failure_reasons(report: &ActionabilityReport) -> String {
    report
        .checks
        .iter()
        .filter(|check| matches!(check.status, ActionabilityStatus::Fail))
        .map(|check| {
            let reason = check.reason.as_deref().unwrap_or("failed");
            format!("{} ({reason})", check.name)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn may_use_fallback(action: &Action, request: &ActionRequest) -> bool {
    action.may_use_focus_fallback() && request.policy.allow_focus_steal
}

fn pass(name: &'static str) -> ActionabilityCheck {
    ActionabilityCheck {
        name,
        status: ActionabilityStatus::Pass,
        reason: None,
    }
}

fn fail(name: &'static str, reason: impl Into<String>) -> ActionabilityCheck {
    ActionabilityCheck {
        name,
        status: ActionabilityStatus::Fail,
        reason: Some(reason.into()),
    }
}

fn unknown(name: &'static str, reason: impl Into<String>) -> ActionabilityCheck {
    ActionabilityCheck {
        name,
        status: ActionabilityStatus::Unknown,
        reason: Some(reason.into()),
    }
}

#[cfg(test)]
#[path = "../actionability_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../actionability_live_tests.rs"]
mod live_tests;
