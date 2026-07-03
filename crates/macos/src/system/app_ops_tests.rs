use super::*;
use agent_desktop_core::error::ErrorCode;

#[test]
fn osascript_quit_maps_automation_denial_to_perm_denied() {
    let stderr =
        "36:44: execution error: Not authorized to send Apple events to System Events. (-1743)";
    let err = map_osascript_quit_error("TextEdit", stderr);

    assert_eq!(err.code, ErrorCode::PermDenied);
    assert!(err.suggestion.is_some());
    assert!(
        err.platform_detail
            .as_ref()
            .is_some_and(|d| d.contains("-1743"))
    );
}

#[test]
fn osascript_quit_keeps_action_failed_for_other_errors() {
    let err = map_osascript_quit_error("TextEdit", "application isn't running");

    assert_eq!(err.code, ErrorCode::ActionFailed);
}

#[test]
fn open_app_args_preserve_current_focus() {
    assert_eq!(open_app_args("Mail"), ["-g", "-a", "Mail"]);
}

#[test]
fn protected_processes_match_display_and_bundle_forms() {
    assert!(is_protected_process("Finder"));
    assert!(is_protected_process("Dock"));
    assert!(is_protected_process("com.apple.dock"));
    assert!(is_protected_process("WindowServer"));
    assert!(is_protected_process("loginwindow"));
}

#[test]
fn ordinary_apps_are_not_protected() {
    assert!(!is_protected_process("TextEdit"));
    assert!(!is_protected_process("Safari"));
    assert!(!is_protected_process("com.company.MyApp"));
}

#[test]
fn lookalike_names_containing_protected_substrings_stay_closable() {
    assert!(!is_protected_process("Docker"));
    assert!(!is_protected_process("Docker Desktop"));
    assert!(!is_protected_process("com.docker.docker-desktop"));
    assert!(!is_protected_process("FinderSync"));
    assert!(!is_protected_process("PathFinder"));
    assert!(!is_protected_process("launchdarkly-agent"));
}

#[test]
fn adapter_guard_refuses_protected_processes_with_the_cli_contract() {
    let err = ensure_not_protected("loginwindow").unwrap_err();

    assert_eq!(err.code, agent_desktop_core::error::ErrorCode::InvalidArgs);
    assert!(err.message.contains("protected"));
    assert!(err.suggestion.is_some());
    assert!(ensure_not_protected("TextEdit").is_ok());
}
