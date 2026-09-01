//! U1 item 5: SetFocus foreground effect on the COM product stack.

use serde_json::{Value, json};
use uiautomation::UIAutomation;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
};

use crate::util::{
    automation_id_of, digest_of, failure_shape, foreground_hwnd, refind, window_is_foreground,
};

pub fn measure(automation: &UIAutomation, background_hwnd: isize, decoy_hwnd: Option<isize>) -> Value {
    if background_hwnd == 0 {
        return json!({ "skipped": "background hwnd unavailable" });
    }
    if let Some(decoy) = decoy_hwnd {
        if decoy != 0 && unsafe { IsWindow(decoy as HWND) } != 0 {
            unsafe {
                SetForegroundWindow(decoy as HWND);
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }
    let fg_before = foreground_hwnd();
    let target_was_fg = window_is_foreground(background_hwnd);
    let element = match refind(automation, background_hwnd, "txtValue") {
        Ok(element) => element,
        Err(error) => return json!({ "error": error }),
    };
    let call = match element.set_focus() {
        Ok(()) => json!({ "ok": true }),
        Err(error) => json!({ "ok": false, "failure": failure_shape(&error) }),
    };
    std::thread::sleep(std::time::Duration::from_millis(200));
    let fg_after = foreground_hwnd();
    let focused = automation.get_focused_element().ok();
    let focused_id = focused.as_ref().and_then(automation_id_of);
    let focused_hwnd = focused
        .as_ref()
        .and_then(|el| el.get_native_window_handle().ok())
        .map(|handle| Into::<isize>::into(handle));
    let moved = fg_before != fg_after && (fg_after == background_hwnd || focused_hwnd == Some(background_hwnd));
    let mut before_pid = 0u32;
    let mut after_pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(fg_before as HWND, &mut before_pid);
        GetWindowThreadProcessId(fg_after as HWND, &mut after_pid);
    }
    json!({
        "target_hwnd_nonzero": background_hwnd != 0,
        "target_was_foreground_before": target_was_fg,
        "foreground_changed": fg_before != fg_after,
        "foreground_pid_changed": before_pid != after_pid,
        "foreground_equals_target_after": fg_after == background_hwnd,
        "set_focus": call,
        "focused_automation_id_digest": focused_id.as_ref().map(|id| digest_of(id)),
        "focused_automation_id_len": focused_id.as_ref().map(|id| id.len()),
        "branch": if moved || fg_after == background_hwnd {
            "foreground_moves_ktd4_10_stands"
        } else {
            "foreground_did_not_move_gate_stands_on_A3_4"
        },
        "stack": "uia3-com-product-CUIAutomation8",
    })
}
