use crate::{
    action::{Action, ActionRequest},
    adapter::{NativeHandle, PlatformAdapter},
    error::{AdapterError, ErrorCode},
    refs::RefEntry,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionabilityStatus {
    Pass,
    Fail,
    Unknown,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ActionabilityCheck {
    pub name: &'static str,
    pub status: ActionabilityStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ActionabilityReport {
    pub actionable: bool,
    pub checks: Vec<ActionabilityCheck>,
}

pub(crate) fn check(
    entry: &RefEntry,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    let checks = vec![
        visibility_check(entry),
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
    .with_suggestion("Wait for the target to become actionable, refresh the snapshot, or use an explicit physical/focus command if intended."))
}

pub fn check_live(
    entry: &RefEntry,
    handle: &NativeHandle,
    adapter: &dyn PlatformAdapter,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    let mut observed = entry.clone();
    let live = adapter.get_live_element(handle)?;
    if let Some(state) = live.state {
        observed.role = state.role;
        observed.states = state.states;
        observed.value = state.value.or(observed.value);
    }
    if let Some(bounds) = live.bounds {
        observed.bounds = Some(bounds);
    }
    if let Some(actions) = live.available_actions
        && !actions.is_empty()
    {
        observed.available_actions = actions;
    }
    check(&observed, request)
}

fn visibility_check(entry: &RefEntry) -> ActionabilityCheck {
    let Some(bounds) = entry.bounds else {
        return unknown("visible", "bounds unavailable");
    };
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return fail("visible", "bounds are zero-sized");
    }
    pass("visible")
}

fn enabled_check(entry: &RefEntry) -> ActionabilityCheck {
    if entry.states.iter().any(|state| state == "disabled") {
        return fail("enabled", "entry state contains disabled");
    }
    pass("enabled")
}

fn action_supported_check(entry: &RefEntry, request: &ActionRequest) -> ActionabilityCheck {
    if request.action.requires_cursor_policy() {
        return pass("supported_action");
    }
    if supported_by_available_actions(&request.action, &entry.available_actions) {
        return pass("supported_action");
    }
    if may_use_fallback(&request.action, request) {
        return unknown(
            "supported_action",
            "semantic action unavailable but fallback policy allows attempt",
        );
    }
    let expected = request.action.semantic_capabilities().join(" or ");
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
    if entry
        .available_actions
        .iter()
        .any(|action| action == "SetValue")
    {
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

fn supported_by_available_actions(action: &Action, available_actions: &[String]) -> bool {
    action
        .semantic_capabilities()
        .iter()
        .any(|expected| available_actions.iter().any(|action| action == expected))
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
#[path = "actionability_tests.rs"]
mod tests;
