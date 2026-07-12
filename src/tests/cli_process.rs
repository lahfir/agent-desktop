use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
}

#[test]
fn clap_help_returns_success_without_structured_error_noise() {
    let output = binary().arg("--help").output().expect("binary starts");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    assert!(output.stderr.is_empty());
}

#[test]
fn clap_parse_failure_is_structured_on_stdout_with_exit_two() {
    let output = binary()
        .arg("--definitely-not-a-real-flag")
        .output()
        .expect("binary starts");
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is one JSON envelope");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["command"], "unknown");
    assert_eq!(envelope["error"]["code"], "INVALID_ARGS");
    assert_eq!(
        envelope["error"]["disposition"]["delivery"],
        "not_delivered"
    );
    assert_eq!(envelope["error"]["disposition"]["retry"], "safe");
}

#[test]
fn version_has_exact_package_identity() {
    let output = binary().arg("version").output().expect("binary starts");
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is one JSON envelope");

    assert!(output.status.success());
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["version"], env!("CARGO_PKG_VERSION"));
}

#[cfg(target_os = "macos")]
#[test]
fn malformed_permission_helper_invocation_bypasses_clap_and_tracing() {
    const HELPER_ENV: [&str; 6] = [
        "AGENT_DESKTOP_PERMISSION_HELPER",
        "AGENT_DESKTOP_PERMISSION_OPERATION",
        "AGENT_DESKTOP_PERMISSION_TOKEN",
        "AGENT_DESKTOP_PERMISSION_PARENT_PID",
        "AGENT_DESKTOP_PERMISSION_PARENT_INSTANCE",
        "AGENT_DESKTOP_PERMISSION_EXECUTABLE",
    ];

    let mut command = binary();
    command.arg("--definitely-not-a-real-flag");
    for name in HELPER_ENV {
        command.env_remove(name);
    }
    let output = command
        .env("AGENT_DESKTOP_PERMISSION_HELPER", "invalid")
        .env("RUST_LOG", "trace")
        .output()
        .expect("binary starts");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    let response: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is one JSON line");
    assert_eq!(response["version"], 1);
    assert_eq!(response["ok"], false);
    assert_eq!(response["error"], "invalid_helper_invocation");
}
