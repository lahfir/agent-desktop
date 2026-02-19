use crate::{adapter::PlatformAdapter, commands::helpers::resolve_ref, error::AppError};
use serde_json::{json, Value};

pub struct IsArgs {
    pub ref_id: String,
    pub property: IsProperty,
}

pub enum IsProperty {
    Visible,
    Enabled,
    Checked,
    Focused,
    Expanded,
}

pub fn execute(args: IsArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (entry, _handle) = resolve_ref(&args.ref_id, adapter)?;

    let prop_name = match args.property {
        IsProperty::Visible => "visible",
        IsProperty::Enabled => "enabled",
        IsProperty::Checked => "checked",
        IsProperty::Focused => "focused",
        IsProperty::Expanded => "expanded",
    };

    let result = match args.property {
        IsProperty::Visible => true,
        IsProperty::Enabled => !entry.role.is_empty(),
        IsProperty::Checked => false,
        IsProperty::Focused => false,
        IsProperty::Expanded => false,
    };

    Ok(json!({ "property": prop_name, "ref": args.ref_id, "result": result }))
}
