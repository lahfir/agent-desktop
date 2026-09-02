use super::update;
use agent_desktop_core::{
    CursorOverlayConfig, CursorOverlayControl, CursorOverlayInstruction, CursorOverlayStyle,
    CursorPhase, ErrorCode, Point,
};

fn session() -> String {
    format!("s{:07}", std::process::id() % 10_000_000)
}

fn travel() -> CursorOverlayControl {
    let config =
        CursorOverlayConfig::enabled(Some("open the file".into()), 6).expect("an enabled config");
    let instruction = CursorOverlayInstruction::new(Point { x: 10.0, y: 10.0 }, &config, false)
        .expect("a valid instruction")
        .with_phase(CursorPhase::Travel);
    CursorOverlayControl::present(session(), instruction)
}

/// A `Disable` against a session with no renderer answers `Ok` without
/// starting one. Spawning here would start a renderer in order to tell it to
/// stop, and the same path serves `session end`.
#[test]
fn a_disable_with_no_renderer_running_starts_nothing_and_succeeds() {
    let before = renderer_count();

    update(&CursorOverlayControl::disable(session())).expect("a disable with nothing to disable");

    assert_eq!(
        renderer_count(),
        before,
        "a disable must never bring a renderer into existence"
    );
}

/// Dispatch sends `Hide` before and `Show` after every mutating command in a
/// headed overlay-enabled session. Without this refusal such a session would
/// fork a detached renderer per command.
#[test]
fn a_hide_or_show_with_no_renderer_running_starts_nothing() {
    let before = renderer_count();

    update(&CursorOverlayControl::hide(session())).expect("a hide with nothing to hide");
    update(&CursorOverlayControl::show(session())).expect("a show with nothing to show");

    assert_eq!(renderer_count(), before);
}

/// The spawn guard refuses from any image but the CLI's own, so a test binary
/// and an FFI host never fork a renderer. It refuses with `Err`, not `Ok`,
/// because dispatch turns `Ok` into `rendered: true` - which would claim an
/// overlay that was never started.
#[test]
fn a_control_that_may_spawn_refuses_from_a_test_binary_rather_than_claiming_success() {
    let error = update(&travel()).expect_err("a test binary is not the agent-desktop image");

    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert!(
        error
            .platform_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("agent-desktop")),
        "the refusal names the image it expected, so a reader is not left guessing"
    );
}

#[test]
fn an_enable_refuses_from_a_test_binary_for_the_same_reason() {
    let error = update(&CursorOverlayControl::enable(
        session(),
        CursorOverlayStyle::default(),
    ))
    .expect_err("a test binary is not the agent-desktop image");

    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
}

/// Counts processes carrying the overlay's argv token, which is how anything
/// outside the child finds it: a process's environment block is not readable
/// from outside, and this child's command line would otherwise be bare.
fn renderer_count() -> usize {
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("cmd")
            .args([
                "/c",
                "wmic process where \"name='agent-desktop.exe'\" get commandline 2>nul",
            ])
            .output();
        let Ok(output) = output else { return 0 };
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| line.contains(super::pipe_name::CHILD_ARGV_FLAG))
            .count()
    }
    #[cfg(not(target_os = "windows"))]
    0
}
