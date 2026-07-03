use crate::{
    adapter::{PlatformAdapter, optional_live_read},
    commands::helpers::resolve_ref_with_context,
    context::CommandContext,
    element_state::ElementState,
    error::AppError,
    refs::RefEntry,
    state::{self, CHECKED, DISABLED, EXPANDED, FOCUSED, VisibilityEvidence},
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

pub fn execute(
    args: IsArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let (entry, handle) =
        resolve_ref_with_context(&args.ref_id, args.snapshot_id.as_deref(), adapter, context)?;

    let prop_name = match args.property {
        IsProperty::Visible => "visible",
        IsProperty::Enabled => "enabled",
        IsProperty::Checked => "checked",
        IsProperty::Focused => "focused",
        IsProperty::Expanded => "expanded",
    };

    let live_state = optional_live_read(adapter.get_live_state(handle.handle()))?;
    let state = live_state
        .clone()
        .unwrap_or_else(|| state_from_ref_entry(&entry));
    let states_from_live = live_state.is_some();
    let live_bounds = optional_live_read(adapter.get_element_bounds(handle.handle()))?;
    let visibility = VisibilityEvidence {
        bounds: live_bounds.or(entry.bounds),
        states: state.states.clone(),
        bounds_from_live: live_bounds.is_some(),
        states_from_live,
    };

    let applicable = match args.property {
        IsProperty::Visible => visibility.applicable(),
        IsProperty::Enabled | IsProperty::Focused => true,
        IsProperty::Checked => {
            crate::roles::is_toggleable_role(&entry.role)
                || state::has_state(&state.states, CHECKED)
                || crate::capability::contains_any(
                    &entry.available_actions,
                    crate::capability::CHECKED_APPLICABILITY,
                )
        }
        IsProperty::Expanded => {
            crate::roles::is_expandable_role(&entry.role)
                || state::has_state(&state.states, EXPANDED)
                || crate::capability::contains_any(
                    &entry.available_actions,
                    crate::capability::EXPANDED_APPLICABILITY,
                )
        }
    };

    let result = match args.property {
        IsProperty::Visible => visibility.result(),
        IsProperty::Enabled => !state::has_state(&state.states, DISABLED),
        IsProperty::Checked => state::has_state(&state.states, CHECKED),
        IsProperty::Focused => state::has_state(&state.states, FOCUSED),
        IsProperty::Expanded => state::has_state(&state.states, EXPANDED),
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

#[cfg(test)]
#[path = "is_check_tests.rs"]
mod tests;
