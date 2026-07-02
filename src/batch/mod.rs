use agent_desktop_core::{
    PermissionReport,
    adapter::PlatformAdapter,
    commands::batch::BatchCommand,
    context::CommandContext,
    error::AppError,
    output::{ENVELOPE_VERSION, ErrorPayload},
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::{
    cli::Commands,
    cli_args::{
        session::{SessionAction, SessionArgs, SessionEndArgs, SessionGcArgs, SessionStartArgs},
        skills::{SkillsAction, SkillsArgs, SkillsGetArgs},
        system::BatchArgs,
        trace::{TraceAction, TraceArgs, TraceExportArgs, TraceShowArgs},
    },
};

pub(crate) fn execute(
    args: BatchArgs,
    adapter: &dyn PlatformAdapter,
    permission_report: &PermissionReport,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let commands = agent_desktop_core::commands::batch::parse_commands(&args.commands_json)?;
    let mut results = Vec::new();

    for item in commands {
        let command = item.command.clone();
        let result = match context.for_batch_item(item.session.clone()) {
            Ok(item_context) => parse_command(item).and_then(|typed| {
                crate::command_policy::preflight(&typed, permission_report)?;
                crate::dispatch::dispatch(typed, adapter, permission_report, &item_context)
            }),
            Err(err) => Err(err),
        };
        let ok = result.is_ok();
        results.push(batch_entry(&command, result));
        if !ok && args.stop_on_error {
            break;
        }
    }

    Ok(json!({ "results": results }))
}

pub(crate) fn parse_command(item: BatchCommand) -> Result<Commands, AppError> {
    let command = item.command.as_str();
    match command {
        "snapshot" => decode(command, item.args).map(Commands::Snapshot),
        "find" => decode(command, item.args).map(Commands::Find),
        "screenshot" => decode(command, item.args).map(Commands::Screenshot),
        "get" => decode(command, item.args).map(Commands::Get),
        "is" => decode(command, item.args).map(Commands::Is),
        "click" => decode(command, item.args).map(Commands::Click),
        "double-click" => decode(command, item.args).map(Commands::DoubleClick),
        "triple-click" => decode(command, item.args).map(Commands::TripleClick),
        "right-click" => decode(command, item.args).map(Commands::RightClick),
        "type" => decode(command, item.args).map(Commands::Type),
        "set-value" => decode(command, item.args).map(Commands::SetValue),
        "clear" => decode(command, item.args).map(Commands::Clear),
        "focus" => decode(command, item.args).map(Commands::Focus),
        "select" => decode(command, item.args).map(Commands::Select),
        "toggle" => decode(command, item.args).map(Commands::Toggle),
        "check" => decode(command, item.args).map(Commands::Check),
        "uncheck" => decode(command, item.args).map(Commands::Uncheck),
        "expand" => decode(command, item.args).map(Commands::Expand),
        "collapse" => decode(command, item.args).map(Commands::Collapse),
        "scroll" => decode(command, item.args).map(Commands::Scroll),
        "scroll-to" => decode(command, item.args).map(Commands::ScrollTo),
        "press" => decode(command, item.args).map(Commands::Press),
        "key-down" => decode(command, item.args).map(Commands::KeyDown),
        "key-up" => decode(command, item.args).map(Commands::KeyUp),
        "hover" => decode(command, item.args).map(Commands::Hover),
        "drag" => decode(command, item.args).map(Commands::Drag),
        "mouse-move" => decode(command, item.args).map(Commands::MouseMove),
        "mouse-click" => decode(command, item.args).map(Commands::MouseClick),
        "mouse-down" => decode(command, item.args).map(Commands::MouseDown),
        "mouse-up" => decode(command, item.args).map(Commands::MouseUp),
        "launch" => decode(command, item.args).map(Commands::Launch),
        "close-app" => decode(command, item.args).map(Commands::CloseApp),
        "list-windows" => decode(command, item.args).map(Commands::ListWindows),
        "list-apps" => decode(command, item.args).map(Commands::ListApps),
        "focus-window" => decode(command, item.args).map(Commands::FocusWindow),
        "resize-window" => decode(command, item.args).map(Commands::ResizeWindow),
        "move-window" => decode(command, item.args).map(Commands::MoveWindow),
        "minimize" => decode(command, item.args).map(Commands::Minimize),
        "maximize" => decode(command, item.args).map(Commands::Maximize),
        "restore" => decode(command, item.args).map(Commands::Restore),
        "list-surfaces" => decode(command, item.args).map(Commands::ListSurfaces),
        "list-notifications" => decode(command, item.args).map(Commands::ListNotifications),
        "dismiss-notification" => decode(command, item.args).map(Commands::DismissNotification),
        "dismiss-all-notifications" => {
            decode(command, item.args).map(Commands::DismissAllNotifications)
        }
        "notification-action" => decode(command, item.args).map(Commands::NotificationAction),
        "clipboard-get" => no_args(command, item.args).map(|()| Commands::ClipboardGet),
        "clipboard-set" => decode(command, item.args).map(Commands::ClipboardSet),
        "clipboard-clear" => no_args(command, item.args).map(|()| Commands::ClipboardClear),
        "wait" => decode(command, item.args).map(Commands::Wait),
        "status" => no_args(command, item.args).map(|()| Commands::Status),
        "permissions" => decode(command, item.args).map(Commands::Permissions),
        "version" => no_args(command, item.args).map(|()| Commands::Version),
        "skills" => parse_skills(item.args).map(Commands::Skills),
        "session" => parse_session(item.args).map(Commands::Session),
        "trace" => parse_trace(item.args).map(Commands::Trace),
        "batch" => Err(AppError::invalid_input_with_suggestion(
            "Batch commands cannot be nested",
            "Flatten nested batches into one top-level batch array",
        )),
        other => Err(AppError::invalid_input(format!(
            "Unknown batch command '{other}'"
        ))),
    }
}

fn batch_entry(command: &str, result: Result<Value, AppError>) -> Value {
    match result {
        Ok(data) => {
            json!({ "version": ENVELOPE_VERSION, "ok": true, "command": command, "data": data })
        }
        Err(err) => {
            let error = ErrorPayload::from_app_error(&err);
            json!({ "version": ENVELOPE_VERSION, "ok": false, "command": command, "error": error })
        }
    }
}

fn decode<T>(command: &str, args: Value) -> Result<T, AppError>
where
    T: DeserializeOwned,
{
    serde_json::from_value(args_or_empty(args)).map_err(|e| {
        AppError::invalid_input_with_suggestion(
            format!("Invalid batch args for '{command}': {e}"),
            "Use the same argument names and value types as the matching CLI command",
        )
    })
}

fn no_args(command: &str, args: Value) -> Result<(), AppError> {
    match args_or_empty(args) {
        Value::Object(map) if map.is_empty() => Ok(()),
        _ => Err(AppError::invalid_input_with_suggestion(
            format!("Batch command '{command}' does not accept args"),
            "Use an empty object or omit args for this command",
        )),
    }
}

fn args_or_empty(args: Value) -> Value {
    match args {
        Value::Null => Value::Object(Map::new()),
        other => other,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchSkillsArgs {
    action: Option<String>,
    name: Option<String>,
    reference: Option<String>,
    #[serde(default)]
    full: bool,
}

fn parse_skills(args: Value) -> Result<SkillsArgs, AppError> {
    let args: BatchSkillsArgs = decode("skills", args)?;
    let action = match args.action.as_deref() {
        None if args.name.is_none() => Some(SkillsAction::List),
        None | Some("get") => Some(SkillsAction::Get(SkillsGetArgs {
            name: args
                .name
                .ok_or_else(|| AppError::invalid_input("Batch skills get requires 'name'"))?,
            reference: args.reference,
            full: args.full,
        })),
        Some("list") => Some(SkillsAction::List),
        Some("path") => Some(SkillsAction::Path),
        Some(other) => {
            return Err(AppError::invalid_input(format!(
                "Unknown skills action '{other}'"
            )));
        }
    };
    Ok(SkillsArgs { action })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchSessionArgs {
    #[serde(default)]
    action: Option<String>,
    name: Option<String>,
    #[serde(default)]
    no_trace: bool,
    #[serde(default)]
    screenshots: bool,
    #[serde(default)]
    force: bool,
    id: Option<String>,
    older_than: Option<u64>,
    #[serde(default)]
    ended: bool,
}

fn parse_session(args: Value) -> Result<SessionArgs, AppError> {
    let args: BatchSessionArgs = decode("session", args)?;
    let action = match args.action.as_deref() {
        None | Some("list") => SessionAction::List,
        Some("start") => SessionAction::Start(SessionStartArgs {
            name: args.name,
            no_trace: args.no_trace,
            screenshots: args.screenshots,
            force: args.force,
        }),
        Some("end") => SessionAction::End(SessionEndArgs { id: args.id }),
        Some("gc") => SessionAction::Gc(SessionGcArgs {
            older_than: args.older_than,
            ended: args.ended,
        }),
        Some(other) => {
            return Err(AppError::invalid_input(format!(
                "Unknown session action '{other}'"
            )));
        }
    };
    Ok(SessionArgs { action })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchTraceArgs {
    action: String,
    #[serde(default)]
    limit: Option<usize>,
    event: Option<String>,
    out: Option<std::path::PathBuf>,
}

fn parse_trace(args: Value) -> Result<TraceArgs, AppError> {
    let args: BatchTraceArgs = decode("trace", args)?;
    let action = match args.action.as_str() {
        "show" => TraceAction::Show(TraceShowArgs {
            limit: args
                .limit
                .unwrap_or(agent_desktop_core::commands::trace::TRACE_SHOW_DEFAULT_LIMIT),
            event: args.event,
        }),
        "export" => TraceAction::Export(TraceExportArgs {
            limit: args
                .limit
                .unwrap_or(agent_desktop_core::trace_read::TRACE_EXPORT_DEFAULT_LIMIT),
            out: args.out,
        }),
        other => {
            return Err(AppError::invalid_input(format!(
                "Unknown trace action '{other}'"
            )));
        }
    };
    Ok(TraceArgs { action })
}

#[cfg(test)]
mod tests;
