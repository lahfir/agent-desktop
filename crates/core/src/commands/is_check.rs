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

/// States are read from the last snapshot's RefMap. `resolve_ref` verifies the element
/// is still live before returning, but the state values themselves are not re-queried
/// from the AX API. Run `snapshot` to refresh state before calling `is`.
pub fn execute(args: IsArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (entry, _handle) = resolve_ref(&args.ref_id, adapter)?;

    let prop_name = match args.property {
        IsProperty::Visible => "visible",
        IsProperty::Enabled => "enabled",
        IsProperty::Checked => "checked",
        IsProperty::Focused => "focused",
        IsProperty::Expanded => "expanded",
    };

    let applicable = is_applicable(&args.property, &entry.role);

    let result = match args.property {
        IsProperty::Visible => !entry.states.contains(&"hidden".to_string()),
        IsProperty::Enabled => !entry.states.contains(&"disabled".to_string()),
        IsProperty::Checked => entry.states.contains(&"checked".to_string()),
        IsProperty::Focused => entry.states.contains(&"focused".to_string()),
        IsProperty::Expanded => entry.states.contains(&"expanded".to_string()),
    };

    Ok(json!({ "property": prop_name, "ref": args.ref_id, "result": result, "applicable": applicable }))
}

fn is_applicable(property: &IsProperty, role: &str) -> bool {
    match property {
        IsProperty::Visible | IsProperty::Enabled | IsProperty::Focused => true,
        IsProperty::Checked => matches!(
            role,
            "checkbox" | "switch" | "radiobutton" | "togglebutton"
                | "menuitemcheckbox" | "menuitemradio"
        ),
        IsProperty::Expanded => matches!(
            role,
            "disclosuretriangle" | "treeitem" | "combobox" | "popupbutton" | "outline" | "row"
        ),
    }
}
