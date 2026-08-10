use super::{capture_window, fail_after_alloc, gdi_balance};
use crate::system::png_codec::decode_png_to_bgra;
use crate::tree::fixture::{LocalPatternFixture, StalledFixture, bootstrap};
use agent_desktop_core::{Deadline, ErrorCode};
use std::time::Duration;

fn deadline() -> Deadline {
    Deadline::after(10_000).expect("capture tests use a generous deadline")
}

fn sample_rgb(bgra: &[u8], width: u32, x: i32, y: i32) -> [u8; 3] {
    let offset = ((y as u32 * width + x as u32) * 4) as usize;
    [bgra[offset + 2], bgra[offset + 1], bgra[offset]]
}

#[test]
fn pattern_fixture_capture_matches_sampled_colours() {
    bootstrap();
    let fixture = LocalPatternFixture::create().expect("pattern fixture starts");
    let image = capture_window(fixture.handle() as _, 1.0, deadline())
        .expect("PrintWindow capture of the pattern fixture succeeds");

    let (bgra, width, height) =
        decode_png_to_bgra(&image.data, deadline()).expect("decode captured PNG");
    assert_eq!((width, height), (image.width, image.height));

    let expectation = fixture.expectation();
    let mut samples = [[0u8; 3]; 4];
    for (index, point) in expectation.sample_points().into_iter().enumerate() {
        samples[index] = sample_rgb(&bgra, width, point.x, point.y);
    }
    assert!(
        expectation.matches_samples(&samples),
        "captured samples {samples:?} must match {:?}",
        expectation.sample_points()
    );
}

#[test]
fn stalled_fixture_returns_app_unresponsive_without_hanging() {
    bootstrap();
    let stalled = StalledFixture::create().expect("stalled fixture starts");
    let started = std::time::Instant::now();
    let error = capture_window(stalled.handle() as _, 1.0, deadline())
        .expect_err("a non-pumping window must be refused before PrintWindow");
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "the pump probe must bound the refusal, elapsed {:?}",
        started.elapsed()
    );
}

/// Callers corroborate identity before [`capture_window`]. This module then
/// refuses a destroyed handle before the pump probe, so a gone window is
/// reported as a stale-identity style miss rather than `APP_UNRESPONSIVE`.
#[test]
fn destroyed_handle_is_not_reported_unresponsive() {
    bootstrap();
    let fixture = LocalPatternFixture::create().expect("pattern fixture starts");
    let handle = fixture.handle();
    drop(fixture);
    assert!(
        !crate::tree::automation::window_exists(handle),
        "fixture drop must destroy the window before the capture call"
    );

    let error = capture_window(handle as _, 1.0, deadline())
        .expect_err("a destroyed handle must fail closed");
    assert_ne!(
        error.code,
        ErrorCode::AppUnresponsive,
        "existence is checked before the pump probe"
    );
    assert_eq!(error.code, ErrorCode::WindowNotFound);
}

#[test]
fn zero_area_and_minimized_windows_are_rejected_before_bitmap_alloc() {
    bootstrap();
    gdi_balance::reset();
    let fixture = LocalPatternFixture::create().expect("pattern fixture starts");
    let handle = fixture.handle() as windows_sys::Win32::Foundation::HWND;

    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
            handle,
            windows_sys::Win32::UI::WindowsAndMessaging::SW_MINIMIZE,
        );
    }
    std::thread::sleep(Duration::from_millis(50));

    let before = gdi_balance::live();
    let error = capture_window(handle, 1.0, deadline())
        .expect_err("minimized windows are rejected before PrintWindow");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(
        gdi_balance::live(),
        before,
        "rejection must not allocate GDI objects"
    );
}

#[test]
fn gdi_objects_balance_across_success_deadline_and_forced_failure() {
    bootstrap();
    gdi_balance::reset();
    let fixture = LocalPatternFixture::create().expect("pattern fixture starts");
    let handle = fixture.handle() as _;

    let _ = capture_window(handle, 1.0, deadline()).expect("success path");
    assert_eq!(
        gdi_balance::live(),
        0,
        "success path must release every GDI object"
    );

    let expired = Deadline::after(1).expect("tiny deadline");
    std::thread::sleep(Duration::from_millis(5));
    let timeout = capture_window(handle, 1.0, expired).expect_err("expired deadline");
    assert_eq!(timeout.code, ErrorCode::Timeout);
    assert_eq!(
        gdi_balance::live(),
        0,
        "early deadline abort allocates nothing"
    );

    let forced = fail_after_alloc::with(|| capture_window(handle, 1.0, deadline()))
        .expect_err("forced failure after allocation");
    assert_eq!(forced.code, ErrorCode::ActionFailed);
    assert_eq!(
        gdi_balance::live(),
        0,
        "forced failure must still Drop every GDI object"
    );
}
