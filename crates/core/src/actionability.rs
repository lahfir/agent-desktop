use crate::{
    action::{Action, ActionRequest},
    adapter::{NativeHandle, PlatformAdapter},
    error::{AdapterError, ErrorCode},
    refs::RefEntry,
};
use serde::Serialize;

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

pub fn check(
    entry: &RefEntry,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    let mut checks = vec![pass("unique_target")];
    checks.push(visibility_check(entry, &request.action));
    checks.push(enabled_check(entry));
    checks.push(action_supported_check(entry, request));
    checks.push(policy_check(request));
    checks.push(editable_check(entry, &request.action));

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
    .with_suggestion("Wait for the target to become actionable, refresh the snapshot, or use an explicit physical/focus command if intended."))
}

pub fn check_live(
    entry: &RefEntry,
    handle: &NativeHandle,
    adapter: &dyn PlatformAdapter,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    let mut observed = entry.clone();
    if let Some(state) = adapter.get_live_state(handle).ok().flatten() {
        observed.role = state.role;
        observed.states = state.states;
        observed.value = state.value.or(observed.value);
    }
    if let Some(bounds) = adapter.get_element_bounds(handle).ok().flatten() {
        observed.bounds = Some(bounds);
    }
    check(&observed, request)
}

fn visibility_check(entry: &RefEntry, action: &Action) -> ActionabilityCheck {
    let Some(bounds) = entry.bounds else {
        return unknown("visible", "bounds unavailable");
    };
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return fail("visible", "bounds are zero-sized");
    }
    if matches!(action, Action::Scroll(_, _)) {
        return pass("visible");
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
    if requires_physical_policy(&request.action) {
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
    let expected = action_capabilities(&request.action).join(" or ");
    fail("supported_action", format!("{expected} is not available"))
}

fn policy_check(request: &ActionRequest) -> ActionabilityCheck {
    if requires_cursor_policy(&request.action) && !request.policy.allow_cursor_move {
        return fail(
            "policy",
            "action requires cursor movement but policy denies it",
        );
    }
    if requires_focus_policy(&request.action) && !request.policy.allow_focus_steal {
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

fn action_capabilities(action: &Action) -> &'static [&'static str] {
    match action {
        Action::Click | Action::DoubleClick | Action::TripleClick => &["Click"],
        Action::RightClick => &["RightClick", "Click"],
        Action::SetValue(_) | Action::Clear => &["SetValue"],
        Action::SetFocus => &["SetFocus"],
        Action::Expand => &["Expand"],
        Action::Collapse => &["Collapse"],
        Action::Select(_) => &["Select", "Click"],
        Action::Toggle => &["Toggle", "Click"],
        Action::Check | Action::Uncheck => &["Toggle", "Click"],
        Action::Scroll(_, _) | Action::ScrollTo => &["ScrollTo"],
        Action::PressKey(_) => &["PressKey"],
        Action::KeyDown(_) => &["KeyDown"],
        Action::KeyUp(_) => &["KeyUp"],
        Action::TypeText(_) => &["TypeText", "SetValue"],
        Action::Hover => &["Hover"],
        Action::Drag(_) => &["Drag"],
    }
}

fn supported_by_available_actions(action: &Action, available_actions: &[String]) -> bool {
    action_capabilities(action)
        .iter()
        .any(|expected| available_actions.iter().any(|action| action == expected))
}

fn may_use_fallback(action: &Action, request: &ActionRequest) -> bool {
    matches!(action, Action::TypeText(_) | Action::PressKey(_)) && request.policy.allow_focus_steal
}

fn requires_physical_policy(action: &Action) -> bool {
    matches!(action, Action::Hover | Action::Drag(_))
}

fn requires_cursor_policy(action: &Action) -> bool {
    matches!(action, Action::Hover | Action::Drag(_))
}

fn requires_focus_policy(action: &Action) -> bool {
    matches!(action, Action::TypeText(_) | Action::PressKey(_))
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
