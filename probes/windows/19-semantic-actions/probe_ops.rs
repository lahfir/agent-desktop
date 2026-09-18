//! Pattern invoke/read helpers with re-find discipline for A19.

use serde_json::{Value, json};
use uiautomation::patterns::{
    UIExpandCollapsePattern, UIInvokePattern, UILegacyIAccessiblePattern, UIRangeValuePattern,
    UIScrollPattern, UISelectionItemPattern, UITogglePattern, UIValuePattern,
};
use uiautomation::types::{ExpandCollapseState, ScrollAmount, ToggleState, UIProperty};
use uiautomation::{UIAutomation, UIElement};

use crate::util::{
    Bounds, digest_of, element_shape, failure_shape, outcome_of, pattern_available, redacted_text,
    refind, window_is_foreground,
};

pub fn toggle_state_name(state: ToggleState) -> &'static str {
    match state {
        ToggleState::Off => "Off",
        ToggleState::On => "On",
        ToggleState::Indeterminate => "Indeterminate",
    }
}

pub fn expand_state_name(state: ExpandCollapseState) -> &'static str {
    match state {
        ExpandCollapseState::Collapsed => "Collapsed",
        ExpandCollapseState::Expanded => "Expanded",
        ExpandCollapseState::PartiallyExpanded => "PartiallyExpanded",
        ExpandCollapseState::LeafNode => "LeafNode",
    }
}

pub fn read_toggle(element: &UIElement) -> Value {
    match element.get_pattern::<UITogglePattern>() {
        Ok(pattern) => match pattern.get_toggle_state() {
            Ok(state) => json!({ "ok": true, "state": toggle_state_name(state) }),
            Err(error) => json!({ "ok": false, "failure": failure_shape(&error) }),
        },
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn read_value(element: &UIElement) -> Value {
    match element.get_pattern::<UIValuePattern>() {
        Ok(pattern) => match pattern.get_value() {
            Ok(text) => json!({ "ok": true, "value": redacted_text(&text), "readonly": pattern.is_readonly().ok() }),
            Err(error) => json!({ "ok": false, "failure": failure_shape(&error) }),
        },
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn read_range(element: &UIElement) -> Value {
    match element.get_pattern::<UIRangeValuePattern>() {
        Ok(pattern) => match pattern.get_value() {
            Ok(value) => json!({
                "ok": true,
                "value": value,
                "min": pattern.get_minimum().ok(),
                "max": pattern.get_maximum().ok(),
                "readonly": pattern.is_readonly().ok(),
            }),
            Err(error) => json!({ "ok": false, "failure": failure_shape(&error) }),
        },
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn read_expand(element: &UIElement) -> Value {
    match element.get_pattern::<UIExpandCollapsePattern>() {
        Ok(pattern) => match pattern.get_state() {
            Ok(state) => json!({ "ok": true, "state": expand_state_name(state) }),
            Err(error) => json!({ "ok": false, "failure": failure_shape(&error) }),
        },
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn read_selected(element: &UIElement) -> Value {
    match element.get_pattern::<UISelectionItemPattern>() {
        Ok(pattern) => match pattern.is_selected() {
            Ok(selected) => json!({ "ok": true, "selected": selected }),
            Err(error) => json!({ "ok": false, "failure": failure_shape(&error) }),
        },
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn read_status_sink(automation: &UIAutomation, hwnd: isize) -> Value {
    match refind(automation, hwnd, "lblStatus") {
        Ok(element) => match element.get_name() {
            Ok(name) => json!({ "ok": true, "name": redacted_text(&name) }),
            Err(error) => json!({ "ok": false, "failure": failure_shape(&error) }),
        },
        Err(error) => error,
    }
}

pub fn invoke_pattern(element: &UIElement) -> Value {
    match element.get_pattern::<UIInvokePattern>() {
        Ok(pattern) => outcome_of(pattern.invoke()),
        Err(error) => json!({
            "ok": false,
            "pattern": failure_shape(&error),
            "ktd2_arm": crate::util::map_ktd2_arm(
                failure_shape(&error).get("result_hex").and_then(|v| v.as_str()),
                failure_shape(&error).get("code").and_then(|v| v.as_i64()).map(|v| v as i32),
            ),
        }),
    }
}

pub fn toggle_pattern(element: &UIElement) -> Value {
    match element.get_pattern::<UITogglePattern>() {
        Ok(pattern) => outcome_of(pattern.toggle()),
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn set_value_pattern(element: &UIElement, text: &str) -> Value {
    match element.get_pattern::<UIValuePattern>() {
        Ok(pattern) => {
            let mut outcome = outcome_of(pattern.set_value(text));
            if let Some(obj) = outcome.as_object_mut() {
                obj.insert("payload".into(), redacted_text(text));
            }
            outcome
        }
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn set_range_pattern(element: &UIElement, value: f64) -> Value {
    match element.get_pattern::<UIRangeValuePattern>() {
        Ok(pattern) => {
            let mut outcome = outcome_of(pattern.set_value(value));
            if let Some(obj) = outcome.as_object_mut() {
                obj.insert("requested".into(), json!(value));
            }
            outcome
        }
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn expand_pattern(element: &UIElement) -> Value {
    match element.get_pattern::<UIExpandCollapsePattern>() {
        Ok(pattern) => outcome_of(pattern.expand()),
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn collapse_pattern(element: &UIElement) -> Value {
    match element.get_pattern::<UIExpandCollapsePattern>() {
        Ok(pattern) => outcome_of(pattern.collapse()),
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn select_pattern(element: &UIElement) -> Value {
    match element.get_pattern::<UISelectionItemPattern>() {
        Ok(pattern) => outcome_of(pattern.select()),
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn legacy_default_action(element: &UIElement) -> Value {
    match element.get_pattern::<UILegacyIAccessiblePattern>() {
        Ok(pattern) => {
            let default_action = pattern
                .get_default_action()
                .ok()
                .map(|text| redacted_text(&text));
            let mut outcome = outcome_of(pattern.do_default_action());
            if let Some(obj) = outcome.as_object_mut() {
                obj.insert("default_action".into(), json!(default_action));
            }
            outcome
        }
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn scroll_small(
    element: &UIElement,
    horizontal: ScrollAmount,
    vertical: ScrollAmount,
) -> Value {
    match element.get_pattern::<UIScrollPattern>() {
        Ok(pattern) => outcome_of(pattern.scroll(horizontal, vertical)),
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

pub fn get_pattern_absent(element: &UIElement) -> Value {
    let available = pattern_available(element, UIProperty::IsInvokePatternAvailable);
    let attempt = element.get_pattern::<UIInvokePattern>();
    match attempt {
        Ok(_) => json!({
            "shape": "unexpected_success",
            "invoke_available": available,
            "ktd2_arm": "clean_ok",
        }),
        Err(error) => {
            let shape = failure_shape(&error);
            let hex = shape.get("result_hex").and_then(|v| v.as_str());
            let code = shape.get("code").and_then(|v| v.as_i64()).map(|v| v as i32);
            json!({
                "shape": if hex.is_some() { "hresult" } else { "sentinel_or_non_hresult" },
                "invoke_available": available,
                "failure": shape,
                "ktd2_arm": crate::util::map_ktd2_arm(hex, code),
            })
        }
    }
}

pub fn measure_call(
    automation: &UIAutomation,
    hwnd: isize,
    automation_id: &str,
    label: &str,
    invoke: impl FnOnce(&UIElement) -> Value,
    verify: impl FnOnce(&UIElement) -> Value,
) -> Value {
    let before_fg = window_is_foreground(hwnd);
    let element = match refind(automation, hwnd, automation_id) {
        Ok(element) => element,
        Err(error) => {
            return json!({
                "label": label,
                "automation_id_digest": digest_of(automation_id),
                "error": error,
            });
        }
    };
    let shape_before = element_shape(&element);
    let call = invoke(&element);
    std::thread::sleep(std::time::Duration::from_millis(120));
    let after = match refind(automation, hwnd, automation_id) {
        Ok(element) => verify(&element),
        Err(error) => error,
    };
    json!({
        "label": label,
        "automation_id_digest": digest_of(automation_id),
        "shape_before": shape_before,
        "foreground_at_call": before_fg,
        "call": call,
        "verify": after,
        "status_sink": read_status_sink(automation, hwnd),
    })
}

pub fn bounds_of(element: &UIElement) -> Option<String> {
    Bounds::from_element(element).map(|b| b.as_csv())
}
