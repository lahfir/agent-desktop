use super::*;
use crate::cli::{Cli, Commands};
use crate::cli_args::{RefArgs, ScreenshotArgs, SnapshotArgs};
use agent_desktop_core::{PermissionReport, PermissionState};
use clap::CommandFactory;

const VALID_REF_ID: &str = "@e1";

#[test]
fn every_cli_subcommand_has_policy() {
    for subcommand in Cli::command().get_subcommands() {
        let name = subcommand.get_name();
        assert!(
            command_name_is_covered(name),
            "missing permission policy coverage for {name}"
        );
    }
}

fn command_name_is_covered(name: &str) -> bool {
    matches!(
        name,
        "snapshot"
            | "find"
            | "screenshot"
            | "get"
            | "is"
            | "click"
            | "double-click"
            | "triple-click"
            | "right-click"
            | "type"
            | "set-value"
            | "clear"
            | "focus"
            | "select"
            | "toggle"
            | "check"
            | "uncheck"
            | "expand"
            | "collapse"
            | "scroll"
            | "scroll-to"
            | "press"
            | "key-down"
            | "key-up"
            | "hover"
            | "drag"
            | "mouse-move"
            | "mouse-click"
            | "mouse-down"
            | "mouse-up"
            | "launch"
            | "close-app"
            | "list-windows"
            | "list-apps"
            | "focus-window"
            | "resize-window"
            | "move-window"
            | "minimize"
            | "maximize"
            | "restore"
            | "list-surfaces"
            | "list-notifications"
            | "dismiss-notification"
            | "dismiss-all-notifications"
            | "notification-action"
            | "clipboard-get"
            | "clipboard-set"
            | "clipboard-clear"
            | "wait"
            | "status"
            | "permissions"
            | "version"
            | "batch"
            | "skills"
    )
}

#[test]
fn unknown_permission_does_not_mask_platform_errors() {
    let report = PermissionReport::default();
    let command = Commands::Screenshot(ScreenshotArgs {
        app: None,
        window_id: None,
        output_path: None,
    });

    assert!(preflight(&command, &report).is_ok());
}

#[test]
fn screen_recording_denial_is_preflighted() {
    let report = PermissionReport {
        accessibility: PermissionState::Granted,
        screen_recording: PermissionState::Denied {
            suggestion: "grant screen recording".into(),
        },
        automation: PermissionState::NotRequired,
    };
    let command = Commands::Screenshot(ScreenshotArgs {
        app: None,
        window_id: None,
        output_path: None,
    });

    let err = preflight(&command, &report).expect_err("denied screen capture fails");

    assert_eq!(err.code(), "PERM_DENIED");
}

#[test]
fn accessibility_denial_is_preflighted_for_ax_commands() {
    let report = PermissionReport {
        accessibility: PermissionState::Denied {
            suggestion: "grant accessibility".into(),
        },
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };
    let command = Commands::Click(crate::cli_args::RefArgs {
        ref_id: VALID_REF_ID.into(),
        snapshot_id: None,
    });

    let err = preflight(&command, &report).expect_err("denied accessibility fails");

    assert_eq!(err.code(), "PERM_DENIED");
    assert_eq!(err.suggestion(), Some("grant accessibility"));
}

#[test]
fn invalid_ref_args_are_rejected_before_permission_preflight() {
    let report = PermissionReport {
        accessibility: PermissionState::Denied {
            suggestion: "grant accessibility".into(),
        },
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };
    let command = Commands::Click(RefArgs {
        ref_id: "bad-ref".into(),
        snapshot_id: None,
    });

    let err = preflight(&command, &report).expect_err("invalid ref fails first");

    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn invalid_snapshot_root_is_rejected_before_permission_preflight() {
    let report = PermissionReport {
        accessibility: PermissionState::Denied {
            suggestion: "grant accessibility".into(),
        },
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };
    let command = Commands::Snapshot(SnapshotArgs {
        app: None,
        window_id: None,
        max_depth: 10,
        include_bounds: false,
        interactive_only: false,
        compact: false,
        surface: crate::cli_args::Surface::Window,
        skeleton: false,
        root: Some("bad-root".into()),
        snapshot: None,
    });

    let err = preflight(&command, &report).expect_err("invalid root fails first");

    assert_eq!(err.code(), "INVALID_ARGS");
}
