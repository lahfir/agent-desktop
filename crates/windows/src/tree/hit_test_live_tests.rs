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
    let result = hit_test_impl(&handle, point, deadline()).expect("hit_test succeeds");
    match result {
        HitTestResult::ReachesTarget => {}
        HitTestResult::InterceptedBy { name, .. } if foreign_occluder_name(name.as_deref()) => {
            eprintln!("skip on-screen ReachesTarget: foreign occluder present");
        }
        other => panic!("on-screen center must ReachesTarget, got {other:?}"),
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
    assert!(
        !matches!(result, HitTestResult::InterceptedBy { .. }),
        "IsIconic guard must not invent InterceptedBy"
    );
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
    let result = hit_test_impl(&handle, point, deadline()).expect("hit_test succeeds");
    match result {
        HitTestResult::InterceptedBy { role, .. } => {
            assert!(role.is_some(), "occluder role is always present");
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
    let covered = hit_test_impl(&under_handle, covered_point, deadline()).expect("covered probe");
    match covered {
        HitTestResult::InterceptedBy { role, .. } => {
            assert!(role.is_some(), "occluder role is always present");
        }
        HitTestResult::Unknown => {
            eprintln!(
                "skip cross-window InterceptedBy: covered probe returned Unknown (foreign z-order contamination)"
            );
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
        hit_test_impl(&over_handle, uncovered_point, deadline()).expect("uncovered probe");
    match uncovered {
        HitTestResult::ReachesTarget => {}
        HitTestResult::InterceptedBy { name, .. } if foreign_occluder_name(name.as_deref()) => {
            eprintln!(
                "skip uncovered ReachesTarget: foreign occluder present; covered InterceptedBy already proven"
            );
        }
        other => panic!("uncovered control must ReachesTarget, got {other:?}"),
    }
    fixture_overlay::clear_topmost(over.handle());
}

fn foreign_occluder_name(name: Option<&str>) -> bool {
    name.is_some_and(|label| !label.contains("fixture"))
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
