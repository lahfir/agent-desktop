use super::hit_test_impl;
use crate::tree::automation::{automation_client, root_from_hwnd};
use crate::tree::element::UIAElement;
use crate::tree::fixture::{LocalFixture, ensure_test_apartment};
use crate::tree::fixture_overlay;
use crate::tree::fixture_window;
use crate::tree::walker_fake::deadline;
use agent_desktop_core::{
    AdapterError, Point, Rect, hit_test::HitTestResult, native_handle::NativeHandle,
};
use uiautomation::types::Handle;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, HWND_TOPMOST, SWP_SHOWWINDOW, SetWindowPos,
};

/// The window text the fixture gives its overlay child. An interception that
/// names it is the staged occluder; any other name is a window this repository
/// does not own, and the pin it would satisfy is not the one being made.
const OVERLAY_LABEL: &str = "fixture-overlay";

#[test]
fn on_screen_fixture_center_reaches_target() {
    ensure_test_apartment();
    let (left, top) = fixture_window::on_screen_origin();
    let fixture = LocalFixture::create_at(left, top).expect("on-screen fixture starts");
    fixture_overlay::raise_window(fixture.handle());
    let handle = control_handle(&fixture).expect("button handle");
    let bounds = window_bounds(fixture_window::find_button(fixture.handle()));
    let point = Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    };
    let result = hit_test_impl(&handle, point.clone(), deadline()).expect("hit_test succeeds");
    let owner = win32_root_at(&point);
    if owner == fixture.handle() {
        assert_eq!(
            result,
            HitTestResult::ReachesTarget,
            "the window manager gives the point to the fixture, so nothing occludes it"
        );
    } else {
        eprintln!("{}", foreign_owner_skip("on-screen ReachesTarget", owner));
    }
    fixture_overlay::clear_topmost(fixture.handle());
}

#[test]
fn minimized_on_screen_fixture_yields_unknown() {
    ensure_test_apartment();
    let (left, top) = fixture_window::on_screen_origin();
    let fixture = LocalFixture::create_at(left, top).expect("on-screen fixture starts");
    let handle = control_handle(&fixture).expect("button handle");
    let bounds = window_bounds(fixture_window::find_button(fixture.handle()));
    let point = Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    };
    fixture.minimize();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let result = hit_test_impl(&handle, point, deadline()).expect("hit_test succeeds");
    assert_eq!(result, HitTestResult::Unknown);
}

#[test]
fn same_root_overlay_reports_intercepted_by() {
    ensure_test_apartment();
    let (left, top) = fixture_window::on_screen_origin();
    let fixture = LocalFixture::create_at(left, top).expect("on-screen fixture starts");
    fixture_overlay::raise_window(fixture.handle());
    let handle = control_handle(&fixture).expect("covered button handle");
    let bounds = window_bounds(fixture_window::find_button(fixture.handle()));
    let point = Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    };
    let overlay = fixture_overlay::stage_sibling_overlay(fixture.handle());
    assert!(!overlay.is_null(), "overlay stages over the primary button");
    std::thread::sleep(std::time::Duration::from_millis(50));
    let result = hit_test_impl(&handle, point.clone(), deadline()).expect("hit_test succeeds");
    let owner = win32_root_at(&point);
    if owner != fixture.handle() {
        eprintln!("{}", foreign_owner_skip("same-root InterceptedBy", owner));
        fixture_overlay::clear_topmost(fixture.handle());
        return;
    }
    match result {
        HitTestResult::InterceptedBy { role, name, .. } => {
            assert!(role.is_some(), "occluder role is always present");
            assert!(
                name.as_deref()
                    .is_some_and(|label| label.contains(OVERLAY_LABEL)),
                "the interception must name the staged fixture overlay, got {name:?}"
            );
        }
        other => panic!("same-root overlay must InterceptedBy, got {other:?}"),
    }
    fixture_overlay::clear_topmost(fixture.handle());
}

#[test]
fn cross_window_overlap_reports_intercepted_and_uncovered_reaches() {
    ensure_test_apartment();
    let (left, top) = fixture_window::on_screen_origin();
    let under = LocalFixture::create_at(left, top).expect("under fixture");
    let (over_left, over_top) = fixture_window::on_screen_origin();
    let over = LocalFixture::create_at(over_left, over_top).expect("over fixture");
    let under_button = fixture_window::find_button(under.handle());
    let covered_bounds = window_bounds(under_button);
    unsafe {
        SetWindowPos(
            over.handle() as *mut std::ffi::c_void,
            HWND_TOPMOST,
            covered_bounds.x as i32 - 40,
            covered_bounds.y as i32 - 40,
            420,
            320,
            SWP_SHOWWINDOW,
        );
    }
    std::thread::sleep(std::time::Duration::from_millis(80));

    let under_handle = control_handle(&under).expect("under button");
    let covered_point = Point {
        x: covered_bounds.x + covered_bounds.width / 2.0,
        y: covered_bounds.y + covered_bounds.height / 2.0,
    };
    let covered =
        hit_test_impl(&under_handle, covered_point.clone(), deadline()).expect("covered probe");
    match covered {
        HitTestResult::InterceptedBy { role, .. } => {
            assert!(role.is_some(), "occluder role is always present");
        }
        HitTestResult::Unknown => {
            let owner = win32_root_at(&covered_point);
            assert_ne!(
                owner,
                over.handle(),
                "Win32 independently names the over fixture at the covered point, so Unknown is a hit-test regression and not z-order contamination"
            );
            if owner == under.handle() {
                eprintln!(
                    "skip cross-window InterceptedBy: Win32 still names the under fixture at the covered point, so the overlap never staged"
                );
            } else {
                eprintln!(
                    "{}",
                    foreign_owner_skip("cross-window InterceptedBy", owner)
                );
            }
            fixture_overlay::clear_topmost(over.handle());
            return;
        }
        other => panic!("cross-window cover must InterceptedBy, got {other:?}"),
    }

    let (clean_left, clean_top) = fixture_window::on_screen_origin();
    unsafe {
        SetWindowPos(
            over.handle() as *mut std::ffi::c_void,
            HWND_TOPMOST,
            clean_left,
            clean_top,
            420,
            320,
            SWP_SHOWWINDOW,
        );
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    let over_handle = control_handle(&over).expect("over button");
    let over_bounds = window_bounds(fixture_window::find_button(over.handle()));
    let uncovered_point = Point {
        x: over_bounds.x + over_bounds.width / 2.0,
        y: over_bounds.y + over_bounds.height / 2.0,
    };
    let uncovered =
        hit_test_impl(&over_handle, uncovered_point.clone(), deadline()).expect("uncovered probe");
    let owner = win32_root_at(&uncovered_point);
    if owner == over.handle() {
        assert_eq!(
            uncovered,
            HitTestResult::ReachesTarget,
            "the window manager gives the point to the uncovered fixture, so nothing occludes it"
        );
    } else {
        eprintln!(
            "{}",
            foreign_owner_skip(
                "uncovered ReachesTarget (covered InterceptedBy already proven)",
                owner
            )
        );
    }
    fixture_overlay::clear_topmost(over.handle());
}

/// The independent second opinion, read by the test rather than taken from the
/// code under test: a soft skip is only honest when something outside
/// `hit_test` agrees the point belongs to a window this repository does not own.
fn win32_root_at(point: &Point) -> isize {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, WindowFromPoint};

    let physical = POINT {
        x: point.x as i32,
        y: point.y as i32,
    };
    let hwnd = unsafe { WindowFromPoint(physical) };
    if hwnd.is_null() {
        return 0;
    }
    unsafe { GetAncestor(hwnd, GA_ROOT) as isize }
}

fn foreign_owner_skip(pin: &str, owner: isize) -> String {
    format!(
        "skip {pin}: Win32 names window {owner:#x} at the point, which is no fixture of this repository — foreign z-order contamination"
    )
}

fn control_handle(fixture: &LocalFixture) -> Result<NativeHandle, AdapterError> {
    let _ = root_from_hwnd(fixture.handle(), deadline())?;
    let button = fixture_window::find_button(fixture.handle());
    assert!(!button.is_null(), "fixture exposes a BUTTON");
    let client = automation_client()?;
    let element = client
        .element_from_handle(Handle::from(button as isize))
        .map_err(|error| {
            crate::tree::automation::uia_error(&error, "resolve the fixture button")
        })?;
    Ok(UIAElement::from(element).into_native_handle())
}

fn window_bounds(hwnd: *mut std::ffi::c_void) -> Rect {
    let mut rect = windows_sys::Win32::Foundation::RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetWindowRect(hwnd, &mut rect) };
    Rect {
        x: f64::from(rect.left),
        y: f64::from(rect.top),
        width: f64::from(rect.right - rect.left),
        height: f64::from(rect.bottom - rect.top),
    }
}
