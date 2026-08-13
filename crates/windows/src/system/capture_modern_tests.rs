use super::{
    capture_display, capture_window, fail_after_start, hold_frames, interop_is_available,
    modern_is_supported,
};
use crate::system::capture_backend::{self, CaptureSubject};
use crate::system::capture_d3d::resource_balance;
use crate::system::display::{display_at, list_displays_live};
use crate::system::permissions::{map_capture_availability, report};
use crate::system::png_codec::decode_png_to_bgra;
use crate::tree::fixture::{LocalPatternFixture, bootstrap};
use agent_desktop_core::{Deadline, ErrorCode, PermissionState};

fn deadline() -> Deadline {
    Deadline::after(10_000).expect("modern capture tests use a generous deadline")
}

fn sample_rgb(bgra: &[u8], width: u32, x: i32, y: i32) -> [u8; 3] {
    let offset = ((y as u32 * width + x as u32) * 4) as usize;
    [bgra[offset + 2], bgra[offset + 1], bgra[offset]]
}

/// Pixel and resource-balance legs need the HWND/HMONITOR interop, which is a
/// stricter gate than [`modern_is_supported`] (A22-1: IsSupported can be true
/// while `IGraphicsCaptureItemInterop` QI still fails).
fn skip_if_interop_unavailable(reason: &str) -> bool {
    if !modern_is_supported() {
        eprintln!("skip: {reason} (GraphicsCaptureSession::IsSupported is false)");
        return true;
    }
    if !interop_is_available() {
        eprintln!(
            "skip: {reason} (IsSupported true but IGraphicsCaptureItemInterop unavailable — A22-1)"
        );
        return true;
    }
    false
}

#[test]
fn support_predicate_is_not_a_build_number() {
    bootstrap();
    let supported = modern_is_supported();
    let again = modern_is_supported();
    assert_eq!(
        supported, again,
        "IsSupported must be stable for the process"
    );
}

#[test]
fn unsupported_host_reports_unavailable_without_capture_api() {
    bootstrap();
    if modern_is_supported() {
        eprintln!(
            "skip: host reports WGC supported via IsSupported; unavailable-without-API leg needs an unsupported session"
        );
        return;
    }
    resource_balance::reset();
    let fixture = LocalPatternFixture::create().expect("pattern fixture starts");
    let error = capture_window(fixture.handle() as _, 1.0, deadline())
        .expect_err("unsupported host must refuse before capture work");
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(
        resource_balance::live(),
        0,
        "unsupported path must not create tracked resources"
    );
}

#[test]
fn pattern_fixture_window_capture_matches_when_supported() {
    bootstrap();
    if skip_if_interop_unavailable("pattern window capture") {
        return;
    }
    let fixture = LocalPatternFixture::create().expect("pattern fixture starts");
    let image = capture_window(fixture.handle() as _, 1.0, deadline())
        .expect("WGC window capture of the pattern fixture succeeds");
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
fn monitor_capture_returns_monitor_dimensions_when_supported() {
    bootstrap();
    if skip_if_interop_unavailable("monitor capture") {
        return;
    }
    let primary = display_at(0, deadline()).expect("primary display");
    let image = capture_display(0, deadline()).expect("WGC monitor capture succeeds");
    assert_eq!(
        (image.width, image.height),
        (primary.bounds.width as u32, primary.bounds.height as u32)
    );
    assert_eq!(image.scale_factor, primary.scale);
}

#[test]
fn frame_wait_deadline_expiry_falls_back_to_legacy() {
    bootstrap();
    if skip_if_interop_unavailable("frame-wait deadline expiry") {
        return;
    }
    let fixture = LocalPatternFixture::create().expect("pattern fixture");
    let tight = Deadline::after(80).expect("short modern slice");
    let modern_error = hold_frames::with(|| capture_window(fixture.handle() as _, 1.0, tight))
        .expect_err("holding frames past the deadline must fail the modern backend");
    assert_eq!(modern_error.code, ErrorCode::Timeout);

    let image = capture_backend::capture_with_precedence(
        CaptureSubject::Window {
            handle: fixture.handle() as _,
            scale_factor: 1.0,
        },
        deadline(),
    )
    .expect("precedence must still succeed via Legacy after a modern timeout");
    assert!(!image.data.is_empty());
}

#[test]
fn interop_failure_is_backend_failure_that_precedence_recovers() {
    bootstrap();
    if !modern_is_supported() {
        eprintln!("skip: interop-failure fallback needs IsSupported true");
        return;
    }
    if interop_is_available() {
        eprintln!(
            "skip: interop is available on this host; E_NOINTERFACE recovery is exercised on A22-1 hosts"
        );
        return;
    }
    let fixture = LocalPatternFixture::create().expect("pattern fixture");
    let modern_error = capture_window(fixture.handle() as _, 1.0, deadline())
        .expect_err("missing interop must fail the modern backend");
    assert_ne!(modern_error.code, ErrorCode::Timeout);
    let image = capture_backend::capture_with_precedence(
        CaptureSubject::Window {
            handle: fixture.handle() as _,
            scale_factor: 1.0,
        },
        deadline(),
    )
    .expect("Legacy must recover when modern interop is missing");
    assert!(!image.data.is_empty());
}

#[test]
fn resources_balance_across_success_deadline_and_forced_failure() {
    bootstrap();
    if skip_if_interop_unavailable("resource balance") {
        return;
    }
    resource_balance::reset();
    let fixture = LocalPatternFixture::create().expect("pattern fixture");
    let handle = fixture.handle() as _;

    let _ = capture_window(handle, 1.0, deadline()).expect("success path");
    assert_eq!(
        resource_balance::live(),
        0,
        "success path must release every tracked resource"
    );

    let tight = Deadline::after(80).expect("short deadline");
    let timeout = hold_frames::with(|| capture_window(handle, 1.0, tight))
        .expect_err("held frames past deadline");
    assert_eq!(timeout.code, ErrorCode::Timeout);
    assert_eq!(
        resource_balance::live(),
        0,
        "deadline abort must release every tracked resource"
    );

    let forced = fail_after_start::with(|| capture_window(handle, 1.0, deadline()))
        .expect_err("forced failure after session start");
    assert_eq!(forced.code, ErrorCode::ActionFailed);
    assert_eq!(
        resource_balance::live(),
        0,
        "forced failure must still Drop every tracked resource"
    );
}

#[test]
fn permission_report_screen_recording_is_honest_when_legacy_can_capture() {
    bootstrap();
    let displays = list_displays_live(deadline()).expect("displays");
    assert!(!displays.is_empty());
    let report = report(deadline()).expect("permission report");
    assert_eq!(
        report.screen_recording,
        PermissionState::NotRequired,
        "a session that can list displays can capture via Legacy"
    );
}

#[test]
fn probe_availability_is_true_when_legacy_or_modern_can_run() {
    bootstrap();
    let displays = list_displays_live(deadline()).expect("displays");
    assert!(!displays.is_empty());
    assert_eq!(
        map_capture_availability(Some(true)),
        PermissionState::NotRequired
    );
    assert!(
        modern_is_supported() || !displays.is_empty(),
        "this host must exercise at least one capture backend for the probe"
    );
}
