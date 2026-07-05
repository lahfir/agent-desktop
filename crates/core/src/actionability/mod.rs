use crate::capability;
use crate::{
    action::{Action, Point},
    action_request::ActionRequest,
    adapter::{NativeHandle, PlatformAdapter},
    error::{AdapterError, ErrorCode},
    hit_test::HitTestResult,
    node::Rect,
    refs::RefEntry,
    state,
};
use serde_json::json;

mod check;
mod report;
mod status;

pub use check::{ActionabilityCheck, Occluder};
pub use report::ActionabilityReport;
pub use status::ActionabilityStatus;

#[cfg(test)]
pub fn check(
    entry: &RefEntry,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    check_with_stability(entry.bounds_hash, entry, request, None)
}

pub fn check_live(
    entry: &RefEntry,
    handle: &NativeHandle,
    adapter: &dyn PlatformAdapter,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    let mut observed = entry.clone();
    match adapter.get_live_element(handle) {
        Ok(live) => {
            if live_element_is_stale(&live) {
                return Err(AdapterError::new(
                    ErrorCode::StaleRef,
                    "Resolved element no longer exposes live accessibility state",
                )
                .with_suggestion(
                    "Re-run a snapshot to obtain fresh refs, then retry with the new ref (CLI: snapshot; FFI: ad_snapshot then ad_execute_by_ref with the returned snapshot_id)",
                ));
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
        }
        Err(err)
            if matches!(
                err.code,
                ErrorCode::PlatformNotSupported | ErrorCode::ActionNotSupported
            ) => {}
        Err(err) => return Err(err),
    }
    check_with_stability(
        entry.bounds_hash,
        &observed,
        request,
        Some((handle, adapter)),
    )
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
    hit_test: Option<(&NativeHandle, &dyn PlatformAdapter)>,
) -> Result<ActionabilityReport, AdapterError> {
    let mut checks = vec![
        visibility_check(entry),
        stability_check(expected_bounds_hash, entry.bounds),
        enabled_check(entry),
        action_supported_check(entry, request),
        policy_check(request),
        editable_check(entry, &request.action),
    ];
    if let Some((handle, adapter)) = hit_test {
        checks.push(receives_events_check(entry, handle, adapter, request));
    }

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
    if state::has_state(&entry.states, state::HIDDEN) {
        return fail("visible", "entry state contains hidden");
    }
    if state::has_state(&entry.states, state::OFFSCREEN) {
        return fail("visible", "entry state contains offscreen");
    }
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
    !state::has_state(states, state::DISABLED)
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

fn receives_events_check(
    entry: &RefEntry,
    handle: &NativeHandle,
    adapter: &dyn PlatformAdapter,
    request: &ActionRequest,
) -> ActionabilityCheck {
    if !request.action.requires_hit_test() {
        return pass("receives_events");
    }
    let Some(bounds) = entry.bounds else {
        return unknown("receives_events", "bounds unavailable");
    };
    let point = Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    };
    match adapter.hit_test(handle, point) {
        Ok(HitTestResult::ReachesTarget) => pass("receives_events"),
        Ok(HitTestResult::Unknown) => unknown("receives_events", "hit test result inconclusive"),
        Ok(HitTestResult::InterceptedBy {
            role,
            name,
            bounds: occluder_bounds,
        }) => occluded(role, name, occluder_bounds),
        Err(_) => unknown("receives_events", "hit test unavailable"),
    }
}

/// The occlusion-only counterpart to [`check_live`] for ref-targeted pointer
/// commands (`hover`, `drag`) that resolve a point via `point_resolve`
/// instead of dispatching an [`ActionRequest`] through `check_live` — they
/// have no `supported_action`/`editable` semantics to check, only whether the
/// resolved point actually reaches the target. Mirrors the three-way
/// [`HitTestResult`] contract: `ReachesTarget` and `Unknown` (including probe
/// errors and `not_supported`) both proceed, only `InterceptedBy` fails.
pub(crate) fn require_receives_events(
    handle: &NativeHandle,
    point: Point,
    adapter: &dyn PlatformAdapter,
) -> Result<(), AdapterError> {
    let check = match adapter.hit_test(handle, point) {
        Ok(HitTestResult::ReachesTarget | HitTestResult::Unknown) | Err(_) => return Ok(()),
        Ok(HitTestResult::InterceptedBy { role, name, bounds }) => occluded(role, name, bounds),
    };
    let report = ActionabilityReport {
        actionable: false,
        checks: vec![check],
    };
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        format!("Target is not actionable: {}", failure_reasons(&report)),
    )
    .with_details(json!(report))
    .with_suggestion(
        "Wait for the target to become actionable, refresh the snapshot, or use an explicit physical/focus command if intended.",
    ))
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
        occluder: None,
    }
}

fn fail(name: &'static str, reason: impl Into<String>) -> ActionabilityCheck {
    ActionabilityCheck {
        name,
        status: ActionabilityStatus::Fail,
        reason: Some(reason.into()),
        occluder: None,
    }
}

fn unknown(name: &'static str, reason: impl Into<String>) -> ActionabilityCheck {
    ActionabilityCheck {
        name,
        status: ActionabilityStatus::Unknown,
        reason: Some(reason.into()),
        occluder: None,
    }
}

fn occluded(
    role: Option<String>,
    name: Option<String>,
    bounds: Option<Rect>,
) -> ActionabilityCheck {
    let reason = match role.as_deref() {
        Some(role) => format!("occluded by {role}"),
        None => "occluded by another element".to_string(),
    };
    ActionabilityCheck {
        name: "receives_events",
        status: ActionabilityStatus::Fail,
        reason: Some(reason),
        occluder: Some(Occluder { role, name, bounds }),
    }
}

#[cfg(test)]
#[path = "../actionability_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../actionability_live_tests.rs"]
mod live_tests;
