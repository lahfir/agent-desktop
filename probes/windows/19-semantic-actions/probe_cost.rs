//! U1 item 8: min-of-seven discarded warm-up costs for representative pattern calls.

use serde_json::{Value, json};
use uiautomation::types::ScrollAmount;
use uiautomation::UIAutomation;

use crate::ops::{
    expand_pattern, invoke_pattern, legacy_default_action, scroll_small, select_pattern,
    set_value_pattern, toggle_pattern,
};
use crate::util::{min_of_ms, refind};

pub fn measure(automation: &UIAutomation, wpf: Option<isize>) -> Value {
    let Some(hwnd) = wpf.filter(|value| *value != 0) else {
        return json!({ "skipped": "wpf hwnd unavailable" });
    };

    let invoke = time_on(automation, hwnd, "btnAction", |element| {
        invoke_pattern(element)
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            .then_some(())
            .ok_or(())
    });
    let toggle = time_on(automation, hwnd, "chkToggle", |element| {
        toggle_pattern(element)
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            .then_some(())
            .ok_or(())
    });
    let set_value = time_on(automation, hwnd, "txtValue", |element| {
        set_value_pattern(element, "cost-probe")
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            .then_some(())
            .ok_or(())
    });
    let select = {
        let _ = refind(automation, hwnd, "cboChoice").map(|combo| expand_pattern(&combo));
        std::thread::sleep(std::time::Duration::from_millis(80));
        time_on(automation, hwnd, "cboItem1", |element| {
            select_pattern(element)
                .get("ok")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                .then_some(())
                .ok_or(())
        })
    };
    let scroll = time_on(automation, hwnd, "svOuter", |element| {
        scroll_small(
            element,
            ScrollAmount::NoAmount,
            ScrollAmount::SmallIncrement,
        )
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        .then_some(())
        .ok_or(())
    });
    let click_chain = min_of_ms(|| {
        let button = refind(automation, hwnd, "btnAction").map_err(|_| ())?;
        let invoke_ok = invoke_pattern(&button)
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if invoke_ok {
            return Ok(());
        }
        let legacy_ok = legacy_default_action(&button)
            .get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if legacy_ok {
            Ok(())
        } else {
            Err(())
        }
    });

    json!({
        "methodology": "min-of-seven discard warm-up (A15-13)",
        "stack": "uia3-com-product-CUIAutomation8",
        "Invoke": invoke,
        "Toggle": toggle,
        "SetValue": set_value,
        "Select": select,
        "Scroll": scroll,
        "click_chain_worst_case": click_chain,
    })
}

fn time_on(
    automation: &UIAutomation,
    hwnd: isize,
    automation_id: &str,
    mut op: impl FnMut(&uiautomation::UIElement) -> Result<(), ()>,
) -> Value {
    min_of_ms(|| {
        let element = refind(automation, hwnd, automation_id).map_err(|_| ())?;
        op(&element)
    })
}
