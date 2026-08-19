#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod batch;
mod cli;
mod cli_args;
mod command_policy;
mod diagnostic;
mod dispatch;

/// Shared blanket-default `PlatformAdapter` test double, sourced once from
/// `tests/support/noop_ops.rs` (also consumed by the standalone
/// `conformance` integration-test crate) and reused by any in-crate unit
/// test that needs "some adapter" without exercising a live capability.
#[cfg(test)]
#[path = "../tests/support/noop_ops.rs"]
mod test_noop_ops;

use agent_desktop_core::{
    AdapterError, AppError, DeliverySemantics, ErrorCode,
    context::{CommandContext, WaitSelector},
    output::{ErrorPayload, Response},
    session::resolve_active_session,
};
use clap::{CommandFactory, Parser};
use cli::{Cli, Commands};
use cli_args::skills::SkillsAction;
use std::io::{BufWriter, Write};
use std::process::ExitCode;

const EXIT_INVALID_ARGS: u8 = 2;

fn main() -> ExitCode {
    #[cfg(target_os = "macos")]
    if let Some(exit_code) = run_permission_prompt_helper() {
        return exit_code;
    }
    run()
}

#[cfg(target_os = "macos")]
fn run_permission_prompt_helper() -> Option<ExitCode> {
    let (exit_code, response) = agent_desktop_macos::permission_prompt_helper_from_env()?;
    let stdout = std::io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    let result = writer
        .write_all(response.as_bytes())
        .and_then(|()| writer.write_all(b"\n"))
        .and_then(|()| writer.flush());
    Some(match result {
        Ok(()) => ExitCode::from(exit_code),
        Err(error) => report_output_failure(error),
    })
}

fn run() -> ExitCode {
    let mut cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                return match e.print() {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(write_err) => report_output_failure(write_err),
                };
            }
            let msg = e.to_string();
            let first_line = msg.lines().next().unwrap_or("parse error");
            let error = AppError::invalid_input(diagnostic::bounded_text(first_line, 512));
            return if let Err(write_err) = emit_response(&Response::err(
                "unknown",
                ErrorPayload::from_app_error(&error),
            )) {
                report_output_failure(write_err)
            } else {
                ExitCode::from(EXIT_INVALID_ARGS)
            };
        }
    };

    init_tracing(cli.verbose);

    let cmd = match cli.command.take() {
        Some(c) => c,
        None => {
            return match Cli::command().print_help() {
                Ok(()) => ExitCode::SUCCESS,
                Err(write_err) => report_output_failure(write_err),
            };
        }
    };

    let cmd_name = cmd.name();

    if let Err(err) = agent_desktop_core::validate_state_root_env() {
        return finish(cmd_name, Err(pre_dispatch_error(err)));
    }

    match cmd {
        Commands::Version => finish(
            cmd_name,
            agent_desktop_core::commands::version::execute().map_err(pre_dispatch_error),
        ),
        Commands::Skills(a) => {
            let result = match a.action.unwrap_or(SkillsAction::List) {
                SkillsAction::List => agent_desktop_core::commands::skills::list(),
                SkillsAction::Path => agent_desktop_core::commands::skills::path(),
                SkillsAction::Get(g) => agent_desktop_core::commands::skills::get(
                    agent_desktop_core::commands::skills::GetArgs {
                        name: g.name,
                        full: g.full,
                        reference: g.reference,
                    },
                ),
            };
            finish(cmd_name, result.map_err(pre_dispatch_error))
        }
        cmd => {
            let wait_selector = match build_wait_selector(&cli) {
                Ok(wait_selector) => wait_selector,
                Err(error) => return finish(cmd_name, Err(error)),
            };
            let session_id = match resolve_active_session(
                cli.session.as_deref(),
                std::env::var("AGENT_DESKTOP_SESSION").ok().as_deref(),
            ) {
                Ok(session_id) => session_id,
                Err(err) => {
                    return finish(cmd_name, Err(pre_dispatch_error(err)));
                }
            };
            let context = match CommandContext::new(session_id, cli.trace, cli.trace_strict) {
                Ok(context) => context
                    .with_headed(cli.headed)
                    .with_wait_selector(wait_selector.clone()),
                Err(err) => {
                    return finish(cmd_name, Err(pre_dispatch_error(err)));
                }
            };
            if let Some(wait) = wait_selector.as_ref() {
                if let Err(err) = validate_wait_for_command(&cmd, wait) {
                    return finish(cmd_name, Err(err));
                }
            }
            run_with_adapter(cmd, cmd_name, &context)
        }
    }
}

fn build_wait_selector(cli: &Cli) -> Result<Option<WaitSelector>, AppError> {
    let query = cli
        .post_action_wait
        .wait_for
        .as_ref()
        .map(|raw| (raw, false))
        .or_else(|| {
            cli.post_action_wait
                .wait_for_gone
                .as_ref()
                .map(|raw| (raw, true))
        });
    let Some((query_raw, gone)) = query else {
        if cli.post_action_wait.wait_timeout.is_some() {
            return Err(AppError::invalid_input_with_suggestion(
                "--wait-timeout requires --wait-for or --wait-for-gone",
                "Add a selector wait flag or remove --wait-timeout",
            ));
        }
        return Ok(None);
    };
    Ok(Some(WaitSelector {
        query_raw: query_raw.clone(),
        gone,
        timeout_ms: cli.post_action_wait.wait_timeout.unwrap_or(30_000),
    }))
}

fn validate_wait_for_command(cmd: &Commands, wait: &WaitSelector) -> Result<(), AppError> {
    if !cmd.supports_post_action_wait() {
        return Err(AppError::invalid_input_with_suggestion(
            format!(
                "Command '{}' does not support --wait-for or --wait-for-gone",
                cmd.name()
            ),
            "Use snapshot --wait-for \"<selector>\" or a supported ref action (click, type, …).",
        ));
    }
    agent_desktop_core::commands::query::validate_selector(&wait.query_raw)?;
    Ok(())
}

fn run_with_adapter(cmd: Commands, cmd_name: &str, context: &CommandContext) -> ExitCode {
    let adapter = build_adapter();
    let adapter: &dyn agent_desktop_core::PlatformAdapter = &adapter;
    let report = if command_policy::requires_permission_report(&cmd) {
        match agent_desktop_core::Deadline::standard()
            .map_err(AppError::from)
            .and_then(|deadline| adapter.permission_report(deadline).map_err(AppError::from))
        {
            Ok(report) => report,
            Err(err) => return finish(cmd_name, Err(pre_dispatch_error(err))),
        }
    } else {
        agent_desktop_core::PermissionReport::default()
    };
    if let Err(err) = command_policy::preflight(&cmd, &report) {
        return finish(cmd_name, Err(err));
    }

    let result = dispatch::dispatch(cmd, adapter, &report, context);
    finish(cmd_name, result)
}

fn pre_dispatch_error(error: AppError) -> AppError {
    match error {
        AppError::Adapter(mut source) => {
            source.disposition = DeliverySemantics::not_delivered();
            source.into()
        }
        other => AdapterError::new(ErrorCode::Internal, other.to_string())
            .with_disposition(DeliverySemantics::not_delivered())
            .into(),
    }
}

fn finish(cmd_name: &str, result: Result<serde_json::Value, AppError>) -> ExitCode {
    match result {
        Ok(data) => {
            if let Err(write_err) = emit_response(&Response::ok(cmd_name, data)) {
                return report_output_failure(write_err);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            if let Err(write_err) = emit_response(&Response::err(
                cmd_name,
                agent_desktop_core::ErrorPayload::from_app_error(&e),
            )) {
                return report_output_failure(write_err);
            }
            ExitCode::FAILURE
        }
    }
}

fn report_output_failure(write_err: std::io::Error) -> ExitCode {
    eprintln!("agent-desktop: failed to write response to stdout: {write_err}");
    ExitCode::FAILURE
}

fn emit_response(response: &Response) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    serde_json::to_writer(&mut writer, response).map_err(std::io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn build_adapter() -> impl agent_desktop_core::PlatformAdapter {
    #[cfg(target_os = "macos")]
    {
        agent_desktop_macos::MacOSAdapter::new()
    }

    #[cfg(target_os = "windows")]
    {
        agent_desktop_windows::WindowsAdapter::new()
    }

    #[cfg(target_os = "linux")]
    {
        agent_desktop_linux::LinuxAdapter::new()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    compile_error!("Unsupported platform")
}

fn init_tracing(verbose: bool) {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter = if verbose { "debug" } else { "warn" };
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
}

#[cfg(test)]
#[path = "tests/main_tests.rs"]
mod main_tests;
