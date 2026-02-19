use agent_desktop_core::{
    action::Direction,
    adapter::PlatformAdapter,
    commands::{
        batch, click, clipboard_get, clipboard_set, close_app, collapse, double_click, expand,
        find, focus, focus_window, get, helpers, is_check, launch, list_apps, list_windows,
        permissions, press, right_click, screenshot, scroll, select, set_value, snapshot, status,
        toggle, type_text, version, wait,
    },
    error::AppError,
};
use serde_json::Value;

use crate::cli::Commands;

pub fn dispatch(cmd: Commands, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    match cmd {
        Commands::Snapshot(a) => snapshot::execute(
            snapshot::SnapshotArgs {
                app: a.app,
                window_id: a.window_id,
                max_depth: a.max_depth,
                include_bounds: a.include_bounds,
                interactive_only: a.interactive_only,
                compact: a.compact,
            },
            adapter,
        ),

        Commands::Find(a) => find::execute(
            find::FindArgs { app: a.app, role: a.role, name: a.name, value: a.value },
            adapter,
        ),

        Commands::Screenshot(a) => screenshot::execute(
            screenshot::ScreenshotArgs {
                app: a.app,
                window_id: a.window_id,
                output_path: a.output_path,
            },
            adapter,
        ),

        Commands::Get(a) => get::execute(
            get::GetArgs {
                ref_id: a.ref_id,
                property: parse_get_property(&a.property)?,
            },
            adapter,
        ),

        Commands::Is(a) => is_check::execute(
            is_check::IsArgs {
                ref_id: a.ref_id,
                property: parse_is_property(&a.property)?,
            },
            adapter,
        ),

        Commands::Click(a) => click::execute(click::ClickArgs { ref_id: a.ref_id }, adapter),
        Commands::DoubleClick(a) => {
            double_click::execute(double_click::DoubleClickArgs { ref_id: a.ref_id }, adapter)
        }
        Commands::RightClick(a) => {
            right_click::execute(right_click::RightClickArgs { ref_id: a.ref_id }, adapter)
        }

        Commands::Type(a) => type_text::execute(
            type_text::TypeArgs { ref_id: a.ref_id, text: a.text },
            adapter,
        ),

        Commands::SetValue(a) => set_value::execute(
            set_value::SetValueArgs { ref_id: a.ref_id, value: a.value },
            adapter,
        ),

        Commands::Focus(a) => focus::execute(helpers::RefArgs { ref_id: a.ref_id }, adapter),
        Commands::Toggle(a) => toggle::execute(helpers::RefArgs { ref_id: a.ref_id }, adapter),
        Commands::Expand(a) => expand::execute(helpers::RefArgs { ref_id: a.ref_id }, adapter),
        Commands::Collapse(a) => {
            collapse::execute(helpers::RefArgs { ref_id: a.ref_id }, adapter)
        }

        Commands::Select(a) => select::execute(
            select::SelectArgs { ref_id: a.ref_id, value: a.value },
            adapter,
        ),

        Commands::Scroll(a) => scroll::execute(
            scroll::ScrollArgs {
                ref_id: a.ref_id,
                direction: parse_direction(&a.direction)?,
                amount: a.amount,
            },
            adapter,
        ),

        Commands::Press(a) => press::execute(press::PressArgs { combo: a.combo }, adapter),

        Commands::Launch(a) => {
            launch::execute(launch::LaunchArgs { app: a.app, wait: a.wait }, adapter)
        }

        Commands::CloseApp(a) => close_app::execute(
            close_app::CloseAppArgs { app: a.app, force: a.force },
            adapter,
        ),

        Commands::ListWindows(a) => {
            list_windows::execute(list_windows::ListWindowsArgs { app: a.app }, adapter)
        }

        Commands::ListApps => list_apps::execute(adapter),

        Commands::FocusWindow(a) => focus_window::execute(
            focus_window::FocusWindowArgs {
                window_id: a.window_id,
                app: a.app,
                title: a.title,
            },
            adapter,
        ),

        Commands::ClipboardGet => clipboard_get::execute(adapter),
        Commands::ClipboardSet(a) => clipboard_set::execute(a.text, adapter),

        Commands::Wait(a) => wait::execute(
            wait::WaitArgs {
                ms: a.ms,
                element: a.element,
                window: a.window,
                timeout_ms: a.timeout,
            },
            adapter,
        ),

        Commands::Status => status::execute(adapter),

        Commands::Permissions(a) => permissions::execute(
            permissions::PermissionsArgs { request: a.request },
            adapter,
        ),

        Commands::Version(a) => version::execute(version::VersionArgs { json: a.json }),

        Commands::Batch(a) => {
            let commands = batch::parse_commands(&a.commands_json)?;
            let mut results = Vec::new();
            for cmd in commands {
                let result = dispatch_batch_command(&cmd.command, cmd.args, adapter);
                let ok = result.is_ok();
                let entry = match result {
                    Ok(data) => {
                        serde_json::json!({ "ok": true, "command": cmd.command, "data": data })
                    }
                    Err(e) => {
                        serde_json::json!({ "ok": false, "command": cmd.command, "error": e.to_string() })
                    }
                };
                results.push(entry);
                if !ok && a.stop_on_error {
                    break;
                }
            }
            Ok(serde_json::json!({ "results": results }))
        }
    }
}

fn dispatch_batch_command(
    command: &str,
    args: serde_json::Value,
    adapter: &dyn agent_desktop_core::adapter::PlatformAdapter,
) -> Result<serde_json::Value, AppError> {
    fn str_field(v: &serde_json::Value, key: &str) -> Option<String> {
        v.get(key).and_then(|v| v.as_str()).map(String::from)
    }

    fn req_str(v: &serde_json::Value, key: &str) -> Result<String, AppError> {
        str_field(v, key)
            .ok_or_else(|| AppError::invalid_input(format!("Batch: missing required field '{key}'")))
    }

    match command {
        "snapshot" => snapshot::execute(
            snapshot::SnapshotArgs {
                app: str_field(&args, "app"),
                window_id: str_field(&args, "window_id"),
                max_depth: args.get("max_depth").and_then(|v| v.as_u64()).map(|v| v as u8).unwrap_or(10),
                include_bounds: args.get("include_bounds").and_then(|v| v.as_bool()).unwrap_or(false),
                interactive_only: args.get("interactive_only").and_then(|v| v.as_bool()).unwrap_or(false),
                compact: args.get("compact").and_then(|v| v.as_bool()).unwrap_or(false),
            },
            adapter,
        ),

        "find" => find::execute(
            find::FindArgs {
                app: str_field(&args, "app"),
                role: str_field(&args, "role"),
                name: str_field(&args, "name"),
                value: str_field(&args, "value"),
            },
            adapter,
        ),

        "screenshot" => screenshot::execute(
            screenshot::ScreenshotArgs {
                app: str_field(&args, "app"),
                window_id: str_field(&args, "window_id"),
                output_path: str_field(&args, "output_path").map(std::path::PathBuf::from),
            },
            adapter,
        ),

        "get" => get::execute(
            get::GetArgs {
                ref_id: req_str(&args, "ref_id")?,
                property: parse_get_property(
                    args.get("property").and_then(|v| v.as_str()).unwrap_or("text"),
                )?,
            },
            adapter,
        ),

        "is" => is_check::execute(
            is_check::IsArgs {
                ref_id: req_str(&args, "ref_id")?,
                property: parse_is_property(
                    args.get("property").and_then(|v| v.as_str()).unwrap_or("visible"),
                )?,
            },
            adapter,
        ),

        "click" => click::execute(click::ClickArgs { ref_id: req_str(&args, "ref_id")? }, adapter),
        "double-click" => double_click::execute(
            double_click::DoubleClickArgs { ref_id: req_str(&args, "ref_id")? },
            adapter,
        ),
        "right-click" => right_click::execute(
            right_click::RightClickArgs { ref_id: req_str(&args, "ref_id")? },
            adapter,
        ),
        "focus" => focus::execute(helpers::RefArgs { ref_id: req_str(&args, "ref_id")? }, adapter),
        "toggle" => {
            toggle::execute(helpers::RefArgs { ref_id: req_str(&args, "ref_id")? }, adapter)
        }
        "expand" => {
            expand::execute(helpers::RefArgs { ref_id: req_str(&args, "ref_id")? }, adapter)
        }
        "collapse" => {
            collapse::execute(helpers::RefArgs { ref_id: req_str(&args, "ref_id")? }, adapter)
        }

        "type" => type_text::execute(
            type_text::TypeArgs {
                ref_id: req_str(&args, "ref_id")?,
                text: req_str(&args, "text")?,
            },
            adapter,
        ),

        "set-value" => set_value::execute(
            set_value::SetValueArgs {
                ref_id: req_str(&args, "ref_id")?,
                value: req_str(&args, "value")?,
            },
            adapter,
        ),

        "select" => select::execute(
            select::SelectArgs {
                ref_id: req_str(&args, "ref_id")?,
                value: req_str(&args, "value")?,
            },
            adapter,
        ),

        "scroll" => scroll::execute(
            scroll::ScrollArgs {
                ref_id: req_str(&args, "ref_id")?,
                direction: parse_direction(
                    args.get("direction").and_then(|v| v.as_str()).unwrap_or("down"),
                )?,
                amount: args.get("amount").and_then(|v| v.as_u64()).map(|v| v as u32).unwrap_or(3),
            },
            adapter,
        ),

        "press" => press::execute(press::PressArgs { combo: req_str(&args, "combo")? }, adapter),

        "launch" => launch::execute(
            launch::LaunchArgs {
                app: req_str(&args, "app")?,
                wait: args.get("wait").and_then(|v| v.as_bool()).unwrap_or(false),
            },
            adapter,
        ),

        "close-app" => close_app::execute(
            close_app::CloseAppArgs {
                app: req_str(&args, "app")?,
                force: args.get("force").and_then(|v| v.as_bool()).unwrap_or(false),
            },
            adapter,
        ),

        "list-windows" => {
            list_windows::execute(list_windows::ListWindowsArgs { app: str_field(&args, "app") }, adapter)
        }

        "list-apps" => list_apps::execute(adapter),

        "focus-window" => focus_window::execute(
            focus_window::FocusWindowArgs {
                window_id: str_field(&args, "window_id"),
                app: str_field(&args, "app"),
                title: str_field(&args, "title"),
            },
            adapter,
        ),

        "clipboard-get" => clipboard_get::execute(adapter),
        "clipboard-set" => clipboard_set::execute(req_str(&args, "text")?, adapter),

        "wait" => wait::execute(
            wait::WaitArgs {
                ms: args.get("ms").and_then(|v| v.as_u64()),
                element: str_field(&args, "element"),
                window: str_field(&args, "window"),
                timeout_ms: args.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(30000),
            },
            adapter,
        ),

        "status" => status::execute(adapter),

        "permissions" => permissions::execute(
            permissions::PermissionsArgs {
                request: args.get("request").and_then(|v| v.as_bool()).unwrap_or(false),
            },
            adapter,
        ),

        "version" => version::execute(version::VersionArgs {
            json: args.get("json").and_then(|v| v.as_bool()).unwrap_or(false),
        }),

        other => Err(AppError::invalid_input(format!(
            "Unknown batch command '{other}'. Supported: snapshot, find, screenshot, get, is, \
             click, double-click, right-click, type, set-value, focus, select, toggle, expand, \
             collapse, scroll, press, launch, close-app, list-windows, list-apps, focus-window, \
             clipboard-get, clipboard-set, wait, status, permissions, version"
        ))),
    }
}

fn parse_get_property(s: &str) -> Result<get::GetProperty, AppError> {
    match s {
        "text" => Ok(get::GetProperty::Text),
        "value" => Ok(get::GetProperty::Value),
        "title" => Ok(get::GetProperty::Title),
        "bounds" => Ok(get::GetProperty::Bounds),
        "role" => Ok(get::GetProperty::Role),
        "states" => Ok(get::GetProperty::States),
        other => Err(AppError::invalid_input(format!(
            "Unknown property '{other}'. Valid: text, value, title, bounds, role, states"
        ))),
    }
}

fn parse_is_property(s: &str) -> Result<is_check::IsProperty, AppError> {
    match s {
        "visible" => Ok(is_check::IsProperty::Visible),
        "enabled" => Ok(is_check::IsProperty::Enabled),
        "checked" => Ok(is_check::IsProperty::Checked),
        "focused" => Ok(is_check::IsProperty::Focused),
        "expanded" => Ok(is_check::IsProperty::Expanded),
        other => Err(AppError::invalid_input(format!(
            "Unknown property '{other}'. Valid: visible, enabled, checked, focused, expanded"
        ))),
    }
}

fn parse_direction(s: &str) -> Result<Direction, AppError> {
    match s {
        "up" => Ok(Direction::Up),
        "down" => Ok(Direction::Down),
        "left" => Ok(Direction::Left),
        "right" => Ok(Direction::Right),
        other => Err(AppError::invalid_input(format!(
            "Unknown direction '{other}'. Valid: up, down, left, right"
        ))),
    }
}
