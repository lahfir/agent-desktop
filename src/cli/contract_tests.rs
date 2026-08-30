use crate::cli::{Cli, Commands};
use clap::{CommandFactory, Parser, error::ErrorKind};

#[test]
fn standard_version_flag_reports_the_package_version() {
    let error = Cli::try_parse_from(["agent-desktop", "--version"])
        .expect_err("--version should use clap's display-version path");

    assert_eq!(error.kind(), ErrorKind::DisplayVersion);
    assert_eq!(
        error.to_string().trim(),
        format!("agent-desktop {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn cursor_overlay_enable_is_session_scoped_and_bounded() {
    let cli = Cli::try_parse_from([
        "agent-desktop",
        "--session",
        "run-1",
        "cursor-overlay",
        "enable",
        "--label",
        "Opening the profile menu for this account now",
        "--max-words",
        "5",
        "--size",
        "2.5",
        "--no-ripple",
    ])
    .expect("valid cursor overlay command");
    let Commands::CursorOverlay(args) = cli.command.expect("command") else {
        panic!("expected cursor overlay command");
    };
    let crate::cli_args::cursor_overlay_action::CursorOverlayAction::Enable(args) = args.action
    else {
        panic!("expected enable action");
    };
    let config = args.to_core().expect("valid cursor config");

    assert_eq!(cli.session.as_deref(), Some("run-1"));
    assert!(config.is_enabled());
    assert_eq!(config.label(), Some("Opening the profile menu for…"));
    assert_eq!(config.style().size(), 2.5);
    assert!(!config.style().ripple());
    assert!(config.style().highlight());
}

#[test]
fn cursor_overlay_disable_is_a_dedicated_command() {
    let cli = Cli::try_parse_from([
        "agent-desktop",
        "--session",
        "run-1",
        "cursor-overlay",
        "disable",
    ])
    .expect("valid disable command");

    let Commands::CursorOverlay(args) = cli.command.expect("command") else {
        panic!("expected cursor overlay command");
    };
    assert!(matches!(
        args.action,
        crate::cli_args::cursor_overlay_action::CursorOverlayAction::Disable
    ));
}

#[test]
fn action_commands_reject_the_removed_cursor_flag() {
    let error = Cli::try_parse_from(["agent-desktop", "--agent-cursor", "on", "click", "@e1"])
        .expect_err("cursor configuration is not an action flag");

    assert_eq!(error.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn ci_compares_the_release_binary_to_the_workspace_version() {
    let workflow = include_str!("../../.github/workflows/ci.yml");

    assert!(workflow.contains("PACKAGE_ID=$(cargo pkgid -p agent-desktop)"));
    assert!(workflow.contains("EXPECTED_VERSION=${PACKAGE_ID##*@}"));
    assert!(workflow.contains("EXPECTED_OUTPUT=\"agent-desktop $EXPECTED_VERSION\""));
    assert!(workflow.contains("[ \"$ACTUAL_OUTPUT\" != \"$EXPECTED_OUTPUT\" ]"));
}

#[test]
fn ci_windows_lane_gates_the_full_package_surface() {
    let workflow = include_str!("../../.github/workflows/ci.yml").replace("\r\n", "\n");

    assert!(workflow.contains(
        "cargo clippy --locked -p agent-desktop-core -p agent-desktop-windows \
         -p agent-desktop -p agent-desktop-ffi --all-targets -- -D warnings"
    ));
    assert!(workflow.contains("--edges', 'normal,build,dev"));
    assert!(workflow.contains("run: cargo test --locked -p agent-desktop\n"));
    assert!(workflow.contains("run: cargo test --locked -p agent-desktop-ffi --tests"));
    assert!(
        workflow.contains("run: cargo test --locked -p agent-desktop-windows --examples"),
        "the census redaction guard lives in an example, which a --lib run skips"
    );
    assert!(workflow.contains("expected exactly 2 windows cfg shims"));
    assert!(workflow.contains("Get-Item target/release/agent-desktop.exe"));
    assert!(workflow.contains("ORIGINAL_USERPROFILE=$env:USERPROFILE"));
    assert!(workflow.contains("Guard profile isolation"));
    assert!(workflow.contains("FAIL: HOME is not under RUNNER_TEMP"));
}

#[test]
fn every_visible_cli_argument_has_help_text() {
    fn inspect(command: &clap::Command) {
        for argument in command
            .get_arguments()
            .filter(|argument| !argument.is_hide_set())
        {
            assert!(
                argument
                    .get_help()
                    .is_some_and(|help| !help.to_string().is_empty()),
                "{} argument {:?} has no help text",
                command.get_name(),
                argument.get_id()
            );
        }
        for child in command.get_subcommands() {
            inspect(child);
        }
    }

    inspect(&Cli::command());
}

#[test]
fn curated_help_exposes_new_surfaces_and_current_ref_contract() {
    let help = Cli::command().render_long_help().to_string();
    for expected in [
        "mouse-wheel",
        "list-displays",
        "--file-url",
        "wait --event <kind>",
        "@s8f3k2p9:e1",
        "does not activate",
        "session-owned refs require the same scope",
    ] {
        assert!(
            help.contains(expected),
            "missing curated help text: {expected}"
        );
    }
    assert!(!help.contains("current-session pointer"));
    assert!(!help.contains("explicit --snapshot IDs do not require it"));
}

#[test]
fn permissions_help_names_the_isolated_request_boundary() {
    let mut command = Cli::command();
    let help = command
        .find_subcommand_mut("permissions")
        .expect("permissions command")
        .render_long_help()
        .to_string();

    assert!(help.contains("Request missing permissions in the bounded isolated helper"));
}

#[test]
fn faq_uses_the_unwind_safe_ffi_build_profile() {
    let faq = include_str!("../../docs/faq.md");

    assert!(faq.contains("cargo build --profile release-ffi -p agent-desktop-ffi"));
    assert!(faq.contains("target/release-ffi/"));
    assert!(!faq.contains("cargo build --release\n# Outputs: libagent_desktop_ffi"));
}

/// `open-system-surface` shares `snapshot --surface`'s vocabulary: the same
/// `Surface` `ValueEnum`, so an unknown token is refused at the clap layer
/// with the argument-parse exit code, before any adapter is reached.
#[test]
fn open_system_surface_rejects_an_unknown_surface_token_at_the_clap_layer() {
    let error = Cli::try_parse_from([
        "agent-desktop",
        "open-system-surface",
        "--surface",
        "not-a-surface",
    ])
    .expect_err("an unknown surface token must not parse");

    assert_eq!(error.kind(), ErrorKind::InvalidValue);
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn open_system_surface_requires_a_surface_token() {
    let error = Cli::try_parse_from(["agent-desktop", "open-system-surface"])
        .expect_err("opening without naming a surface must not parse");

    assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn open_system_surface_parses_the_shared_surface_vocabulary() {
    let cli = Cli::try_parse_from([
        "agent-desktop",
        "open-system-surface",
        "--surface",
        "action-center",
    ])
    .expect("the kebab-case token of the shared Surface enum parses");

    let Commands::OpenSystemSurface(args) = cli.command.expect("command") else {
        panic!("expected open-system-surface command");
    };
    assert!(matches!(
        args.surface,
        crate::cli_args::Surface::ActionCenter
    ));
}
