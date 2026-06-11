use super::*;

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
fn adapter_guard_refuses_protected_processes_with_the_cli_contract() {
    let err = ensure_not_protected("loginwindow").unwrap_err();

    assert_eq!(err.code, agent_desktop_core::error::ErrorCode::InvalidArgs);
    assert!(err.message.contains("protected"));
    assert!(err.suggestion.is_some());
    assert!(ensure_not_protected("TextEdit").is_ok());
}
