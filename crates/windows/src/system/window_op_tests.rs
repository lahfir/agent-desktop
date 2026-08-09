use super::*;
use agent_desktop_core::{DeliveryDisposition, InteractionLease, ProcessId, WindowState};

#[test]
fn geometry_rejects_non_finite_and_extreme_values() {
    assert!(validate_size(f64::NAN, 100.0).is_err());
    assert!(validate_size(100.0, f64::INFINITY).is_err());
    assert!(validate_size(0.0, 100.0).is_err());
    assert!(validate_size(-10.0, 100.0).is_err());
    assert!(validate_size(1_000_001.0, 100.0).is_err());
    assert!(validate_position(f64::NEG_INFINITY, 0.0).is_err());
    assert!(validate_position(1_000_001.0, 0.0).is_err());
}

#[test]
fn geometry_accepts_negative_screen_coordinates() {
    assert!(validate_position(-1920.0, 0.0).is_ok());
    assert!(validate_position(0.0, -1080.0).is_ok());
    assert!(validate_size(1920.0, 1080.0).is_ok());
}

#[cfg(target_os = "windows")]
fn deadline() -> Deadline {
    Deadline::after(10_000).expect("deadline")
}

#[cfg(target_os = "windows")]
fn lease() -> InteractionLease {
    InteractionLease::guarded(deadline(), ()).expect("lease")
}

#[cfg(target_os = "windows")]
fn staged_fixture() -> (crate::tree::fixture::LocalFixture, WindowInfo) {
    crate::tree::fixture::ensure_test_apartment();
    let (left, top) = crate::tree::fixture_window::on_screen_origin();
    let fixture =
        crate::tree::fixture::LocalFixture::create_at(left, top).expect("on-screen fixture");
    let info = window_info_for(fixture.handle());
    (fixture, info)
}

#[cfg(target_os = "windows")]
fn window_info_for(handle: isize) -> WindowInfo {
    let pid = ProcessId::from(std::process::id());
    let token = crate::system::process_identity::token_for_pid(pid)
        .expect("token read")
        .expect("live token");
    let app = crate::system::process_identity::process_image_name(pid).unwrap_or_default();
    WindowInfo {
        id: format!("w-{}", handle as usize),
        title: String::new(),
        app,
        pid,
        process_instance: Some(token),
        bounds: None,
        state: WindowState::default(),
    }
}

#[cfg(target_os = "windows")]
fn current_rect(handle: isize) -> (i32, i32, i32, i32) {
    window_rect(handle as super::super::window_enum::WindowHandle).expect("rect")
}

#[cfg(target_os = "windows")]
fn current_show_cmd(handle: isize) -> u32 {
    placement_show_cmd(handle as super::super::window_enum::WindowHandle).expect("placement")
}

#[cfg(target_os = "windows")]
#[test]
fn resize_is_confirmed_by_get_window_rect_within_tolerance() {
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let (fixture, info) = staged_fixture();
    let (left, top, width, height) = current_rect(fixture.handle());
    let target_w = width + 24;
    let target_h = height + 16;
    window_op_impl(
        &info,
        WindowOp::Resize {
            width: f64::from(target_w),
            height: f64::from(target_h),
        },
        lease().deadline(),
    )
    .expect("resize");
    let (after_left, after_top, after_w, after_h) = current_rect(fixture.handle());
    assert!(within_tolerance(after_w, target_w));
    assert!(within_tolerance(after_h, target_h));
    assert!(within_tolerance(after_left, left));
    assert!(within_tolerance(after_top, top));
}

#[cfg(target_os = "windows")]
#[test]
fn move_is_confirmed_by_get_window_rect_within_tolerance() {
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let (fixture, info) = staged_fixture();
    let (left, top, width, height) = current_rect(fixture.handle());
    let target_x = left + 32;
    let target_y = top + 24;
    window_op_impl(
        &info,
        WindowOp::Move {
            x: f64::from(target_x),
            y: f64::from(target_y),
        },
        lease().deadline(),
    )
    .expect("move");
    let (after_left, after_top, after_w, after_h) = current_rect(fixture.handle());
    assert!(within_tolerance(after_left, target_x));
    assert!(within_tolerance(after_top, target_y));
    assert!(within_tolerance(after_w, width));
    assert!(within_tolerance(after_h, height));
}

#[cfg(target_os = "windows")]
#[test]
fn minimize_is_confirmed_by_show_cmd_not_uia_geometry() {
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWMINIMIZED;

    let _stage = crate::tree::fixture_window::on_screen_stage();
    let (fixture, info) = staged_fixture();
    window_op_impl(&info, WindowOp::Minimize, lease().deadline()).expect("minimize");
    assert_eq!(current_show_cmd(fixture.handle()), SW_SHOWMINIMIZED as u32);
}

#[cfg(target_os = "windows")]
#[test]
fn maximize_is_confirmed_by_show_cmd() {
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWMAXIMIZED;

    let _stage = crate::tree::fixture_window::on_screen_stage();
    let (fixture, info) = staged_fixture();
    window_op_impl(&info, WindowOp::Maximize, lease().deadline()).expect("maximize");
    assert_eq!(current_show_cmd(fixture.handle()), SW_SHOWMAXIMIZED as u32);
}

#[cfg(target_os = "windows")]
#[test]
fn restore_is_confirmed_by_show_cmd() {
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let _stage = crate::tree::fixture_window::on_screen_stage();
    let (fixture, info) = staged_fixture();
    window_op_impl(&info, WindowOp::Minimize, lease().deadline()).expect("minimize");
    window_op_impl(&info, WindowOp::Restore, lease().deadline()).expect("restore");
    assert_eq!(current_show_cmd(fixture.handle()), SW_SHOWNORMAL as u32);
}

#[cfg(target_os = "windows")]
#[test]
fn non_finite_geometry_is_invalid_args_before_native_write() {
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let (fixture, info) = staged_fixture();
    let before = current_rect(fixture.handle());
    let error = window_op_impl(
        &info,
        WindowOp::Resize {
            width: f64::NAN,
            height: 100.0,
        },
        lease().deadline(),
    )
    .expect_err("nan refused");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    let after = current_rect(fixture.handle());
    assert_eq!(after, before);
}

#[cfg(target_os = "windows")]
#[test]
fn negative_move_coordinates_are_accepted_when_finite() {
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let (fixture, info) = staged_fixture();
    let (left, top, _, _) = current_rect(fixture.handle());
    let target_x = left.saturating_sub(48);
    let target_y = top;
    window_op_impl(
        &info,
        WindowOp::Move {
            x: f64::from(target_x),
            y: f64::from(target_y),
        },
        lease().deadline(),
    )
    .expect("negative-capable move");
    let (after_left, after_top, _, _) = current_rect(fixture.handle());
    assert!(within_tolerance(after_left, target_x));
    assert!(within_tolerance(after_top, target_y));
}

#[cfg(target_os = "windows")]
#[test]
fn recycled_hwnd_ownership_mismatch_fails_closed_not_delivered() {
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let (fixture, mut info) = staged_fixture();
    let before = current_rect(fixture.handle());
    info.process_instance = Some("windows-proc-v1:0:0".into());
    let error = window_op_impl(
        &info,
        WindowOp::Resize {
            width: f64::from(before.2 + 40),
            height: f64::from(before.3 + 40),
        },
        lease().deadline(),
    )
    .expect_err("stale generation refused");
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    let after = current_rect(fixture.handle());
    assert_eq!(after, before);
}

#[cfg(target_os = "windows")]
#[test]
fn window_op_refuses_an_expired_deadline_before_any_write() {
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let (fixture, info) = staged_fixture();
    let before = current_rect(fixture.handle());
    let expired = Deadline::after(1).expect("deadline");
    std::thread::sleep(std::time::Duration::from_millis(5));
    let error = window_op_impl(&info, WindowOp::Minimize, expired).expect_err("timeout");
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    let after = current_rect(fixture.handle());
    assert_eq!(after, before);
}
