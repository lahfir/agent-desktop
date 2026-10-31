use agent_desktop_core::{AppError, response::output::Response};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HostRequest {
    pub identity: String,
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin_text: Option<String>,
    pub directory: PathBuf,
    pub agent_id: Option<String>,
    pub expires_ns: u64,
    #[serde(default)]
    pub baseline: Option<agent_desktop_core::SignalBaseline>,
}

pub(super) fn execute(bytes: &[u8], identity: &str, session: Option<&str>) -> Response {
    match prepare(bytes, identity, session) {
        Ok((cli, command, deadline, baseline)) => crate::response(
            command.name(),
            crate::execute_with_deadline(cli, command, Some(deadline), baseline),
        ),
        Err(error) => crate::response("unknown", Err(crate::pre_dispatch_error(error))),
    }
}

fn prepare(
    bytes: &[u8],
    identity: &str,
    session: Option<&str>,
) -> Result<
    (
        crate::Cli,
        crate::Commands,
        agent_desktop_core::Deadline,
        Option<agent_desktop_core::SignalBaseline>,
    ),
    AppError,
> {
    let request: HostRequest = serde_json::from_slice(bytes)
        .map_err(|_| AppError::invalid_input("Invalid command host request envelope"))?;
    if request.identity != identity {
        return Err(AppError::invalid_input(
            "Command host identity does not match the caller",
        ));
    }
    if !request.directory.is_absolute() {
        return Err(AppError::invalid_input(
            "Command host working directory must be absolute",
        ));
    }
    let (mut cli, mut command) =
        super::parse_request(&serde_json::to_vec(&request.args)?, session)?;
    apply_stdin_text(&mut command, request.stdin_text)?;
    if cli.identity.agent_id.is_none() {
        cli.identity.agent_id = request.agent_id;
    }
    let deadline =
        agent_desktop_core::Deadline::from_duration(super::budget::remaining(request.expires_ns)?)?;
    std::env::set_current_dir(&request.directory)?;
    Ok((cli, command, deadline, request.baseline))
}

fn apply_stdin_text(
    command: &mut crate::Commands,
    stdin_text: Option<String>,
) -> Result<(), AppError> {
    match (crate::cli_args::stdin::input(command), stdin_text) {
        (Some(value), Some(text)) => {
            crate::cli_args::stdin::validate_bytes(text.as_bytes())?;
            *value = Some(text);
            Ok(())
        }
        (Some(_), None) => Err(AppError::invalid_input(
            "stdin text is missing from the command host request",
        )),
        (None, Some(_)) => Err(AppError::invalid_input(
            "stdin text was supplied without the --stdin flag",
        )),
        (None, None) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_stdin_requests_reconstruct_exact_text_for_each_command() {
        for args in [
            ["type", "@s8f3k2p9:e1", "--stdin"].as_slice(),
            ["set-value", "--stdin", "@s8f3k2p9:e1"].as_slice(),
            ["clipboard-set", "--stdin"].as_slice(),
        ] {
            let text = "first\r\nsecond\n世界";
            let (cli, mut command) =
                super::super::parse_request(&serde_json::to_vec(&args).unwrap(), None).unwrap();
            apply_stdin_text(&mut command, Some(text.to_owned())).unwrap();
            let request = HostRequest {
                identity: "owner".into(),
                args: args.iter().map(|arg| (*arg).to_owned()).collect(),
                stdin_text: Some(text.to_owned()),
                directory: std::env::current_dir().unwrap(),
                agent_id: None,
                expires_ns: super::super::budget::expires_at(&cli, &command).unwrap(),
                baseline: None,
            };
            let (_, mut command, _, _) =
                prepare(&serde_json::to_vec(&request).unwrap(), "owner", None).unwrap();
            assert_eq!(
                crate::cli_args::stdin::input(&mut command)
                    .unwrap()
                    .as_deref(),
                Some(text)
            );
            if let crate::Commands::Type(args) = command {
                assert_eq!(args.effective_timeout_ms(), 5480);
            }
        }
    }

    #[test]
    fn stdin_requests_reject_missing_unexpected_invalid_and_oversized_text() {
        use clap::Parser;
        let oversized = "x".repeat(128 * 1024 + 1);
        let mut cli =
            crate::Cli::try_parse_from(["agent-desktop", "type", "@s8f3k2p9:e1", "--stdin"])
                .unwrap();
        let mut command = cli.command.take().unwrap();

        for (text, expected) in [
            (None, "missing"),
            (Some("private\0marker".to_owned()), "NUL"),
            (Some(oversized), "128 KiB"),
        ] {
            let error = apply_stdin_text(&mut command, text).unwrap_err();
            assert_eq!(error.code(), "INVALID_ARGS");
            assert!(error.to_string().contains(expected));
            assert!(!error.to_string().contains("private"));
        }

        let error = apply_stdin_text(&mut crate::Commands::Version, Some("private-marker".into()))
            .unwrap_err();
        assert!(error.to_string().contains("without the --stdin flag"));
        assert!(!error.to_string().contains("private-marker"));
    }

    #[test]
    fn wrong_identity_or_namespace_never_changes_the_working_directory() {
        let before = std::env::current_dir().unwrap();
        for (identity, args) in [
            ("another executable", vec!["version"]),
            ("owner", vec!["--session", "other", "version"]),
        ] {
            let bytes = serde_json::to_vec(&HostRequest {
                identity: identity.into(),
                args: args.into_iter().map(str::to_owned).collect(),
                stdin_text: None,
                directory: PathBuf::from("/"),
                agent_id: None,
                expires_ns: u64::MAX,
                baseline: None,
            })
            .unwrap();
            let response = execute(&bytes, "owner", Some("mine"));
            let response = serde_json::to_value(response).unwrap();
            assert_eq!(response["ok"], false);
            assert_eq!(
                response["error"]["disposition"]["delivery"],
                "not_delivered"
            );
            assert_eq!(std::env::current_dir().unwrap(), before);
        }
    }
}
