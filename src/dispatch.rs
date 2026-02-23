use agent_desktop_core::{
    action::{Direction, MouseButton},
    adapter::PlatformAdapter,
    commands::{
        batch, check, clear, click, clipboard_clear, clipboard_get, clipboard_set, close_app,
        collapse, double_click, drag, expand, find, focus, focus_window, get, helpers, hover,
        is_check, key_down, key_up, launch, list_apps, list_surfaces, list_windows, maximize,
        minimize, mouse_click, mouse_down, mouse_move, mouse_up, move_window, permissions, press,
        resize_window, restore, right_click, screenshot, scroll, scroll_to, select, set_value,
        snapshot, status, toggle, triple_click, type_text, uncheck, version, wait,
    },
    error::AppError,
};
use serde_json::Value;

use crate::cli::Commands;

pub fn dispatch(cmd: Commands, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    tracing::debug!("dispatch: {}", command_name(&cmd));
    match cmd {
        Commands::Snapshot(a) => snapshot::execute(
            snapshot::SnapshotArgs {
                app: a.app,
                window_id: a.window_id,
                max_depth: a.max_depth,
                include_bounds: a.include_bounds,
                interactive_only: a.interactive_only,
                compact: a.compact,
                surface: cli_surface_to_core(&a.surface),
            },
            adapter,
        ),

        Commands::Find(a) => find::execute(
            find::FindArgs {
                app: a.app,
                role: a.role,
                name: a.name,
                value: a.value,
                text: a.text,
                count: a.count,
                first: a.first,
                last: a.last,
                nth: a.nth,
            },
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
        Commands::TripleClick(a) => {
            triple_click::execute(triple_click::TripleClickArgs { ref_id: a.ref_id }, adapter)
        }
        Commands::RightClick(a) => {
            right_click::execute(right_click::RightClickArgs { ref_id: a.ref_id }, adapter)
        }

        Commands::Type(a) => type_text::execute(
            type_text::TypeArgs {
                ref_id: a.ref_id,
                text: a.text,
            },
            adapter,
        ),

        Commands::SetValue(a) => set_value::execute(
            set_value::SetValueArgs {
                ref_id: a.ref_id,
                value: a.value,
            },
            adapter,
        ),

        Commands::Clear(a) => clear::execute(clear::ClearArgs { ref_id: a.ref_id }, adapter),

        Commands::Focus(a) => focus::execute(helpers::RefArgs { ref_id: a.ref_id }, adapter),
        Commands::Toggle(a) => toggle::execute(helpers::RefArgs { ref_id: a.ref_id }, adapter),
        Commands::Check(a) => check::execute(check::CheckArgs { ref_id: a.ref_id }, adapter),
        Commands::Uncheck(a) => {
            uncheck::execute(uncheck::UncheckArgs { ref_id: a.ref_id }, adapter)
        }
        Commands::Expand(a) => expand::execute(helpers::RefArgs { ref_id: a.ref_id }, adapter),
        Commands::Collapse(a) => collapse::execute(helpers::RefArgs { ref_id: a.ref_id }, adapter),

        Commands::Select(a) => select::execute(
            select::SelectArgs {
                ref_id: a.ref_id,
                value: a.value,
            },
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

        Commands::ScrollTo(a) => {
            scroll_to::execute(scroll_to::ScrollToArgs { ref_id: a.ref_id }, adapter)
        }

        Commands::Press(a) => press::execute(
            press::PressArgs {
                combo: a.combo,
                app: a.app,
            },
            adapter,
        ),

        Commands::KeyDown(a) => {
            key_down::execute(key_down::KeyDownArgs { combo: a.combo }, adapter)
        }

        Commands::KeyUp(a) => key_up::execute(key_up::KeyUpArgs { combo: a.combo }, adapter),

        Commands::Hover(a) => hover::execute(
            hover::HoverArgs {
                ref_id: a.ref_id,
                xy: parse_xy_opt(a.xy.as_deref())?,
                duration_ms: a.duration,
            },
            adapter,
        ),

        Commands::Drag(a) => drag::execute(
            drag::DragArgs {
                from_ref: a.from,
                from_xy: parse_xy_opt(a.from_xy.as_deref())?,
                to_ref: a.to,
                to_xy: parse_xy_opt(a.to_xy.as_deref())?,
                duration_ms: a.duration,
            },
            adapter,
        ),

        Commands::MouseMove(a) => {
            let (x, y) = parse_xy(&a.xy)?;
            mouse_move::execute(mouse_move::MouseMoveArgs { x, y }, adapter)
        }

        Commands::MouseClick(a) => {
            let (x, y) = parse_xy(&a.xy)?;
            mouse_click::execute(
                mouse_click::MouseClickArgs {
                    x,
                    y,
                    button: parse_mouse_button(&a.button)?,
                    count: a.count,
                },
                adapter,
            )
        }

        Commands::MouseDown(a) => {
            let (x, y) = parse_xy(&a.xy)?;
            mouse_down::execute(
                mouse_down::MouseDownArgs {
                    x,
                    y,
                    button: parse_mouse_button(&a.button)?,
                },
                adapter,
            )
        }

        Commands::MouseUp(a) => {
            let (x, y) = parse_xy(&a.xy)?;
            mouse_up::execute(
                mouse_up::MouseUpArgs {
                    x,
                    y,
                    button: parse_mouse_button(&a.button)?,
                },
                adapter,
            )
        }

        Commands::Launch(a) => launch::execute(
            launch::LaunchArgs {
                app: a.app,
                timeout_ms: a.timeout,
            },
            adapter,
        ),

        Commands::CloseApp(a) => close_app::execute(
            close_app::CloseAppArgs {
                app: a.app,
                force: a.force,
            },
            adapter,
        ),

        Commands::ListWindows(a) => {
            list_windows::execute(list_windows::ListWindowsArgs { app: a.app }, adapter)
        }

        Commands::ListApps => list_apps::execute(adapter),

        Commands::ListSurfaces(a) => {
            list_surfaces::execute(list_surfaces::ListSurfacesArgs { app: a.app }, adapter)
        }

        Commands::FocusWindow(a) => focus_window::execute(
            focus_window::FocusWindowArgs {
                window_id: a.window_id,
                app: a.app,
                title: a.title,
            },
            adapter,
        ),

        Commands::ResizeWindow(a) => resize_window::execute(
            resize_window::ResizeWindowArgs {
                app: a.app,
                width: a.width,
                height: a.height,
            },
            adapter,
        ),

        Commands::MoveWindow(a) => move_window::execute(
            move_window::MoveWindowArgs {
                app: a.app,
                x: a.x,
                y: a.y,
            },
            adapter,
        ),

        Commands::Minimize(a) => minimize::execute(minimize::MinimizeArgs { app: a.app }, adapter),

        Commands::Maximize(a) => maximize::execute(maximize::MaximizeArgs { app: a.app }, adapter),

        Commands::Restore(a) => restore::execute(restore::RestoreArgs { app: a.app }, adapter),

        Commands::ClipboardGet => clipboard_get::execute(adapter),
        Commands::ClipboardSet(a) => clipboard_set::execute(a.text, adapter),
        Commands::ClipboardClear => clipboard_clear::execute(adapter),

        Commands::Wait(a) => wait::execute(
            wait::WaitArgs {
                ms: a.ms,
                element: a.element,
                window: a.window,
                text: a.text,
                timeout_ms: a.timeout,
                menu: a.menu,
                menu_closed: a.menu_closed,
                app: a.app,
            },
            adapter,
        ),

        Commands::Status => status::execute(adapter),

        Commands::Permissions(a) => {
            permissions::execute(permissions::PermissionsArgs { request: a.request }, adapter)
        }

        Commands::Version(a) => version::execute(version::VersionArgs { json: a.json }),

        Commands::Batch(a) => {
            let commands = batch::parse_commands(&a.commands_json)?;
            let mut results = Vec::new();
            for cmd in commands {
                let result =
                    crate::batch_dispatch::dispatch_batch_command(&cmd.command, cmd.args, adapter);
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

pub(crate) fn parse_get_property(s: &str) -> Result<get::GetProperty, AppError> {
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

pub(crate) fn parse_is_property(s: &str) -> Result<is_check::IsProperty, AppError> {
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

pub(crate) fn parse_direction(s: &str) -> Result<Direction, AppError> {
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

pub(crate) fn parse_mouse_button(s: &str) -> Result<MouseButton, AppError> {
    match s {
        "left" => Ok(MouseButton::Left),
        "right" => Ok(MouseButton::Right),
        "middle" => Ok(MouseButton::Middle),
        other => Err(AppError::invalid_input(format!(
            "Unknown button '{other}'. Valid: left, right, middle"
        ))),
    }
}

pub(crate) fn parse_xy(s: &str) -> Result<(f64, f64), AppError> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err(AppError::invalid_input(format!(
            "Invalid coordinates '{s}'. Expected format: x,y (e.g., 500,300)"
        )));
    }
    let x: f64 = parts[0]
        .trim()
        .parse()
        .map_err(|_| AppError::invalid_input(format!("Invalid x coordinate: '{}'", parts[0])))?;
    let y: f64 = parts[1]
        .trim()
        .parse()
        .map_err(|_| AppError::invalid_input(format!("Invalid y coordinate: '{}'", parts[1])))?;
    Ok((x, y))
}

fn parse_xy_opt(s: Option<&str>) -> Result<Option<(f64, f64)>, AppError> {
    match s {
        Some(s) => parse_xy(s).map(Some),
        None => Ok(None),
    }
}

fn command_name(cmd: &Commands) -> &'static str {
    match cmd {
        Commands::Snapshot(_) => "snapshot",
        Commands::Find(_) => "find",
        Commands::Screenshot(_) => "screenshot",
        Commands::Get(_) => "get",
        Commands::Is(_) => "is",
        Commands::Click(_) => "click",
        Commands::DoubleClick(_) => "double-click",
        Commands::TripleClick(_) => "triple-click",
        Commands::RightClick(_) => "right-click",
        Commands::Type(_) => "type",
        Commands::SetValue(_) => "set-value",
        Commands::Clear(_) => "clear",
        Commands::Focus(_) => "focus",
        Commands::Toggle(_) => "toggle",
        Commands::Check(_) => "check",
        Commands::Uncheck(_) => "uncheck",
        Commands::Expand(_) => "expand",
        Commands::Collapse(_) => "collapse",
        Commands::Select(_) => "select",
        Commands::Scroll(_) => "scroll",
        Commands::ScrollTo(_) => "scroll-to",
        Commands::Press(_) => "press",
        Commands::KeyDown(_) => "key-down",
        Commands::KeyUp(_) => "key-up",
        Commands::Hover(_) => "hover",
        Commands::Drag(_) => "drag",
        Commands::MouseMove(_) => "mouse-move",
        Commands::MouseClick(_) => "mouse-click",
        Commands::MouseDown(_) => "mouse-down",
        Commands::MouseUp(_) => "mouse-up",
        Commands::Launch(_) => "launch",
        Commands::CloseApp(_) => "close-app",
        Commands::ListWindows(_) => "list-windows",
        Commands::ListApps => "list-apps",
        Commands::ListSurfaces(_) => "list-surfaces",
        Commands::FocusWindow(_) => "focus-window",
        Commands::ResizeWindow(_) => "resize-window",
        Commands::MoveWindow(_) => "move-window",
        Commands::Minimize(_) => "minimize",
        Commands::Maximize(_) => "maximize",
        Commands::Restore(_) => "restore",
        Commands::ClipboardGet => "clipboard-get",
        Commands::ClipboardSet(_) => "clipboard-set",
        Commands::ClipboardClear => "clipboard-clear",
        Commands::Wait(_) => "wait",
        Commands::Status => "status",
        Commands::Permissions(_) => "permissions",
        Commands::Version(_) => "version",
        Commands::Batch(_) => "batch",
    }
}

fn cli_surface_to_core(s: &crate::cli::Surface) -> agent_desktop_core::adapter::SnapshotSurface {
    use crate::cli::Surface;
    use agent_desktop_core::adapter::SnapshotSurface;
    match s {
        Surface::Window => SnapshotSurface::Window,
        Surface::Focused => SnapshotSurface::Focused,
        Surface::Menu => SnapshotSurface::Menu,
        Surface::Menubar => SnapshotSurface::Menubar,
        Surface::Sheet => SnapshotSurface::Sheet,
        Surface::Popover => SnapshotSurface::Popover,
        Surface::Alert => SnapshotSurface::Alert,
    }
}
