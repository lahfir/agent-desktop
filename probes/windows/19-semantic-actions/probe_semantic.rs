//! U1 item 1: semantic pattern set on the COM product stack (WPF + WinForms).

use serde_json::{Value, json};
use uiautomation::UIAutomation;

use crate::ops::{
    collapse_pattern, expand_pattern, invoke_pattern, measure_call, read_expand, read_range,
    read_selected, read_toggle, read_value, select_pattern, set_range_pattern, set_value_pattern,
    toggle_pattern,
};
use crate::util::{digest_of, redacted_text, refind};

const ASCII: &str = "ascii-roundtrip";
const CJK: &str = "日本語テスト";
const ASTRAL: &str = "a𐐷b";

fn set_value_matrix(automation: &UIAutomation, hwnd: isize, stack: &str) -> Value {
    let payloads = [
        ("ascii", ASCII),
        ("cjk", CJK),
        ("astral", ASTRAL),
    ];
    let mut rows = Vec::new();
    for (label, payload) in payloads {
        rows.push(measure_call(
            automation,
            hwnd,
            "txtValue",
            &format!("SetValue/{label}"),
            |element| set_value_pattern(element, payload),
            |element| {
                let read = read_value(element);
                let equal = read
                    .get("value")
                    .and_then(|value| value.get("digest"))
                    .and_then(|value| value.as_str())
                    == Some(digest_of(payload).as_str());
                json!({ "read": read, "exact_match_by_digest": equal, "payload": redacted_text(payload) })
            },
        ));
    }
    json!(rows)
}

fn restore_seed(automation: &UIAutomation, hwnd: isize) {
    if let Ok(element) = refind(automation, hwnd, "txtValue") {
        let _ = set_value_pattern(&element, "seed-value");
    }
}

pub fn measure_stack(automation: &UIAutomation, hwnd: isize, stack: &str) -> Value {
    if hwnd == 0 {
        return json!({ "skipped": "hwnd unavailable", "stack": stack });
    }

    let invoke = measure_call(
        automation,
        hwnd,
        "btnAction",
        "Invoke",
        invoke_pattern,
        |_| json!({ "note": "no state re-read for Invoke; status_sink recorded" }),
    );

    let toggle = measure_call(
        automation,
        hwnd,
        "chkToggle",
        "Toggle",
        |element| {
            let before = read_toggle(element);
            let call = toggle_pattern(element);
            json!({ "before": before, "call": call })
        },
        read_toggle,
    );

    let values = set_value_matrix(automation, hwnd, stack);
    restore_seed(automation, hwnd);

    let expand = measure_call(
        automation,
        hwnd,
        "cboChoice",
        "ExpandCollapse",
        |element| {
            let before = read_expand(element);
            let expand = expand_pattern(element);
            std::thread::sleep(std::time::Duration::from_millis(80));
            let mid = refind(automation, hwnd, "cboChoice")
                .map(|el| read_expand(&el))
                .unwrap_or(json!({ "refind_failed": true }));
            let collapse = refind(automation, hwnd, "cboChoice")
                .map(|el| collapse_pattern(&el))
                .unwrap_or(json!({ "refind_failed": true }));
            json!({ "before": before, "expand": expand, "mid": mid, "collapse": collapse })
        },
        read_expand,
    );

    let select = measure_call(
        automation,
        hwnd,
        "cboItem1",
        "SelectionItem.Select",
        |element| {
            let _ = refind(automation, hwnd, "cboChoice").map(|combo| expand_pattern(&combo));
            std::thread::sleep(std::time::Duration::from_millis(100));
            select_pattern(element)
        },
        read_selected,
    );

    let range = measure_call(
        automation,
        hwnd,
        "tbSlider",
        "RangeValue.SetValue",
        |element| set_range_pattern(element, 77.0),
        |element| {
            let read = read_range(element);
            let value = read.get("value").and_then(|v| v.as_f64());
            json!({
                "read": read,
                "exact": value == Some(77.0),
                "rounded_or_clamped": value,
            })
        },
    );

    let wpf_blocking = stack == "wpf"
        && [
            invoke.get("call").and_then(|c| c.get("ok")).and_then(|v| v.as_bool()),
            toggle
                .get("call")
                .and_then(|c| c.get("call"))
                .and_then(|c| c.get("ok"))
                .and_then(|v| v.as_bool()),
            range.get("call").and_then(|c| c.get("ok")).and_then(|v| v.as_bool()),
        ]
        .iter()
        .any(|ok| *ok == Some(false));

    let winforms_honest_failure = stack == "winforms"
        && [
            invoke.get("call").and_then(|c| c.get("ok")).and_then(|v| v.as_bool()) == Some(false),
            toggle
                .get("call")
                .and_then(|c| c.get("call"))
                .and_then(|c| c.get("ok"))
                .and_then(|v| v.as_bool())
                == Some(false),
        ]
        .iter()
        .any(|failed| *failed);

    json!({
        "stack": stack,
        "client": "uia3-com-product-CUIAutomation8",
        "Invoke": invoke,
        "Toggle": toggle,
        "SetValue_matrix": values,
        "ExpandCollapse": expand,
        "SelectionItem": select,
        "RangeValue": range,
        "branch": if stack == "wpf" {
            if wpf_blocking { "WPF_COM_FAILURE_BLOCKING" } else { "WPF_COM_VIABLE" }
        } else if winforms_honest_failure {
            "WinForms_COM_HONEST_FAILURE"
        } else {
            "WinForms_COM_VIABLE"
        },
    })
}

pub fn measure(automation: &UIAutomation, wpf: Option<isize>, winforms: Option<isize>) -> Value {
    json!({
        "wpf": wpf.map(|hwnd| measure_stack(automation, hwnd, "wpf")),
        "winforms": winforms.map(|hwnd| measure_stack(automation, hwnd, "winforms")),
    })
}
