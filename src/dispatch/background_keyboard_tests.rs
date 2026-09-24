use super::super::dispatch;
use crate::cli::Commands;
use crate::cli_args::actions::{PressArgs, TypeArgs};
use crate::dispatch::test_support::{BackgroundAdapter, HomeGuard};
use agent_desktop_core::{BackgroundKeyInput, PermissionReport, context::CommandContext};
use clap::Parser;

fn run(
    command: Commands,
    headed: bool,
) -> (
    BackgroundAdapter,
    Result<serde_json::Value, agent_desktop_core::AppError>,
) {
    let adapter = BackgroundAdapter::new();
    let context = CommandContext::default().with_headed(headed);
    let result = dispatch(command, &adapter, &PermissionReport::default(), &context);
    (adapter, result)
}

fn press(argv: &[&str]) -> Commands {
    Commands::Press(PressArgs::try_parse_from(argv).unwrap())
}

fn type_text(argv: &[&str]) -> Commands {
    Commands::Type(TypeArgs::try_parse_from(argv).unwrap())
}

#[test]
fn background_press_posts_the_combo_to_the_named_window_only() {
    let _home = HomeGuard::new();
    let (adapter, result) = run(
        press(&[
            "press",
            "cmd+s",
            "--background",
            "--window-id",
            BackgroundAdapter::WINDOW_ID,
        ]),
        false,
    );

    let value = result.unwrap();
    assert_eq!(value["pressed"], true);
    assert_eq!(
        value["background"]["window_id"],
        BackgroundAdapter::WINDOW_ID
    );
    assert_eq!(value["disposition"]["delivery"], "delivered_unverified");
    let delivered = adapter.background_keys.lock().unwrap();
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].0.id, BackgroundAdapter::WINDOW_ID);
    assert!(matches!(&delivered[0].1, BackgroundKeyInput::Combo(combo) if combo.key == "s"));
}

#[test]
fn background_press_needs_a_window_id_and_rejects_app() {
    let _home = HomeGuard::new();
    let cases: [&[&str]; 2] = [
        &["press", "return", "--background"],
        &[
            "press",
            "return",
            "--background",
            "--app",
            "Code",
            "--window-id",
            BackgroundAdapter::WINDOW_ID,
        ],
    ];

    for argv in cases {
        let (adapter, result) = run(press(argv), false);

        assert_eq!(result.unwrap_err().code(), "INVALID_ARGS");
        assert!(adapter.background_keys.lock().unwrap().is_empty());
    }
}

#[test]
fn press_window_id_without_background_is_rejected() {
    let _home = HomeGuard::new();
    let (adapter, result) = run(
        press(&[
            "press",
            "return",
            "--window-id",
            BackgroundAdapter::WINDOW_ID,
        ]),
        false,
    );

    let error = result.unwrap_err();
    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(error.to_string().contains("--background"));
    assert!(adapter.background_keys.lock().unwrap().is_empty());
}

#[test]
fn background_keys_reject_headed_before_delivery() {
    let _home = HomeGuard::new();
    let commands = [
        press(&[
            "press",
            "return",
            "--background",
            "--window-id",
            BackgroundAdapter::WINDOW_ID,
        ]),
        Commands::Type(
            TypeArgs::try_parse_from(["type", "@s1:e1", "hello", "--background"]).unwrap(),
        ),
    ];

    for command in commands {
        let (adapter, result) = run(command, true);

        let error = result.unwrap_err();
        assert_eq!(error.code(), "INVALID_ARGS");
        assert!(error.to_string().contains("--headed"));
        assert!(adapter.background_keys.lock().unwrap().is_empty());
    }
}

#[test]
fn batch_background_keys_share_cli_routing_and_rejections() {
    let _home = HomeGuard::new();
    let adapter = BackgroundAdapter::new();
    let args = crate::cli_args::batch::BatchArgs {
        commands_json: serde_json::json!([
            {"command": "press", "args": {"combo": "cmd+s", "background": true, "window_id": "w-9555"}},
            {"command": "press", "args": {"combo": "return", "background": true}},
            {"command": "press", "args": {"combo": "return", "window_id": "w-9555"}},
            {"command": "type", "args": {"text": "hi", "background": true, "window_id": "w-9555"}},
            {"command": "type", "args": {"text": "hi", "background": true}}
        ])
        .to_string(),
        stop_on_error: false,
        timeout_ms: 60_000,
    };

    let value = crate::batch::execute(
        args,
        &adapter,
        &PermissionReport::default(),
        &agent_desktop_core::CommandContext::default(),
    )
    .unwrap();
    let results = value["results"].as_array().unwrap();

    assert_eq!(results[0]["ok"], true);
    assert_eq!(results[0]["data"]["pressed"], true);
    assert_eq!(results[1]["error"]["code"], "INVALID_ARGS");
    assert_eq!(results[2]["error"]["code"], "INVALID_ARGS");
    assert_eq!(results[3]["ok"], true, "{}", results[3]);
    assert_eq!(results[3]["data"]["typed"], true);
    assert_eq!(results[4]["error"]["code"], "INVALID_ARGS");
    assert_eq!(adapter.background_keys.lock().unwrap().len(), 2);
}

#[test]
fn background_type_with_a_window_id_types_into_that_window_without_a_ref() {
    let _home = HomeGuard::new();
    let (adapter, result) = run(
        type_text(&[
            "type",
            "--background",
            "--window-id",
            BackgroundAdapter::WINDOW_ID,
            "hello there",
        ]),
        false,
    );

    let value = result.unwrap();
    assert_eq!(value["typed"], true);
    assert!(value["background"].get("ax_focus").is_none());
    let delivered = adapter.background_keys.lock().unwrap();
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].0.id, BackgroundAdapter::WINDOW_ID);
    assert!(matches!(&delivered[0].1, BackgroundKeyInput::Text(text) if text == "hello there"));
}

#[test]
fn type_target_combinations_that_name_no_single_window_are_rejected() {
    let _home = HomeGuard::new();
    let cases: [&[&str]; 4] = [
        &[
            "type",
            "@s1:e1",
            "hi",
            "--background",
            "--window-id",
            BackgroundAdapter::WINDOW_ID,
        ],
        &["type", "hi", "--background"],
        &[
            "type",
            "@s1:e1",
            "hi",
            "--window-id",
            BackgroundAdapter::WINDOW_ID,
        ],
        &["type", "hi"],
    ];

    for argv in cases {
        let (adapter, result) = run(type_text(argv), false);

        assert_eq!(result.unwrap_err().code(), "INVALID_ARGS", "{argv:?}");
        assert!(adapter.background_keys.lock().unwrap().is_empty());
    }
}
