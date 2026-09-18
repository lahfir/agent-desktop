//! Hang defense against a never-pumping window (U1 item 4).

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use uiautomation::UIAutomation;
use uiautomation::types::{Handle, Point};

use crate::hit::{element_root_hwnd, root_from_hwnd};
use crate::util::{Bounds, failure_shape};
use crate::win;

const CONNECTION_TIMEOUT_MS: u64 = 2_000;

fn timed_call<T>(label: &str, work: impl FnOnce() -> Result<T, uiautomation::Error>) -> Value {
    let started = Instant::now();
    let outcome = work();
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let bounded = elapsed_ms < (CONNECTION_TIMEOUT_MS as f64) * 2.5;
    match outcome {
        Ok(_) => json!({
            "call": label,
            "ok": true,
            "elapsed_ms": elapsed_ms,
            "bounded_by_connection_timeout": bounded,
        }),
        Err(error) => json!({
            "call": label,
            "ok": false,
            "elapsed_ms": elapsed_ms,
            "bounded_by_connection_timeout": bounded,
            "failure": failure_shape(&error),
        }),
    }
}

/// Measures whether the bounded client's connection timeout binds
/// `ElementFromPoint`, `ElementFromHandle`, and the hit-root ancestor walk
/// against a never-pumping window. The stalled host is dropped only after
/// every timed call, held through a closing sleep so no measurement races
/// its teardown.
pub fn measure_hang(automation: &UIAutomation) -> Value {
    let stalled = match win::spawn_stalled() {
        Ok(stalled) => stalled,
        Err(error) => {
            return json!({
                "stalled_fixture": false,
                "error": error,
            });
        }
    };
    let mut rect = windows_sys::Win32::Foundation::RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let has_rect = unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(
            stalled.handle as windows_sys::Win32::Foundation::HWND,
            &mut rect,
        )
    } != 0;
    let (x, y) = if has_rect {
        (
            rect.left + (rect.right - rect.left) / 2,
            rect.top + (rect.bottom - rect.top) / 2,
        )
    } else {
        (160, 140)
    };

    let element_from_point = timed_call("ElementFromPoint", || {
        automation.element_from_point(Point::new(x, y))
    });

    let element_from_handle = timed_call("ElementFromHandle", || {
        automation.element_from_handle(Handle::from(stalled.handle))
    });

    let mut occluder_evidence = json!({ "skipped": "no hit element to read" });
    let mut ancestor_walk = json!({ "skipped": "no hit element to walk" });
    if element_from_point.get("ok") == Some(&json!(true)) {
        if let Ok(hit) = automation.element_from_point(Point::new(x, y)) {
            let started = Instant::now();
            let name = hit.get_name();
            let bounds = Bounds::from_element(&hit);
            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            occluder_evidence = json!({
                "call": "occluder_evidence_read",
                "elapsed_ms": elapsed_ms,
                "bounded_by_connection_timeout": elapsed_ms < (CONNECTION_TIMEOUT_MS as f64) * 2.5,
                "name_read_ok": name.is_ok(),
                "name_withheld_from_capture": true,
                "bounds": bounds.as_ref().map(Bounds::as_csv),
                "name_failure": name.err().map(|e| failure_shape(&e)),
            });

            let started = Instant::now();
            let root = element_root_hwnd(automation, &hit);
            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            ancestor_walk = json!({
                "call": "hit_root_ancestor_walk",
                "elapsed_ms": elapsed_ms,
                "bounded_by_connection_timeout": elapsed_ms < (CONNECTION_TIMEOUT_MS as f64) * 2.5,
                "root_obtained": root.is_some(),
                "root": root,
            });
        }
    } else if let Ok(root) = root_from_hwnd(automation, stalled.handle) {
        let _ = root;
    }

    let efp_bounded = element_from_point
        .get("bounded_by_connection_timeout")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let efh_bounded = element_from_handle
        .get("bounded_by_connection_timeout")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let branch = if efp_bounded && efh_bounded {
        "connection_timeout_bounds_swallow_to_unknown"
    } else {
        "does_not_bound_needs_windowfrompoint_preprobe"
    };

    std::thread::sleep(Duration::from_millis(50));
    drop(stalled);

    json!({
        "stack": "uia3-com-product-CUIAutomation8",
        "stalled_fixture": true,
        "probe_point": format!("{x},{y}"),
        "element_from_point": element_from_point,
        "element_from_handle": element_from_handle,
        "occluder_evidence_read": occluder_evidence,
        "hit_root_ancestor_walk": ancestor_walk,
        "connection_timeout_ms": CONNECTION_TIMEOUT_MS,
        "hang_defense_branch": branch,
    })
}
