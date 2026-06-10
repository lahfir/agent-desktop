use crate::{
    action::Action,
    action_request::ActionRequest,
    actionability::{bounds_are_visible, states_are_enabled},
    adapter::{NativeHandle, PlatformAdapter, optional_live_read},
    error::{AdapterError, AppError, ErrorCode},
    refs::RefEntry,
};
use serde_json::{Value, json};

#[derive(Debug)]
pub(crate) enum ElementPredicate {
    Exists,
    Enabled,
    Visible,
    Actionable(Action),
    Value(String),
}

impl ElementPredicate {
    pub(crate) fn parse(
        predicate: Option<&str>,
        value: Option<String>,
        action: Option<&str>,
    ) -> Result<Self, AppError> {
        match predicate.unwrap_or("exists") {
            "exists" => {
                reject_unused_value(value)?;
                reject_unused_action(action)?;
                Ok(Self::Exists)
            }
            "enabled" => {
                reject_unused_value(value)?;
                reject_unused_action(action)?;
                Ok(Self::Enabled)
            }
            "visible" => {
                reject_unused_value(value)?;
                reject_unused_action(action)?;
                Ok(Self::Visible)
            }
            "actionable" => {
                reject_unused_value(value)?;
                Ok(Self::Actionable(parse_actionability_action(action)?))
            }
            "value" => {
                reject_unused_action(action)?;
                value.map(Self::Value).ok_or_else(|| {
                    AppError::invalid_input_with_suggestion(
                        "--predicate value requires --value",
                        "Pass --value <expected> with --predicate value.",
                    )
                })
            }
            other => Err(AppError::invalid_input_with_suggestion(
                format!("Unknown wait predicate '{other}'"),
                "Use one of: exists, enabled, visible, actionable, value.",
            )),
        }
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Exists => "exists",
            Self::Enabled => "enabled",
            Self::Visible => "visible",
            Self::Actionable(_) => "actionable",
            Self::Value(_) => "value",
        }
    }
}

fn parse_actionability_action(action: Option<&str>) -> Result<Action, AppError> {
    match action.unwrap_or("click") {
        "click" => Ok(Action::Click),
        "type" => Ok(Action::TypeText(String::new())),
        "set-value" => Ok(Action::SetValue(String::new())),
        "clear" => Ok(Action::Clear),
        other => Err(AppError::invalid_input_with_suggestion(
            format!("Unknown actionability action '{other}'"),
            "Use one of: click, type, set-value, clear.",
        )),
    }
}

fn reject_unused_action(action: Option<&str>) -> Result<(), AppError> {
    if action.is_none() {
        return Ok(());
    }
    Err(AppError::invalid_input_with_suggestion(
        "--action is only valid with --predicate actionable",
        "Remove --action or use --predicate actionable.",
    ))
}

pub(crate) fn observe(
    entry: &RefEntry,
    handle: &NativeHandle,
    predicate: &ElementPredicate,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AdapterError> {
    match predicate {
        ElementPredicate::Exists => Ok(json!({ "exists": true })),
        ElementPredicate::Enabled => enabled(entry, handle, adapter),
        ElementPredicate::Visible => visible(entry, handle, adapter),
        ElementPredicate::Actionable(action) => actionable(entry, handle, action, adapter),
        ElementPredicate::Value(expected) => value(entry, handle, expected, adapter),
    }
}

pub(crate) fn satisfied(predicate: &ElementPredicate, observed: &Value) -> bool {
    match predicate {
        ElementPredicate::Exists => observed["exists"].as_bool() == Some(true),
        ElementPredicate::Enabled => observed["enabled"].as_bool() == Some(true),
        ElementPredicate::Visible => observed["visible"].as_bool() == Some(true),
        ElementPredicate::Actionable(_) => observed["actionable"].as_bool() == Some(true),
        ElementPredicate::Value(_) => observed["matched"].as_bool() == Some(true),
    }
}

fn reject_unused_value(value: Option<String>) -> Result<(), AppError> {
    if value.is_none() {
        return Ok(());
    }
    Err(AppError::invalid_input_with_suggestion(
        "--value is only valid with --predicate value",
        "Remove --value or use --predicate value.",
    ))
}

fn enabled(
    entry: &RefEntry,
    handle: &NativeHandle,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AdapterError> {
    let enabled = optional_live_read(adapter.get_live_state(handle))?
        .map(|state| states_are_enabled(&state.states))
        .unwrap_or_else(|| states_are_enabled(&entry.states));
    Ok(json!({ "enabled": enabled }))
}

fn visible(
    entry: &RefEntry,
    handle: &NativeHandle,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AdapterError> {
    let bounds = optional_live_read(adapter.get_element_bounds(handle))?.or(entry.bounds);
    Ok(json!({ "visible": bounds_are_visible(bounds) }))
}

fn actionable(
    entry: &RefEntry,
    handle: &NativeHandle,
    action: &Action,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AdapterError> {
    let request = actionability_request(action.clone());
    match crate::actionability::check_live(entry, handle, adapter, &request) {
        Ok(report) => Ok(json!(report)),
        Err(err) if err.code == ErrorCode::ActionFailed => match err.details {
            Some(report) => Ok(report),
            None => Ok(json!({ "actionable": false, "error": err.message })),
        },
        Err(err) => Err(err),
    }
}

/// Mirrors the base policy the real command would run with, so the preflight
/// answers "would this action succeed" rather than always "would a headless
/// click succeed" — `type` runs with the focus fallback its command uses.
fn actionability_request(action: Action) -> ActionRequest {
    match action {
        Action::TypeText(_) => ActionRequest::focus_fallback(action),
        _ => ActionRequest::headless(action),
    }
}

fn value(
    entry: &RefEntry,
    handle: &NativeHandle,
    expected: &str,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AdapterError> {
    let observed = optional_live_read(adapter.get_live_value(handle))?.or(entry.value.clone());
    let matched = observed.as_deref() == Some(expected);
    Ok(json!({
        "matched": matched,
        "value_present": observed.is_some(),
        "value_chars": observed.as_ref().map(|value| value.chars().count()),
        "expected_chars": expected.chars().count()
    }))
}
