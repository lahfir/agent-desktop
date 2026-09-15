//! U1 item 3: secure-field SetValue with planted marker; self-redacting capture.

use serde_json::{Value, json};
use uiautomation::types::UIProperty;
use uiautomation::UIAutomation;

use crate::ops::{read_value, set_value_pattern};
use crate::util::{bool_of, digest_of, element_shape, redacted_text, refind, SECRET_MARKER};

pub fn measure(automation: &UIAutomation, wpf: Option<isize>, winforms: Option<isize>) -> Value {
    json!({
        "marker_len": SECRET_MARKER.len(),
        "marker_digest": digest_of(SECRET_MARKER),
        "wpf": wpf.map(|hwnd| measure_password(automation, hwnd, "pwdSecret", "wpf")),
        "winforms": winforms.map(|hwnd| measure_password(automation, hwnd, "pwdSecret", "winforms")),
        "capture_rule": "never emit marker verbatim; lengths and digests only",
    })
}

fn measure_password(
    automation: &UIAutomation,
    hwnd: isize,
    automation_id: &str,
    stack: &str,
) -> Value {
    let element = match refind(automation, hwnd, automation_id) {
        Ok(element) => element,
        Err(error) => return json!({ "stack": stack, "error": error }),
    };
    let is_password = element
        .get_property_value(UIProperty::IsPassword)
        .ok()
        .and_then(|variant| bool_of(&variant));
    let shape = element_shape(&element);
    let before = read_value(&element);
    let name_before = element.get_name().ok().map(|text| redacted_text(&text));
    let write = set_value_pattern(&element, SECRET_MARKER);
    std::thread::sleep(std::time::Duration::from_millis(100));
    let after_element = match refind(automation, hwnd, automation_id) {
        Ok(element) => element,
        Err(error) => {
            return json!({
                "stack": stack,
                "is_password": is_password,
                "shape": shape,
                "before_value": before,
                "name_before": name_before,
                "write": write,
                "refind_after": error,
            });
        }
    };
    let after = read_value(&after_element);
    let name_after = after_element
        .get_name()
        .ok()
        .map(|text| redacted_text(&text));
    let write_ok = write.get("ok").and_then(|v| v.as_bool()) == Some(true);
    let echoed = [
        before
            .get("value")
            .and_then(|v| v.get("contains_marker"))
            .and_then(|v| v.as_bool()),
        after
            .get("value")
            .and_then(|v| v.get("contains_marker"))
            .and_then(|v| v.as_bool()),
        name_before
            .as_ref()
            .and_then(|v| v.get("contains_marker"))
            .and_then(|v| v.as_bool()),
        name_after
            .as_ref()
            .and_then(|v| v.get("contains_marker"))
            .and_then(|v| v.as_bool()),
    ]
    .iter()
    .any(|flag| *flag == Some(true));
    let branch = if write_ok && !echoed {
        "write_lands_ktd7_as_designed"
    } else if !write_ok {
        "write_rejected_classifier_arm"
    } else {
        "echo_observed_withholding_fixture"
    };
    json!({
        "stack": stack,
        "automation_id_digest": digest_of(automation_id),
        "is_password": is_password,
        "shape": shape,
        "before_value": before,
        "name_before": name_before,
        "write": write,
        "after_value": after,
        "name_after": name_after,
        "any_echo_of_marker": echoed,
        "branch": branch,
        "error_echo_check": write
            .get("failure")
            .map(|_| json!({ "failure_present": true, "note": "failure_shape carries no content strings" }))
            .unwrap_or(json!({ "failure_present": false })),
        "legacy_value": after_element
            .get_pattern::<uiautomation::patterns::UILegacyIAccessiblePattern>()
            .ok()
            .and_then(|pattern| pattern.get_value().ok())
            .map(|text| redacted_text(&text))
            .unwrap_or_else(|| json!({ "unavailable": true })),
        "plant_path": "fixture planted marker in-process and asserted length before Show",
    })
}

pub fn marker_leaked_in(value: &Value) -> bool {
    let rendered = value.to_string();
    rendered.contains(SECRET_MARKER)
}
