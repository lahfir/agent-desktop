use crate::{
    actionability::{bounds_are_visible, states_are_enabled},
    adapter::{NativeHandle, PlatformAdapter, optional_live_read},
    error::{AdapterError, AppError, ErrorCode},
    refs::RefEntry,
};
use serde_json::{Value, json};

pub(crate) enum ElementPredicate {
    Exists,
    Enabled,
    Visible,
    Actionable,
    Value(String),
}

impl ElementPredicate {
    pub(crate) fn parse(predicate: Option<&str>, value: Option<String>) -> Result<Self, AppError> {
        match predicate.unwrap_or("exists") {
            "exists" => {
                reject_unused_value(value)?;
                Ok(Self::Exists)
            }
            "enabled" => {
                reject_unused_value(value)?;
                Ok(Self::Enabled)
            }
            "visible" => {
                reject_unused_value(value)?;
                Ok(Self::Visible)
            }
            "actionable" => {
                reject_unused_value(value)?;
                Ok(Self::Actionable)
            }
            "value" => value.map(Self::Value).ok_or_else(|| {
                AppError::invalid_input_with_suggestion(
                    "--predicate value requires --value",
                    "Pass --value <expected> with --predicate value.",
                )
            }),
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
            Self::Actionable => "actionable",
            Self::Value(_) => "value",
        }
    }
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
        ElementPredicate::Actionable => actionable(entry, handle, adapter),
        ElementPredicate::Value(expected) => value(entry, handle, expected, adapter),
    }
}

pub(crate) fn satisfied(predicate: &ElementPredicate, observed: &Value) -> bool {
    match predicate {
        ElementPredicate::Exists => observed["exists"].as_bool() == Some(true),
        ElementPredicate::Enabled => observed["enabled"].as_bool() == Some(true),
        ElementPredicate::Visible => observed["visible"].as_bool() == Some(true),
        ElementPredicate::Actionable => observed["actionable"].as_bool() == Some(true),
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
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AdapterError> {
    let request = crate::action_request::ActionRequest::headless(crate::action::Action::Click);
    match crate::actionability::check_live(entry, handle, adapter, &request) {
        Ok(report) => Ok(json!(report)),
        Err(err) if err.code == ErrorCode::ActionFailed => {
            Ok(json!({ "actionable": false, "error": err.message }))
        }
        Err(err) => Err(err),
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
