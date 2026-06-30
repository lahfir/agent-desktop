use super::Cli;
use clap::{CommandFactory, Parser};

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
