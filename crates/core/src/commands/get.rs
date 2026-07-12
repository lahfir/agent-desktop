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

pub fn execute(
    args: GetArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let (entry, handle) =
        resolve_ref_with_context(&args.ref_id, args.snapshot_id.as_deref(), adapter, context)?;
    let deadline = crate::Deadline::standard()?;

    let (prop_name, value) = match args.property {
        GetProperty::Role => ("role", json!(entry.identity.role)),
        GetProperty::Title => ("title", json!(entry.identity.name)),
        GetProperty::Text => {
            let live = optional_live_read(adapter.get_live_value(&handle, deadline))?;
            ("text", json!(live.or(entry.identity.value)))
        }
        GetProperty::Value => {
            let live = optional_live_read(adapter.get_live_value(&handle, deadline))?;
            ("value", json!(live.or(entry.identity.value)))
        }
        GetProperty::Bounds => ("bounds", json!(entry.geometry.bounds)),
        GetProperty::States => ("states", json!(entry.capabilities.states)),
    };

    Ok(json!({ "property": prop_name, "ref": args.ref_id, "value": value }))
}
