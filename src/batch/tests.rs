use super::*;
use crate::cli_args::Surface;
use agent_desktop_core::{PermissionReport, adapter::PlatformAdapter};
use clap::CommandFactory;

struct NoopAdapter;
impl PlatformAdapter for NoopAdapter {}

fn item(command: &str, args: Value) -> BatchCommand {
    BatchCommand {
        command: command.to_string(),
        session: None,
        args,
    }
}

#[test]
fn parses_optional_batch_session_scope() {
    let commands = agent_desktop_core::commands::batch::parse_commands(
        r#"[{"command":"status","session":"agent-a","args":{}}]"#,
    )
    .unwrap();

    assert_eq!(commands[0].session.as_deref(), Some("agent-a"));
}

#[test]
fn parses_ref_command_into_cli_enum() {
    let command =
        parse_command(item("click", serde_json::json!({ "ref_id": "@e1" }))).expect("click parses");

    match command {
        Commands::Click(args) => {
            assert_eq!(args.ref_id, "@e1");
            assert_eq!(args.snapshot_id, None);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn applies_cli_defaults_during_batch_decode() {
    let command = parse_command(item("snapshot", serde_json::json!({}))).expect("snapshot parses");

    match command {
        Commands::Snapshot(args) => {
            assert_eq!(args.max_depth, 10);
            assert!(!args.interactive_only);
            assert!(matches!(args.surface, Surface::Window));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn rejects_unknown_batch_args() {
    let err = parse_command(item(
        "click",
        serde_json::json!({ "ref_id": "@e1", "x": 1 }),
    ))
    .expect_err("unknown field is rejected");

    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn rejects_unknown_wait_batch_args_after_flattening() {
    let err = parse_command(item(
        "wait",
        serde_json::json!({ "ms": 1, "unexpected": true }),
    ))
    .expect_err("unknown wait field is rejected");

    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn rejects_version_args_after_json_flag_removal() {
    let err = parse_command(item("version", serde_json::json!({ "json": true })))
        .expect_err("version no longer accepts args");

    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn stop_on_error_halts_after_first_failure() {
    let args = BatchArgs {
        commands_json: serde_json::json!([
            {"command": "missing", "args": {}},
            {"command": "version", "args": {}}
        ])
        .to_string(),
        stop_on_error: true,
    };

    let value = execute(
        args,
        &NoopAdapter,
        &PermissionReport::default(),
        &agent_desktop_core::CommandContext::default(),
    )
    .unwrap();
    let results = value["results"].as_array().unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["ok"], false);
    assert_eq!(results[0]["version"], ENVELOPE_VERSION);
    assert_eq!(results[0]["command"], "missing");
    assert_eq!(results[0]["error"]["code"], "INVALID_ARGS");
    assert!(
        results[0]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Unknown batch command")
    );
}

#[test]
fn no_args_rejection_has_suggestion() {
    let err = parse_command(item("status", serde_json::json!({"x": 1})))
        .expect_err("status rejects args");

    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(err.suggestion().is_some());
}

#[test]
fn nested_batch_rejection_has_suggestion() {
    let err = parse_command(item("batch", serde_json::json!({}))).expect_err("batch rejected");

    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(err.suggestion().is_some());
}

#[test]
fn every_cli_subcommand_is_known_to_batch_parser() {
    for subcommand in crate::cli::Cli::command().get_subcommands() {
        let name = subcommand.get_name();
        let result = parse_command(item(name, serde_json::json!({})));
        if name == "batch" {
            assert!(result.unwrap_err().to_string().contains("nested"));
            continue;
        }
        if let Err(err) = result {
            assert!(
                !err.to_string().contains("Unknown batch command"),
                "{name} is missing from batch parser"
            );
        }
    }
}
