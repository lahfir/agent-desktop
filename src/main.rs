mod batch;
mod cli;
mod cli_args;
mod command_policy;
mod dispatch;

use agent_desktop_core::{
    adapter::PlatformAdapter,
    context::CommandContext,
    output::{ENVELOPE_VERSION, ErrorPayload, Response},
};
use clap::{CommandFactory, Parser};
use cli::{Cli, Commands};
use cli_args::skills::SkillsAction;
use std::io::{BufWriter, Write};

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
    let context = match CommandContext::new(cli.session, cli.trace, cli.trace_strict) {
        Ok(context) => context.with_headed(cli.headed),
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
