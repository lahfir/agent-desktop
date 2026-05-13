use crate::cli::Cli;
use clap::CommandFactory;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const NON_COMMAND_MODULES: &[&str] = &[
    "helpers",
    "mod",
    "ref_policy_tests",
    "resolved_element",
    "search_text",
    "wait_tests",
];

const INLINE_COMMAND_TESTS: &[&str] = &[
    "find",
    "focus-window",
    "is",
    "list-apps",
    "right-click",
    "skills",
    "snapshot",
    "status",
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
    "maximize",
    "minimize",
    "mouse-click",
    "mouse-down",
    "mouse-move",
    "mouse-up",
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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/core/src/commands");
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .expect("command module directory exists")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.path().file_stem().map(|stem| stem.to_owned()))
        .filter_map(|stem| stem.to_str().map(ToOwned::to_owned))
        .collect();
    files.sort();
    files
}

fn coverage_names() -> BTreeSet<&'static str> {
    INLINE_COMMAND_TESTS
        .iter()
        .chain(SHARED_REF_ACTION_TESTS)
        .chain(BINARY_CONTRACT_TESTS)
        .chain(ADAPTER_PASSTHROUGH_COMMANDS)
        .copied()
        .collect()
}
