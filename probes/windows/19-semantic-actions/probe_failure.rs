//! U1 item 2: failure taxonomy staging for KTD2 arms.

use serde_json::{Value, json};
use uiautomation::UIAutomation;

use crate::ops::{
    expand_pattern, get_pattern_absent, invoke_pattern, read_expand, read_range, read_toggle,
    set_range_pattern, set_value_pattern, toggle_pattern, toggle_state_name,
};
use crate::util::{digest_of, element_shape, refind, root_from_hwnd, walk_tree};
use uiautomation::patterns::UITogglePattern;

pub fn measure_taxonomy(automation: &UIAutomation, hwnd: isize) -> Value {
    if hwnd == 0 {
        return json!({ "skipped": "hwnd unavailable" });
    }

    let get_pattern = match refind(automation, hwnd, "txtValue") {
        Ok(element) => get_pattern_absent(&element),
        Err(error) => error,
    };

    let readonly_set = match refind(automation, hwnd, "txtReadOnly") {
        Ok(element) => json!({
            "shape": element_shape(&element),
            "call": set_value_pattern(&element, "should-fail"),
            "after": crate::ops::read_value(&element),
        }),
        Err(error) => error,
    };

    let disabled_value = match refind(automation, hwnd, "txtDisabled") {
        Ok(element) => json!({
            "shape": element_shape(&element),
            "call": set_value_pattern(&element, "disabled-write"),
        }),
        Err(error) => error,
    };

    let disabled_range = match refind(automation, hwnd, "tbSliderDisabled") {
        Ok(element) => json!({
            "shape": element_shape(&element),
            "call": set_range_pattern(&element, 90.0),
        }),
        Err(error) => error,
    };

    let leaf_expand = match refind(automation, hwnd, "trvLeaf") {
        Ok(element) => {
            let before = read_expand(&element);
            let call = expand_pattern(&element);
            let after = refind(automation, hwnd, "trvLeaf")
                .map(|el| read_expand(&el))
                .unwrap_or(json!({ "refind_failed": true }));
            json!({ "before": before, "call": call, "after": after, "shape": element_shape(&element) })
        }
        Err(_) => match refind(automation, hwnd, "lblStatus") {
            Ok(element) => {
                let before = read_expand(&element);
                let call = expand_pattern(&element);
                json!({
                    "fallback_target": "lblStatus",
                    "before": before,
                    "call": call,
                    "shape": element_shape(&element),
                    "note": "trvLeaf absent; measured Expand on non-expandable sink",
                })
            }
            Err(error) => error,
        },
    };

    let out_of_range = match refind(automation, hwnd, "tbSlider") {
        Ok(element) => {
            let before = read_range(&element);
            let high = set_range_pattern(&element, 10_000.0);
            let after_high = refind(automation, hwnd, "tbSlider")
                .map(|el| read_range(&el))
                .unwrap_or(json!({ "refind_failed": true }));
            let low = refind(automation, hwnd, "tbSlider")
                .map(|el| set_range_pattern(&el, -50.0))
                .unwrap_or(json!({ "refind_failed": true }));
            let after_low = refind(automation, hwnd, "tbSlider")
                .map(|el| read_range(&el))
                .unwrap_or(json!({ "refind_failed": true }));
            json!({
                "before": before,
                "set_10000": high,
                "after_10000": after_high,
                "set_neg50": low,
                "after_neg50": after_low,
                "branch": classify_range_branch(&high, &after_high),
            })
        }
        Err(error) => error,
    };

    let tri_state = measure_tri_state(automation, hwnd);

    json!({
        "get_pattern_absent": get_pattern,
        "setvalue_readonly": readonly_set,
        "setvalue_disabled": disabled_value,
        "rangevalue_disabled": disabled_range,
        "expand_leaf": leaf_expand,
        "rangevalue_out_of_range": out_of_range,
        "tri_state_toggle": tri_state,
    })
}

fn classify_range_branch(call: &Value, after: &Value) -> &'static str {
    let ok = call.get("ok").and_then(|v| v.as_bool()) == Some(true);
    let value = after.get("value").and_then(|v| v.as_f64());
    if !ok {
        "error"
    } else if value == Some(10_000.0) {
        "accepted_out_of_range"
    } else if value == Some(100.0) {
        "clamped_to_max"
    } else {
        "provider_specific"
    }
}

fn measure_tri_state(automation: &UIAutomation, hwnd: isize) -> Value {
    let Ok(element) = refind(automation, hwnd, "chkTriState") else {
        return json!({ "skipped": "chkTriState absent" });
    };
    let mut cycle = Vec::new();
    let mut current = element;
    for step in 0..3 {
        let before = read_toggle(&current);
        let call = toggle_pattern(&current);
        std::thread::sleep(std::time::Duration::from_millis(80));
        current = match refind(automation, hwnd, "chkTriState") {
            Ok(element) => element,
            Err(error) => {
                return json!({ "steps": cycle, "error": error, "failed_at": step });
            }
        };
        let after = read_toggle(&current);
        cycle.push(json!({
            "step": step,
            "before": before,
            "call": call,
            "after": after,
        }));
    }
    let states: Vec<String> = cycle
        .iter()
        .filter_map(|row| {
            row.get("after")
                .and_then(|v| v.get("state"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .collect();
    let full_cycle = states.len() == 3
        && states.iter().any(|s| s == "Off")
        && states.iter().any(|s| s == "On")
        && states.iter().any(|s| s == "Indeterminate");
    json!({
        "steps": cycle,
        "states_seen": states,
        "full_cycle_observed": full_cycle,
        "initial_pattern_state": refind(automation, hwnd, "chkTriState")
            .ok()
            .and_then(|el| el.get_pattern::<UITogglePattern>().ok())
            .and_then(|p| p.get_toggle_state().ok())
            .map(toggle_state_name),
    })
}

pub fn measure_killed(
    automation: &UIAutomation,
    hwnd: isize,
    pid: u32,
    automation_id: &str,
) -> Value {
    let element = match refind(automation, hwnd, automation_id) {
        Ok(element) => element,
        Err(error) => return json!({ "pre_kill_refind_failed": error }),
    };
    let shape = element_shape(&element);
    let kill = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output();
    let kill_ok = kill.map(|output| output.status.success()).unwrap_or(false);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let set_value = set_value_pattern(&element, "post-kill");
    let invoke = invoke_pattern(&element);
    let re_root = root_from_hwnd(automation, hwnd);
    let re_walk = re_root
        .as_ref()
        .ok()
        .map(|root| walk_tree(automation, root));
    json!({
        "automation_id_digest": digest_of(automation_id),
        "pre_kill_shape": shape,
        "kill_ok": kill_ok,
        "setvalue_after_kill": set_value,
        "invoke_after_kill": invoke,
        "root_after_kill": re_root.err(),
        "walk_after_kill": match re_walk {
            Some(Ok(elements)) => json!({ "ok": true, "count": elements.len() }),
            Some(Err(error)) => error,
            None => json!({ "skipped": true }),
        },
        "expected_hresult": "0x80040201",
        "setvalue_ktd2": set_value.get("ktd2_arm").cloned(),
        "invoke_ktd2": invoke.get("ktd2_arm").cloned(),
    })
}

pub fn measure(automation: &UIAutomation, wpf: Option<isize>) -> Value {
    json!({
        "wpf_taxonomy": wpf.map(|hwnd| measure_taxonomy(automation, hwnd)),
        "note": "killed-provider leg is merged by the orchestrator",
    })
}
