//! Hit classification, WindowFromPoint corroboration helpers, and cost timing.

use serde_json::{Value, json};
use uiautomation::types::{Handle, Point};
use uiautomation::{UIAutomation, UIElement};
use windows_sys::Win32::Foundation::{HWND, POINT as WinPoint};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, GetSystemMetrics, GetWindowThreadProcessId, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WindowFromPoint,
};

use crate::util::{
    ANCESTRY_DEPTH_CAP, Bounds, REPEATS, element_shape, failure_shape,
};

pub fn virtual_screen() -> Bounds {
    let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    Bounds {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

pub fn window_from_point_root(x: i32, y: i32) -> isize {
    let point = WinPoint { x, y };
    let hwnd = unsafe { WindowFromPoint(point) };
    if hwnd.is_null() {
        return 0;
    }
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    if root.is_null() {
        hwnd as isize
    } else {
        root as isize
    }
}

pub fn window_from_point_child(x: i32, y: i32) -> isize {
    let point = WinPoint { x, y };
    let hwnd = unsafe { WindowFromPoint(point) };
    hwnd as isize
}

pub fn pid_of_hwnd(hwnd: isize) -> u32 {
    if hwnd == 0 {
        return 0;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd as HWND, &mut pid) };
    pid
}

pub fn native_hwnd(element: &UIElement) -> isize {
    element
        .get_native_window_handle()
        .ok()
        .map(Into::into)
        .unwrap_or(0)
}

pub fn element_root_hwnd(automation: &UIAutomation, element: &UIElement) -> Option<isize> {
    let walker = automation.get_raw_view_walker().ok()?;
    let mut current = element.clone();
    for _ in 0..ANCESTRY_DEPTH_CAP {
        let handle = native_hwnd(&current);
        if handle != 0 {
            let root = unsafe { GetAncestor(handle as HWND, GA_ROOT) };
            return Some(if root.is_null() {
                handle
            } else {
                root as isize
            });
        }
        current = walker.get_parent(&current).ok()?;
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitRelation {
    SelfHit,
    Descendant,
    Ancestor,
    UnrelatedSameRoot,
    UnrelatedCrossRoot,
    Unobtainable,
}

pub fn classify_hit(
    automation: &UIAutomation,
    target: &UIElement,
    hit: &UIElement,
) -> HitRelation {
    if automation.compare_elements(target, hit).unwrap_or(false) {
        return HitRelation::SelfHit;
    }
    let Ok(walker) = automation.get_raw_view_walker() else {
        return HitRelation::Unobtainable;
    };
    let mut current = hit.clone();
    for _ in 0..ANCESTRY_DEPTH_CAP {
        match walker.get_parent(&current) {
            Ok(parent) => {
                if automation.compare_elements(target, &parent).unwrap_or(false) {
                    return HitRelation::Descendant;
                }
                current = parent;
            }
            Err(_) => break,
        }
    }
    let mut current = target.clone();
    for _ in 0..ANCESTRY_DEPTH_CAP {
        match walker.get_parent(&current) {
            Ok(parent) => {
                if automation.compare_elements(hit, &parent).unwrap_or(false) {
                    return HitRelation::Ancestor;
                }
                current = parent;
            }
            Err(_) => break,
        }
    }
    let target_root = element_root_hwnd(automation, target);
    let hit_root = element_root_hwnd(automation, hit);
    match (target_root, hit_root) {
        (Some(t), Some(h)) if t == h => HitRelation::UnrelatedSameRoot,
        (Some(_), Some(_)) => HitRelation::UnrelatedCrossRoot,
        _ => HitRelation::Unobtainable,
    }
}

pub fn relation_name(relation: HitRelation) -> &'static str {
    match relation {
        HitRelation::SelfHit => "self",
        HitRelation::Descendant => "descendant",
        HitRelation::Ancestor => "ancestor",
        HitRelation::UnrelatedSameRoot => "unrelated_same_root",
        HitRelation::UnrelatedCrossRoot => "unrelated_cross_root",
        HitRelation::Unobtainable => "unobtainable",
    }
}

pub fn ktd3_arm(
    target_root: Option<isize>,
    hit_root: Option<isize>,
    win32_root: isize,
) -> &'static str {
    match (target_root, hit_root) {
        (Some(t), Some(h)) if h == t && win32_root == t => "same_root_intercepted",
        (Some(t), Some(h)) if h != t && win32_root != t && win32_root == h => {
            "cross_window_intercepted"
        }
        (Some(t), None) if win32_root != t && win32_root != 0 => "pid_widening_candidate",
        (Some(t), Some(h)) if h != t && win32_root == t => "win32_skip_unknown",
        _ => "divergence_unknown",
    }
}

pub fn candidate_points(bounds: &Bounds) -> [(i32, i32); 5] {
    let point = |fx: f64, fy: f64| -> (i32, i32) {
        (
            bounds.left + ((bounds.width() as f64) * fx) as i32,
            bounds.top + ((bounds.height() as f64) * fy) as i32,
        )
    };
    [
        point(0.5, 0.5),
        point(0.25, 0.25),
        point(0.75, 0.25),
        point(0.25, 0.75),
        point(0.75, 0.75),
    ]
}

pub fn element_from_point_shape(automation: &UIAutomation, x: i32, y: i32) -> Value {
    match automation.element_from_point(Point::new(x, y)) {
        Ok(element) => json!({
            "ok": true,
            "hit": element_shape(&element),
        }),
        Err(error) => json!({
            "ok": false,
            "failure": failure_shape(&error),
        }),
    }
}

pub fn min_of_ms(mut operation: impl FnMut() -> Result<(), ()>) -> Value {
    let mut samples = Vec::with_capacity(REPEATS);
    let _ = operation();
    for _ in 0..REPEATS {
        let started = std::time::Instant::now();
        let _ = operation();
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    json!({
        "min_ms": samples[0],
        "median_ms": samples[samples.len() / 2],
        "max_ms": samples[samples.len() - 1],
        "n": samples.len(),
        "warmup_discarded": true,
    })
}

pub fn root_from_hwnd(automation: &UIAutomation, hwnd: isize) -> Result<UIElement, Value> {
    automation
        .element_from_handle(Handle::from(hwnd))
        .map_err(|error| json!({ "root_failed": failure_shape(&error) }))
}
