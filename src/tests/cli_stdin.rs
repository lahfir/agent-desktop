use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run_cli(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI starts");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(input)
        .expect("stdin accepts input");
    child.wait_with_output().expect("CLI exits")
}

#[test]
fn each_stdin_command_rejects_invalid_utf8_before_host_forwarding() {
    let input = b"private-marker\xff";
    for args in [
        vec!["type", "@s8f3k2p9:e1", "--stdin"],
        vec!["set-value", "@s8f3k2p9:e1", "--stdin"],
        vec!["clipboard-set", "--stdin"],
    ] {
        let output = run_cli(&args, input);
        let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(!output.status.success());
        assert_eq!(response["command"], args[0]);
        assert_eq!(response["ok"], false);
        assert_eq!(response["error"]["code"], "INVALID_ARGS");
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains("valid UTF-8")
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("private-marker"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private-marker"));
    }
}
