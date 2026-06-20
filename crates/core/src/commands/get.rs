use crate::{
    adapter::{PlatformAdapter, optional_live_read},
    commands::helpers::resolve_ref_with_context,
    context::CommandContext,
    error::AppError,
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

    let (prop_name, value) = match args.property {
        GetProperty::Role => ("role", json!(entry.role)),
        GetProperty::Title => ("title", json!(entry.name)),
        GetProperty::Text => {
            let live = optional_live_read(adapter.get_live_value(handle.handle()))?;
            ("text", json!(live.or(entry.value)))
        }
        GetProperty::Value => {
            let live = optional_live_read(adapter.get_live_value(handle.handle()))?;
            ("value", json!(live.or(entry.value)))
        }
        GetProperty::Bounds => ("bounds", json!(entry.bounds)),
        GetProperty::States => ("states", json!(entry.states)),
    };

    Ok(json!({ "property": prop_name, "ref": args.ref_id, "value": value }))
}
