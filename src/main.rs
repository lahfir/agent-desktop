mod batch_dispatch;
mod cli;
mod cli_args;
mod dispatch;

use agent_desktop_core::adapter::PlatformAdapter;
use clap::{CommandFactory, Parser};
use cli::{Cli, Commands};
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
            let json = serde_json::json!({
                "version": "1.0",
                "ok": false,
                "command": "unknown",
                "error": { "code": "INVALID_ARGS", "message": first_line }
            });
            emit_json(&json);
            std::process::exit(2);
        }
    };

    init_tracing(cli.verbose);

    let cmd = match cli.command {
        Some(c) => c,
        None => {
            Cli::command().print_help().unwrap_or(());
            std::process::exit(0);
        }
    };

    let cmd_name = cmd.name();

    match &cmd {
        Commands::Version(a) => {
            let result = agent_desktop_core::commands::version::execute(
                agent_desktop_core::commands::version::VersionArgs { json: a.json },
            );
            finish(cmd_name, result);
            return;
        }
        Commands::Status => {
            let adapter = build_adapter();
            let result = agent_desktop_core::commands::status::execute(&adapter);
            finish(cmd_name, result);
            return;
        }
        _ => {}
    }

    let adapter = build_adapter();

    if let agent_desktop_core::adapter::PermissionStatus::Denied { suggestion } =
        adapter.check_permissions()
    {
        match &cmd {
            Commands::Permissions(_) | Commands::Version(_) | Commands::Status => {}
            _ => {
                let json = serde_json::json!({
                    "version": "1.0",
                    "ok": false,
                    "command": cmd_name,
                    "error": {
                        "code": "PERM_DENIED",
                        "message": "Accessibility permission not granted",
                        "suggestion": suggestion
                    }
                });
                emit_json(&json);
                std::process::exit(1);
            }
        }
    }

    let result = dispatch::dispatch(cmd, &adapter);
    finish(cmd_name, result);
}

fn finish(cmd_name: &str, result: Result<serde_json::Value, agent_desktop_core::error::AppError>) {
    match result {
        Ok(data) => {
            let response = serde_json::json!({
                "version": "1.0",
                "ok": true,
                "command": cmd_name,
                "data": data
            });
            emit_json(&response);
            std::process::exit(0);
        }
        Err(e) => {
            let mut error = serde_json::json!({
                "code": e.code(),
                "message": e.to_string(),
            });
            if let Some(s) = e.suggestion() {
                error["suggestion"] = serde_json::Value::String(s.to_string());
            }
            let response = serde_json::json!({
                "version": "1.0",
                "ok": false,
                "command": cmd_name,
                "error": error
            });
            emit_json(&response);
            std::process::exit(1);
        }
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
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = if verbose { "debug" } else { "warn" };
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
}
