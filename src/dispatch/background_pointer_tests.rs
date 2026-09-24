use super::super::dispatch;
use crate::cli::Commands;
use crate::cli_args::actions::{HoverArgs, MouseClickArgs, MouseMoveArgs};
use crate::dispatch::test_support::{BackgroundPointerAdapter, HomeGuard};
use agent_desktop_core::{PermissionReport, context::CommandContext};
use clap::Parser;

fn run(
    command: Commands,
    headed: bool,
) -> (
    BackgroundPointerAdapter,
    Result<serde_json::Value, agent_desktop_core::AppError>,
) {
    let adapter = BackgroundPointerAdapter::new();
    let context = CommandContext::default().with_headed(headed);
    let result = dispatch(command, &adapter, &PermissionReport::default(), &context);
    (adapter, result)
}

fn mouse_click(argv: &[&str]) -> Commands {
    Commands::MouseClick(MouseClickArgs::try_parse_from(argv).unwrap())
}

#[test]
fn background_mouse_click_routes_to_background_delivery_only() {
    let _home = HomeGuard::new();
    let (adapter, result) = run(
        mouse_click(&[
            "mouse-click",
            "--background",
            "--window-id",
            BackgroundPointerAdapter::WINDOW_ID,
            "--xy",
            "10,20",
        ]),
        false,
    );

    let value = result.unwrap();
    assert_eq!(value["clicked"], true);
    assert_eq!(
        value["background"]["window_id"],
        BackgroundPointerAdapter::WINDOW_ID
    );
    let delivered = adapter.background.lock().unwrap();
    assert_eq!(delivered.len(), 1);
    assert_eq!(
        (delivered[0].1.point.x, delivered[0].1.point.y),
        (10.0, 20.0)
    );
    assert_eq!(*adapter.real_mouse_events.lock().unwrap(), 0);
}

#[test]
fn background_mouse_move_routes_to_background_delivery() {
    let _home = HomeGuard::new();
    let (adapter, result) = run(
        Commands::MouseMove(
            MouseMoveArgs::try_parse_from([
                "mouse-move",
                "--background",
                "--window-id",
                BackgroundPointerAdapter::WINDOW_ID,
                "--xy",
                "10,20",
            ])
            .unwrap(),
        ),
        false,
    );

    assert_eq!(result.unwrap()["moved"], true);
    assert_eq!(adapter.background.lock().unwrap().len(), 1);
}

#[test]
fn default_mouse_click_keeps_the_headless_policy_denial() {
    let _home = HomeGuard::new();
    let (adapter, result) = run(mouse_click(&["mouse-click", "--xy", "10,20"]), false);

    assert_eq!(result.unwrap_err().code(), "POLICY_DENIED");
    assert!(adapter.background.lock().unwrap().is_empty());
}

#[test]
fn background_xy_without_window_id_is_rejected() {
    let _home = HomeGuard::new();
    let (adapter, result) = run(
        mouse_click(&["mouse-click", "--background", "--xy", "10,20"]),
        false,
    );

    let error = result.unwrap_err();
    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(error.to_string().contains("--window-id"));
    assert!(adapter.background.lock().unwrap().is_empty());
}

#[test]
fn window_id_without_background_is_rejected() {
    let _home = HomeGuard::new();
    let (adapter, result) = run(
        mouse_click(&["mouse-click", "--window-id", "w-9555", "--xy", "10,20"]),
        true,
    );

    assert_eq!(result.unwrap_err().code(), "INVALID_ARGS");
    assert!(adapter.background.lock().unwrap().is_empty());
    assert_eq!(*adapter.real_mouse_events.lock().unwrap(), 0);
}

#[test]
fn background_with_headed_is_rejected() {
    let _home = HomeGuard::new();
    let (adapter, result) = run(
        mouse_click(&[
            "mouse-click",
            "--background",
            "--window-id",
            BackgroundPointerAdapter::WINDOW_ID,
            "--xy",
            "10,20",
        ]),
        true,
    );

    let error = result.unwrap_err();
    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(error.to_string().contains("--headed"));
    assert!(adapter.background.lock().unwrap().is_empty());
    assert_eq!(*adapter.real_mouse_events.lock().unwrap(), 0);
}

#[test]
fn background_hover_rejects_window_id_alongside_a_ref() {
    let _home = HomeGuard::new();
    let (adapter, result) = run(
        Commands::Hover(
            HoverArgs::try_parse_from(["hover", "@s1:e1", "--background", "--window-id", "w-9555"])
                .unwrap(),
        ),
        false,
    );

    assert_eq!(result.unwrap_err().code(), "INVALID_ARGS");
    assert!(adapter.background.lock().unwrap().is_empty());
}

#[test]
fn background_hover_by_xy_routes_to_background_delivery() {
    let _home = HomeGuard::new();
    let (adapter, result) = run(
        Commands::Hover(
            HoverArgs::try_parse_from([
                "hover",
                "--background",
                "--window-id",
                BackgroundPointerAdapter::WINDOW_ID,
                "--xy",
                "30,40",
            ])
            .unwrap(),
        ),
        false,
    );

    assert_eq!(result.unwrap()["hovered"], true);
    assert_eq!(adapter.background.lock().unwrap().len(), 1);
}
