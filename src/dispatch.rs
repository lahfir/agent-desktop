use agent_desktop_core::{
    action::Direction,
    adapter::PlatformAdapter,
    commands::{
        batch, clipboard, click, close_app, collapse, expand, find, focus, focus_window, get,
        is_check, launch, list_apps, list_windows, permissions, press, screenshot, scroll, select,
        set_value, snapshot, status, toggle, type_text, version, wait,
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
            click::execute_double(click::ClickArgs { ref_id: a.ref_id }, adapter)
        }
        Commands::RightClick(a) => {
            click::execute_right(click::ClickArgs { ref_id: a.ref_id }, adapter)
        }

        Commands::Type(a) => type_text::execute(
            type_text::TypeArgs { ref_id: a.ref_id, text: a.text },
            adapter,
        ),

        Commands::SetValue(a) => set_value::execute(
            set_value::SetValueArgs { ref_id: a.ref_id, value: a.value },
            adapter,
        ),

        Commands::Focus(a) => focus::execute(focus::RefArgs { ref_id: a.ref_id }, adapter),
        Commands::Toggle(a) => toggle::execute(toggle::RefArgs { ref_id: a.ref_id }, adapter),
        Commands::Expand(a) => expand::execute(expand::RefArgs { ref_id: a.ref_id }, adapter),
        Commands::Collapse(a) => {
            collapse::execute(collapse::RefArgs { ref_id: a.ref_id }, adapter)
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

        Commands::ClipboardGet => clipboard::execute_get(adapter),
        Commands::ClipboardSet(a) => clipboard::execute_set(a.text, adapter),

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

        Commands::Batch(a) => batch::execute(
            batch::BatchArgs {
                commands_json: a.commands_json,
                stop_on_error: a.stop_on_error,
            },
            adapter,
        ),
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
