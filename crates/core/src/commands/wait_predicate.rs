use crate::{
    adapter::{NativeHandle, PlatformAdapter},
    error::AppError,
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

pub(crate) fn matches(
    entry: &RefEntry,
    handle: &NativeHandle,
    predicate: &ElementPredicate,
    adapter: &dyn PlatformAdapter,
) -> Option<Value> {
    match predicate {
        ElementPredicate::Exists => Some(json!({ "exists": true })),
        ElementPredicate::Enabled => enabled(entry, handle, adapter),
        ElementPredicate::Visible => visible(entry, handle, adapter),
        ElementPredicate::Actionable => actionable(entry),
        ElementPredicate::Value(expected) => value(entry, handle, expected, adapter),
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
) -> Option<Value> {
    let enabled = adapter
        .get_live_state(handle)
        .ok()
        .flatten()
        .map(|state| !state.states.iter().any(|item| item == "disabled"))
        .unwrap_or_else(|| !entry.states.iter().any(|item| item == "disabled"));
    enabled.then(|| json!({ "enabled": true }))
}

fn visible(
    entry: &RefEntry,
    handle: &NativeHandle,
    adapter: &dyn PlatformAdapter,
) -> Option<Value> {
    let bounds = adapter
        .get_element_bounds(handle)
        .ok()
        .flatten()
        .or(entry.bounds);
    let visible = bounds
        .map(|bounds| bounds.width > 0.0 && bounds.height > 0.0)
        .unwrap_or(false);
    visible.then(|| json!({ "visible": true }))
}

fn actionable(entry: &RefEntry) -> Option<Value> {
    let request = crate::action::ActionRequest::headless(crate::action::Action::Click);
    crate::actionability::check(entry, &request)
        .ok()
        .map(|report| json!(report))
}

fn value(
    entry: &RefEntry,
    handle: &NativeHandle,
    expected: &str,
    adapter: &dyn PlatformAdapter,
) -> Option<Value> {
    let observed = adapter
        .get_live_value(handle)
        .ok()
        .flatten()
        .or(entry.value.clone());
    (observed.as_deref() == Some(expected)).then(|| json!({ "value": observed }))
}
