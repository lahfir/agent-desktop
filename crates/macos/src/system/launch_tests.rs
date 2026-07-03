use super::*;
use agent_desktop_core::error::ErrorCode;

#[test]
fn open_app_args_preserve_current_focus() {
    assert_eq!(open_app_args("Mail"), ["-g", "-a", "Mail"]);
}

#[test]
fn open_argv_with_no_args_omits_the_args_flag() {
    assert_eq!(open_argv("Mail", &[]), vec!["-g", "-a", "Mail"]);
}

#[test]
fn open_argv_with_one_arg_emits_the_args_flag_once() {
    let args = vec!["foo".to_string()];

    assert_eq!(
        open_argv("Mail", &args),
        vec!["-g", "-a", "Mail", "--args", "foo"]
    );
}

#[test]
fn open_argv_with_two_args_emits_exactly_one_args_flag() {
    let args = vec!["foo".to_string(), "bar".to_string()];
    let argv = open_argv("Mail", &args);

    assert_eq!(argv, vec!["-g", "-a", "Mail", "--args", "foo", "bar"]);
    assert_eq!(
        argv.iter().filter(|a| a.as_str() == "--args").count(),
        1,
        "exactly one --args flag regardless of how many app args are passed, got argv: {argv:?}"
    );
}

#[test]
fn empty_options_argv_is_byte_identical_to_the_pre_options_launch_path() {
    let empty_options_argv = open_argv("Preview", &[]);

    assert_eq!(empty_options_argv, vec!["-g", "-a", "Preview"]);
    assert_eq!(
        empty_options_argv,
        open_app_args("Preview")
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>(),
        "LaunchOptions with no args must produce the exact argv the options-free launch path used"
    );
}

#[test]
fn no_attach_against_a_running_app_fails_with_a_structured_error_naming_the_pid() {
    let err = launch_conflict_error("Marker9182App", 4242);

    assert_eq!(err.code, ErrorCode::ActionFailed);
    assert!(
        err.message.contains("4242"),
        "structured --no-attach conflict must name the running pid so the caller can act on \
         it, got message: {}",
        err.message
    );
}

#[test]
fn launch_conflict_error_never_puts_the_raw_app_id_in_the_message() {
    let marker = "MARKER_APP_ID_9f31c4";
    let err = launch_conflict_error(marker, 777);

    assert!(
        !err.message.contains(marker),
        "raw app id leaked into a trace-reachable message: {}",
        err.message
    );
    let details = err
        .details
        .expect("--no-attach conflict error should carry the app id in details");
    assert_eq!(details["app_name"], marker);
}

#[test]
fn launch_no_window_error_never_puts_the_raw_app_id_in_the_message() {
    let marker = "MARKER_APP_ID_9f31c4";
    let err = launch_no_window_error(marker, 5000);

    assert!(
        !err.message.contains(marker),
        "raw app id leaked into a trace-reachable message: {}",
        err.message
    );
    assert!(err.message.contains("5000"));
    let details = err
        .details
        .expect("no-window error should carry the app id in details");
    assert_eq!(details["app_name"], marker);
}
