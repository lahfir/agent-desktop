//! U1 item 4: Medium→High pattern writes when the orchestrator manufactures Medium.

use serde_json::{Value, json};
use uiautomation::UIAutomation;

use crate::ops::{invoke_pattern, read_value, set_value_pattern};
use crate::util::{digest_of, element_shape, refind, window_is_foreground};

pub fn measure(
    automation: &UIAutomation,
    high_hwnd: isize,
    value_id: &str,
    invoke_id: &str,
) -> Value {
    if high_hwnd == 0 {
        return json!({
            "measurable": false,
            "branch": "unmeasurable",
            "reason": "high-owned hwnd unavailable",
        });
    }
    let set_value = match refind(automation, high_hwnd, value_id) {
        Ok(element) => {
            let before = read_value(&element);
            let call = set_value_pattern(&element, "uipi-medium-write");
            std::thread::sleep(std::time::Duration::from_millis(120));
            let after = refind(automation, high_hwnd, value_id)
                .map(|el| read_value(&el))
                .unwrap_or(json!({ "refind_failed": true }));
            let effect = before != after;
            json!({
                "automation_id_digest": digest_of(value_id),
                "shape": element_shape(&element),
                "foreground_at_call": window_is_foreground(high_hwnd),
                "call": call,
                "before": before,
                "after": after,
                "effect_observed": effect,
            })
        }
        Err(error) => error,
    };
    let invoke = match refind(automation, high_hwnd, invoke_id) {
        Ok(element) => json!({
            "automation_id_digest": digest_of(invoke_id),
            "shape": element_shape(&element),
            "foreground_at_call": window_is_foreground(high_hwnd),
            "call": invoke_pattern(&element),
        }),
        Err(error) => error,
    };
    let set_arm = set_value
        .get("call")
        .and_then(|call| call.get("ktd2_arm"))
        .and_then(|v| v.as_str());
    let effect = set_value
        .get("effect_observed")
        .and_then(|v| v.as_bool())
        == Some(true);
    let branch = if set_arm == Some("denied_E_ACCESSDENIED") {
        "clean_E_ACCESSDENIED_denied_arm_live"
    } else if set_value.get("call").and_then(|c| c.get("ok")).and_then(|v| v.as_bool())
        == Some(true)
        && !effect
    {
        "silent_no_effect_success_ktd6_verification"
    } else if effect {
        "delivery_across_uipi_recorded"
    } else {
        "other_outcome_recorded"
    };
    json!({
        "measurable": true,
        "integrity_note": "probe process expected Medium; target window expected High",
        "SetValue": set_value,
        "Invoke": invoke,
        "branch": branch,
    })
}
