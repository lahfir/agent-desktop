use crate::{adapter::PlatformAdapter, commands::helpers::resolve_ref, error::AppError};
use serde_json::{json, Value};

pub struct GetArgs {
    pub ref_id: String,
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

pub fn execute(args: GetArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (entry, handle) = resolve_ref(&args.ref_id, adapter)?;

    let value = match args.property {
        GetProperty::Role => json!(entry.role),
        GetProperty::Title => json!(entry.name),
        GetProperty::Text | GetProperty::Value => {
            let live = adapter.get_live_value(&handle).ok().flatten();
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
