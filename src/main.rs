mod batch;
mod cli;
mod cli_args;
mod command_policy;
mod dispatch;

use agent_desktop_core::{
    adapter::PlatformAdapter,
    context::{CommandContext, WaitSelector},
    error::AppError,
    output::{ENVELOPE_VERSION, ErrorPayload, Response},
    session::resolve_active_session,
};
use clap::{CommandFactory, Parser};
use cli::{Cli, Commands};
use cli_args::skills::SkillsAction;
use std::io::{BufWriter, Write};

const WAIT_SUPPORTED: &[&str] = &[
    "snapshot",
    "click",
    "double-click",
    "triple-click",
    "right-click",
    "clear",
    "focus",
    "toggle",
    "check",
    "uncheck",
    "expand",
    "collapse",
    "scroll-to",
    "type",
    "set-value",
    "select",
    "scroll",
];

fn main() {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                e.exit();
            }
            let msg = e.to_string();
            let first_line = msg.lines().next().unwrap_or("parse error");
            emit_response(&Response::err(
                "unknown",
                ErrorPayload::new("INVALID_ARGS", first_line),
            ));
            std::process::exit(2);
        }
    };

    init_tracing(cli.verbose);
    let wait_selector = build_wait_selector(&cli);
    let session_id = match resolve_active_session(
        cli.session.as_deref(),
        std::env::var("AGENT_DESKTOP_SESSION").ok().as_deref(),
    ) {
        Ok(session_id) => session_id,
        Err(err) => {
            finish("unknown", Err(err));
            return;
        }
    };
    let context = match CommandContext::new(session_id, cli.trace, cli.trace_strict) {
        Ok(context) => context
            .with_headed(cli.headed)
            .with_wait_selector(wait_selector.clone()),
        Err(err) => {
            finish("unknown", Err(err));
            return;
        }
    };

    let cmd = match cli.command {
        Some(c) => c,
        None => {
            Cli::command().print_help().unwrap_or(());
            std::process::exit(0);
        }
    };

    let cmd_name = cmd.name();

    if let Some(wait) = wait_selector.as_ref() {
        if let Err(err) = validate_wait_for_command(cmd_name, wait) {
            finish(cmd_name, Err(err));
            return;
        }
    }

    match cmd {
        Commands::Version => {
            let result = agent_desktop_core::commands::version::execute();
            finish(cmd_name, result);
        }
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
            finish(cmd_name, result);
        }
        cmd => run_with_adapter(cmd, cmd_name, &context),
    }
}

fn build_wait_selector(cli: &Cli) -> Option<WaitSelector> {
    let (query_raw, gone) = cli
        .wait_for
        .as_ref()
        .map(|raw| (raw, false))
        .or_else(|| cli.wait_for_gone.as_ref().map(|raw| (raw, true)))?;
    Some(WaitSelector {
        query_raw: query_raw.clone(),
        gone,
        timeout_ms: cli.wait_timeout,
    })
}

fn validate_wait_for_command(cmd_name: &str, wait: &WaitSelector) -> Result<(), AppError> {
    if !WAIT_SUPPORTED.contains(&cmd_name) {
        return Err(AppError::invalid_input_with_suggestion(
            format!("Command '{cmd_name}' does not support --wait-for or --wait-for-gone"),
            "Use snapshot --wait-for \"<selector>\" or a supported ref action (click, type, …).",
        ));
    }
    agent_desktop_core::commands::query::validate_selector(&wait.query_raw)?;
    Ok(())
}

fn run_with_adapter(cmd: Commands, cmd_name: &str, context: &CommandContext) {
    let adapter = build_adapter();
    let report = adapter.permission_report();
    if let Err(err) = command_policy::preflight(&cmd, &report) {
        finish(cmd_name, Err(err));
        return;
    }

    let result = dispatch::dispatch(cmd, &adapter, &report, context);
    finish(cmd_name, result);
}

fn finish(cmd_name: &str, result: Result<serde_json::Value, agent_desktop_core::error::AppError>) {
    match result {
        Ok(data) => {
            emit_response(&Response::ok(cmd_name, data));
            std::process::exit(0);
        }
        Err(e) => {
            emit_response(&Response::err(
                cmd_name,
                agent_desktop_core::ErrorPayload::from_app_error(&e),
            ));
            std::process::exit(1);
        }
    }
}

fn emit_response(response: &Response) {
    match serde_json::to_value(response) {
        Ok(value) => emit_json(&value),
        Err(err) => emit_json(&serde_json::json!({
            "version": ENVELOPE_VERSION,
            "ok": false,
            "command": "internal",
            "error": {
                "code": "INTERNAL",
                "message": format!("Failed to serialize response: {err}")
            }
        })),
    }
}

fn emit_json(value: &serde_json::Value) {
    let stdout = std::io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    if serde_json::to_writer(&mut writer, value).is_err() {
        return;
    }
    let _ = writer.write_all(b"\n");
    let _ = writer.flush();
}

fn build_adapter() -> impl agent_desktop_core::adapter::PlatformAdapter {
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
