use crate::cli::Cli;
use clap::{CommandFactory, Parser, error::ErrorKind};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const NON_COMMAND_MODULES: &[&str] = &[
    "combo",
    "execute_by_ref",
    "find_live",
    "helpers",
    "helpers_test_support",
    "input_hold_policy",
    "launch_cdp_test_support",
    "mod",
    "notification_identity",
    "notification_policy",
    "point_resolve",
    "pointer_action",
    "query",
    "find_live_test_support",
    "surface_scope",
    "stale_retry_test_support",
    "wait_element",
    "wait_latest_ref_cache",
    "wait_mode",
    "wait_predicate",
    "wait_surface",
    "wait_test_support",
    "wait_text_match",
    "wait_timeout",
    "wait_event",
    "wait_event_input",
    "wait_selector",
    "window_target",
];

const COMMAND_SPECIFIC_TESTS: &[&str] = &[
    "find",
    "focus-window",
    "is",
    "list-apps",
    "right-click",
    "skills",
    "session",
    "snapshot",
    "status",
    "trace",
    "wait",
];

const SHARED_REF_ACTION_TESTS: &[&str] = &[
    "check",
    "clear",
    "click",
    "collapse",
    "double-click",
    "expand",
    "focus",
    "right-click",
    "scroll",
    "scroll-to",
    "select",
    "set-value",
    "toggle",
    "triple-click",
    "type",
    "uncheck",
];

const BINARY_CONTRACT_TESTS: &[&str] = &["batch", "permissions", "version"];

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
fn ci_compares_the_release_binary_to_the_workspace_version() {
    let workflow = include_str!("../../.github/workflows/ci.yml");

    assert!(workflow.contains("PACKAGE_ID=$(cargo pkgid -p agent-desktop)"));
    assert!(workflow.contains("EXPECTED_VERSION=${PACKAGE_ID##*@}"));
    assert!(workflow.contains("EXPECTED_OUTPUT=\"agent-desktop $EXPECTED_VERSION\""));
    assert!(workflow.contains("[ \"$ACTUAL_OUTPUT\" != \"$EXPECTED_OUTPUT\" ]"));
}

const ADAPTER_PASSTHROUGH_COMMANDS: &[&str] = &[
    "clipboard-clear",
    "clipboard-get",
    "clipboard-set",
    "close-app",
    "dismiss-all-notifications",
    "dismiss-notification",
    "drag",
    "get",
    "hover",
    "key-down",
    "key-up",
    "launch",
    "list-notifications",
    "list-surfaces",
    "list-windows",
    "list-displays",
    "maximize",
    "minimize",
    "mouse-click",
    "mouse-down",
    "mouse-move",
    "mouse-up",
    "mouse-wheel",
    "move-window",
    "notification-action",
    "press",
    "resize-window",
    "restore",
    "screenshot",
];

#[test]
fn every_cli_subcommand_has_core_command_module() {
    for command in cli_command_names() {
        let module = command_module_name(&command);
        let path = command_module_path(&module);
        assert!(
            path.exists(),
            "{command} is missing core command module {path:?}"
        );
    }
}

#[test]
fn every_core_command_module_is_registered_or_declared_helper() {
    let command_modules: BTreeSet<_> = cli_command_names()
        .into_iter()
        .map(|name| command_module_name(&name))
        .collect();

    for module in command_module_files() {
        if module.ends_with("_tests") {
            continue;
        }
        assert!(
            command_modules.contains(module.as_str())
                || NON_COMMAND_MODULES.contains(&module.as_str()),
            "{module}.rs is not registered as a CLI command module or declared helper"
        );
    }
}

#[test]
fn every_cli_subcommand_has_explicit_test_coverage_classification() {
    let covered = coverage_names();
    for command in cli_command_names() {
        assert!(
            covered.contains(command.as_str()),
            "{command} has no explicit command coverage classification"
        );
    }
}

#[test]
fn macos_capability_count_includes_restored_notifications() {
    let commands = cli_command_names();
    assert_eq!(
        commands.len(),
        58,
        "the published CLI command count changed"
    );
    assert_eq!(
        commands.len(),
        58,
        "macOS operational command count changed; update capability documentation"
    );
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

#[test]
fn core_command_modules_keep_tests_in_sibling_files() {
    for path in command_module_paths() {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if stem.ends_with("_tests") || NON_COMMAND_MODULES.contains(&stem) {
            continue;
        }

        let source = std::fs::read_to_string(&path).expect("command module should be readable");
        assert!(
            !source.contains("mod tests {"),
            "{} keeps inline tests; move them to {stem}_tests.rs and include with #[path]",
            path.display()
        );
    }
}

fn cli_command_names() -> Vec<String> {
    Cli::command()
        .get_subcommands()
        .map(|cmd| cmd.get_name().to_owned())
        .collect()
}

fn command_module_name(command: &str) -> String {
    match command {
        "is" => "is_check".into(),
        "type" => "type_text".into(),
        other => other.replace('-', "_"),
    }
}

fn command_module_path(module: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/core/src/commands")
        .join(format!("{module}.rs"))
}

fn command_module_files() -> Vec<String> {
    let mut files: Vec<String> = command_module_paths()
        .into_iter()
        .filter_map(|path| path.file_stem().map(|stem| stem.to_owned()))
        .filter_map(|stem| stem.to_str().map(ToOwned::to_owned))
        .collect();
    files.sort();
    files
}

fn command_module_paths() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/core/src/commands");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("command module directory exists")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    paths.sort();
    paths
}

fn coverage_names() -> BTreeSet<&'static str> {
    COMMAND_SPECIFIC_TESTS
        .iter()
        .chain(SHARED_REF_ACTION_TESTS)
        .chain(BINARY_CONTRACT_TESTS)
        .chain(ADAPTER_PASSTHROUGH_COMMANDS)
        .copied()
        .collect()
}
