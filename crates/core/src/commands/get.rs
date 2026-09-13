use crate::{
    AppError,
    adapter::{PlatformAdapter, optional_live_read},
    commands::helpers::resolve_ref_with_context,
    context::CommandContext,
};
use serde_json::{Value, json};

pub struct GetArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
    pub property: GetProperty,
}

pub enum GetProperty {
    Text,
    Value,
    Title,
    Bounds,
    Role,
    States,
}

/// An empty string is not text a person reads, so it does not block the
/// fallback to the other half of the element's identity.
fn meaningful(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.trim().is_empty())
}

pub fn execute(
    args: GetArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let (entry, handle) =
        resolve_ref_with_context(&args.ref_id, args.snapshot_id.as_deref(), adapter, context)?;
    let deadline = crate::Deadline::standard()?;

    let (prop_name, value, bounds_live) = match args.property {
        GetProperty::Role => ("role", json!(entry.identity.role), None),
        GetProperty::Title => ("title", json!(entry.identity.name), None),
        GetProperty::Text => {
            let live = optional_live_read(adapter.get_live_value(&handle, deadline))?;
            let value = meaningful(live.or(entry.identity.value));
            let name = meaningful(entry.identity.name);
            let readable = if crate::role_text::value_is_the_readable_text(&entry.identity.role) {
                value.or(name)
            } else {
                name.or(value)
            };
            ("text", json!(readable), None)
        }
        GetProperty::Value => {
            let live = optional_live_read(adapter.get_live_value(&handle, deadline))?;
            ("value", json!(live.or(entry.identity.value)), None)
        }
        GetProperty::Bounds => {
            let live = optional_live_read(adapter.get_element_bounds(&handle, deadline))?;
            let bounds_live = live.is_some();
            (
                "bounds",
                json!(live.or(entry.geometry.bounds)),
                Some(bounds_live),
            )
        }
        GetProperty::States => ("states", json!(entry.capabilities.states), None),
    };

    let mut response = json!({ "property": prop_name, "ref": args.ref_id, "value": value });
    if let Some(live) = bounds_live {
        response["live"] = json!(live);
    }
    Ok(response)
}

#[cfg(test)]
#[path = "get_tests.rs"]
mod tests;
