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

    let value = match args.property {
        GetProperty::Role => json!(entry.role),
        GetProperty::Title => json!(entry.name),
        GetProperty::Text | GetProperty::Value => {
            let live = optional_live_read(adapter.get_live_value(handle.handle()))?;
            json!(live.or(entry.value))
        }
        GetProperty::Bounds => json!(entry.bounds),
        GetProperty::States => json!(entry.states),
    };

    let prop_name = match args.property {
        GetProperty::Text => "text",
        GetProperty::Value => "value",
        GetProperty::Title => "title",
        GetProperty::Bounds => "bounds",
        GetProperty::Role => "role",
        GetProperty::States => "states",
    };

    Ok(json!({ "property": prop_name, "ref": args.ref_id, "value": value }))
}
