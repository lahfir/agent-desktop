use crate::{
    action::ElementState,
    adapter::{PlatformAdapter, optional_live_read},
    commands::helpers::resolve_ref_with_context,
    context::CommandContext,
    error::AppError,
    refs::RefEntry,
};
use serde_json::{Value, json};

pub struct IsArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
    pub property: IsProperty,
}

pub enum IsProperty {
    Visible,
    Enabled,
    Checked,
    Focused,
    Expanded,
}

/// State is read live when the platform supports it, then falls back to snapshot state.
#[cfg(test)]
pub fn execute(args: IsArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    execute_with_context(args, adapter, &CommandContext::default())
}

pub fn execute_with_context(
    args: IsArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let (entry, handle) =
        resolve_ref_with_context(&args.ref_id, args.snapshot_id.as_deref(), adapter, context)?;
    let state = optional_live_read(adapter.get_live_state(handle.handle()))?
        .unwrap_or_else(|| state_from_ref_entry(&entry));

    let prop_name = match args.property {
        IsProperty::Visible => "visible",
        IsProperty::Enabled => "enabled",
        IsProperty::Checked => "checked",
        IsProperty::Focused => "focused",
        IsProperty::Expanded => "expanded",
    };

    let applicable = is_applicable(&args.property, &entry, &state);

    let result = match args.property {
        IsProperty::Visible => !has_state(&state, "hidden"),
        IsProperty::Enabled => !has_state(&state, "disabled"),
        IsProperty::Checked => has_state(&state, "checked"),
        IsProperty::Focused => has_state(&state, "focused"),
        IsProperty::Expanded => has_state(&state, "expanded"),
    };

    Ok(
        json!({ "property": prop_name, "ref": args.ref_id, "result": result, "applicable": applicable }),
    )
}

fn state_from_ref_entry(entry: &RefEntry) -> ElementState {
    ElementState {
        role: entry.role.clone(),
        states: entry.states.clone(),
        value: entry.value.clone(),
    }
}

fn has_state(state: &ElementState, name: &str) -> bool {
    state.states.iter().any(|s| s == name)
}

fn is_applicable(property: &IsProperty, entry: &RefEntry, state: &ElementState) -> bool {
    match property {
        IsProperty::Visible | IsProperty::Enabled | IsProperty::Focused => true,
        IsProperty::Checked => {
            crate::roles::is_toggleable_role(&entry.role)
                || has_state(state, "checked")
                || has_available_action(entry, "Toggle")
                || has_available_action(entry, "Check")
                || has_available_action(entry, "Uncheck")
        }
        IsProperty::Expanded => {
            crate::roles::is_expandable_role(&entry.role)
                || has_state(state, "expanded")
                || has_available_action(entry, "Expand")
                || has_available_action(entry, "Collapse")
        }
    }
}

fn has_available_action(entry: &RefEntry, action: &str) -> bool {
    entry.available_actions.iter().any(|a| a == action)
}

#[cfg(test)]
#[path = "is_check_tests.rs"]
mod tests;
