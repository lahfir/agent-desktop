use super::Cli;
use agent_desktop_core::context::WaitSelector;
use clap::{CommandFactory, Parser};

fn selector(query_raw: &str) -> WaitSelector {
    WaitSelector {
        query_raw: query_raw.into(),
        gone: false,
        timeout_ms: 30_000,
    }
}

#[test]
fn help_lists_global_wait_for_flags() {
    let help = Cli::command().render_help();
    let help = help.to_string();
    assert!(help.contains("--wait-for"));
    assert!(help.contains("--wait-for-gone"));
    assert!(help.contains("--wait-timeout"));
}

#[test]
fn wait_for_and_wait_for_gone_conflict() {
    let err = Cli::try_parse_from([
        "agent-desktop",
        "--wait-for",
        "button:OK",
        "--wait-for-gone",
        "button:Spinner",
        "snapshot",
        "--app",
        "Finder",
    ])
    .expect_err("mutually exclusive wait flags must fail parse");
    assert_eq!(err.exit_code(), 2);
}

#[test]
fn short_w_flag_maps_to_wait_for() {
    let cli = Cli::try_parse_from([
        "agent-desktop",
        "-w",
        "button:Submit",
        "snapshot",
        "--app",
        "Finder",
    ])
    .expect("short -w should parse");
    assert_eq!(cli.wait_for.as_deref(), Some("button:Submit"));
    assert_eq!(cli.wait_timeout, 30_000);
}

#[test]
fn wait_timeout_parses_custom_value() {
    let cli = Cli::try_parse_from([
        "agent-desktop",
        "--wait-for",
        "button:OK",
        "--wait-timeout",
        "5000",
        "snapshot",
    ])
    .expect("custom wait timeout should parse");
    assert_eq!(cli.wait_timeout, 5_000);
}

#[test]
fn validate_rejects_unsupported_command() {
    let err = crate::validate_wait_for_command("find", &selector("button:OK"))
        .expect_err("find must not accept --wait-for");
    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn validate_rejects_match_everything_selector_before_dispatch() {
    let err = crate::validate_wait_for_command("click", &selector(""))
        .expect_err("empty selector must be rejected before the action runs");
    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(crate::validate_wait_for_command("click", &selector(":")).is_err());
}

#[test]
fn validate_accepts_supported_command_with_constraining_selector() {
    assert!(crate::validate_wait_for_command("click", &selector(":Saved!")).is_ok());
    assert!(crate::validate_wait_for_command("snapshot", &selector("button")).is_ok());
}

#[test]
fn wait_supported_names_are_real_subcommands() {
    let cmd = Cli::command();
    let subcommands: Vec<String> = cmd
        .get_subcommands()
        .map(|sub| sub.get_name().to_string())
        .collect();
    for name in crate::WAIT_SUPPORTED {
        assert!(
            subcommands.iter().any(|sub| sub == name),
            "WAIT_SUPPORTED entry '{name}' is not a real subcommand"
        );
    }
}
