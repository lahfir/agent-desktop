use agent_desktop_core::{
    commands::{
        check, clear, click, clipboard_clear, clipboard_get, clipboard_set, close_app, collapse,
        double_click, drag, expand, find, focus, focus_window, get, helpers, hover, is_check,
        key_down, key_up, launch, list_apps, list_surfaces, list_windows, maximize, minimize,
        mouse_click, mouse_down, mouse_move, mouse_up, move_window, permissions, press,
        resize_window, restore, right_click, screenshot, scroll, scroll_to, select, set_value,
        snapshot, status, toggle, triple_click, type_text, uncheck, version, wait,
    },
    error::AppError,
};
use serde_json::Value;

use crate::dispatch::{parse_direction, parse_mouse_button, parse_xy};

pub fn dispatch_batch_command(
    command: &str,
    args: Value,
    adapter: &dyn agent_desktop_core::adapter::PlatformAdapter,
) -> Result<Value, AppError> {
    fn str_field(v: &Value, key: &str) -> Option<String> {
        v.get(key).and_then(|v| v.as_str()).map(String::from)
    }

    fn req_str(v: &Value, key: &str) -> Result<String, AppError> {
        str_field(v, key).ok_or_else(|| {
            AppError::invalid_input(format!("Batch: missing required field '{key}'"))
        })
    }

    match command {
        "snapshot" => snapshot::execute(
            snapshot::SnapshotArgs {
                app: str_field(&args, "app"),
                window_id: str_field(&args, "window_id"),
                max_depth: args
                    .get("max_depth")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u8)
                    .unwrap_or(10),
                include_bounds: args
                    .get("include_bounds")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                interactive_only: args
                    .get("interactive_only")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                compact: args
                    .get("compact")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                surface: parse_batch_surface(args.get("surface").and_then(|v| v.as_str())),
            },
            adapter,
        ),

        "find" => find::execute(
            find::FindArgs {
                app: str_field(&args, "app"),
                role: str_field(&args, "role"),
                name: str_field(&args, "name"),
                value: str_field(&args, "value"),
                text: str_field(&args, "text"),
                count: args.get("count").and_then(|v| v.as_bool()).unwrap_or(false),
                first: args.get("first").and_then(|v| v.as_bool()).unwrap_or(false),
                last: args.get("last").and_then(|v| v.as_bool()).unwrap_or(false),
                nth: args.get("nth").and_then(|v| v.as_u64()).map(|v| v as usize),
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
                property: crate::dispatch::parse_get_property(
                    args.get("property")
                        .and_then(|v| v.as_str())
                        .unwrap_or("text"),
                )?,
            },
            adapter,
        ),

        "is" => is_check::execute(
            is_check::IsArgs {
                ref_id: req_str(&args, "ref_id")?,
                property: crate::dispatch::parse_is_property(
                    args.get("property")
                        .and_then(|v| v.as_str())
                        .unwrap_or("visible"),
                )?,
            },
            adapter,
        ),

        "click" => click::execute(
            click::ClickArgs {
                ref_id: req_str(&args, "ref_id")?,
            },
            adapter,
        ),
        "double-click" => double_click::execute(
            double_click::DoubleClickArgs {
                ref_id: req_str(&args, "ref_id")?,
            },
            adapter,
        ),
        "triple-click" => triple_click::execute(
            triple_click::TripleClickArgs {
                ref_id: req_str(&args, "ref_id")?,
            },
            adapter,
        ),
        "right-click" => right_click::execute(
            right_click::RightClickArgs {
                ref_id: req_str(&args, "ref_id")?,
            },
            adapter,
        ),
        "focus" => focus::execute(
            helpers::RefArgs {
                ref_id: req_str(&args, "ref_id")?,
            },
            adapter,
        ),
        "toggle" => toggle::execute(
            helpers::RefArgs {
                ref_id: req_str(&args, "ref_id")?,
            },
            adapter,
        ),
        "check" => check::execute(
            check::CheckArgs {
                ref_id: req_str(&args, "ref_id")?,
            },
            adapter,
        ),
        "uncheck" => uncheck::execute(
            uncheck::UncheckArgs {
                ref_id: req_str(&args, "ref_id")?,
            },
            adapter,
        ),
        "expand" => expand::execute(
            helpers::RefArgs {
                ref_id: req_str(&args, "ref_id")?,
            },
            adapter,
        ),
        "collapse" => collapse::execute(
            helpers::RefArgs {
                ref_id: req_str(&args, "ref_id")?,
            },
            adapter,
        ),
        "clear" => clear::execute(
            clear::ClearArgs {
                ref_id: req_str(&args, "ref_id")?,
            },
            adapter,
        ),
        "scroll-to" => scroll_to::execute(
            scroll_to::ScrollToArgs {
                ref_id: req_str(&args, "ref_id")?,
            },
            adapter,
        ),

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
                    args.get("direction")
                        .and_then(|v| v.as_str())
                        .unwrap_or("down"),
                )?,
                amount: args
                    .get("amount")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32)
                    .unwrap_or(3),
            },
            adapter,
        ),

        "press" => press::execute(
            press::PressArgs {
                combo: req_str(&args, "combo")?,
                app: str_field(&args, "app"),
            },
            adapter,
        ),

        "key-down" => key_down::execute(
            key_down::KeyDownArgs {
                combo: req_str(&args, "combo")?,
            },
            adapter,
        ),

        "key-up" => key_up::execute(
            key_up::KeyUpArgs {
                combo: req_str(&args, "combo")?,
            },
            adapter,
        ),

        "hover" => {
            let xy = str_field(&args, "xy").map(|s| parse_xy(&s)).transpose()?;
            hover::execute(
                hover::HoverArgs {
                    ref_id: str_field(&args, "ref_id"),
                    xy,
                    duration_ms: args.get("duration_ms").and_then(|v| v.as_u64()),
                },
                adapter,
            )
        }

        "drag" => {
            let from_xy = str_field(&args, "from_xy")
                .map(|s| parse_xy(&s))
                .transpose()?;
            let to_xy = str_field(&args, "to_xy")
                .map(|s| parse_xy(&s))
                .transpose()?;
            drag::execute(
                drag::DragArgs {
                    from_ref: str_field(&args, "from"),
                    from_xy,
                    to_ref: str_field(&args, "to"),
                    to_xy,
                    duration_ms: args.get("duration_ms").and_then(|v| v.as_u64()),
                },
                adapter,
            )
        }

        "mouse-move" => {
            let xy = req_str(&args, "xy")?;
            let (x, y) = parse_xy(&xy)?;
            mouse_move::execute(mouse_move::MouseMoveArgs { x, y }, adapter)
        }

        "mouse-click" => {
            let xy = req_str(&args, "xy")?;
            let (x, y) = parse_xy(&xy)?;
            let button = args
                .get("button")
                .and_then(|v| v.as_str())
                .unwrap_or("left");
            mouse_click::execute(
                mouse_click::MouseClickArgs {
                    x,
                    y,
                    button: parse_mouse_button(button)?,
                    count: args.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as u32,
                },
                adapter,
            )
        }

        "mouse-down" => {
            let xy = req_str(&args, "xy")?;
            let (x, y) = parse_xy(&xy)?;
            let button = args
                .get("button")
                .and_then(|v| v.as_str())
                .unwrap_or("left");
            mouse_down::execute(
                mouse_down::MouseDownArgs {
                    x,
                    y,
                    button: parse_mouse_button(button)?,
                },
                adapter,
            )
        }

        "mouse-up" => {
            let xy = req_str(&args, "xy")?;
            let (x, y) = parse_xy(&xy)?;
            let button = args
                .get("button")
                .and_then(|v| v.as_str())
                .unwrap_or("left");
            mouse_up::execute(
                mouse_up::MouseUpArgs {
                    x,
                    y,
                    button: parse_mouse_button(button)?,
                },
                adapter,
            )
        }

        "launch" => launch::execute(
            launch::LaunchArgs {
                app: req_str(&args, "app")?,
                timeout_ms: args
                    .get("timeout")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(30000),
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

        "list-windows" => list_windows::execute(
            list_windows::ListWindowsArgs {
                app: str_field(&args, "app"),
            },
            adapter,
        ),

        "list-apps" => list_apps::execute(adapter),

        "focus-window" => focus_window::execute(
            focus_window::FocusWindowArgs {
                window_id: str_field(&args, "window_id"),
                app: str_field(&args, "app"),
                title: str_field(&args, "title"),
            },
            adapter,
        ),

        "resize-window" => resize_window::execute(
            resize_window::ResizeWindowArgs {
                app: str_field(&args, "app"),
                width: args.get("width").and_then(|v| v.as_f64()).unwrap_or(800.0),
                height: args.get("height").and_then(|v| v.as_f64()).unwrap_or(600.0),
            },
            adapter,
        ),

        "move-window" => move_window::execute(
            move_window::MoveWindowArgs {
                app: str_field(&args, "app"),
                x: args.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0),
                y: args.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
            },
            adapter,
        ),

        "minimize" => minimize::execute(
            minimize::MinimizeArgs {
                app: str_field(&args, "app"),
            },
            adapter,
        ),

        "maximize" => maximize::execute(
            maximize::MaximizeArgs {
                app: str_field(&args, "app"),
            },
            adapter,
        ),

        "restore" => restore::execute(
            restore::RestoreArgs {
                app: str_field(&args, "app"),
            },
            adapter,
        ),

        "clipboard-get" => clipboard_get::execute(adapter),
        "clipboard-set" => clipboard_set::execute(req_str(&args, "text")?, adapter),
        "clipboard-clear" => clipboard_clear::execute(adapter),

        "wait" => wait::execute(
            wait::WaitArgs {
                ms: args.get("ms").and_then(|v| v.as_u64()),
                element: str_field(&args, "element"),
                window: str_field(&args, "window"),
                text: str_field(&args, "text"),
                timeout_ms: args
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(30000),
                menu: args.get("menu").and_then(|v| v.as_bool()).unwrap_or(false),
                menu_closed: args
                    .get("menu_closed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                app: str_field(&args, "app"),
            },
            adapter,
        ),

        "list-surfaces" => list_surfaces::execute(
            list_surfaces::ListSurfacesArgs {
                app: str_field(&args, "app"),
            },
            adapter,
        ),

        "status" => status::execute(adapter),

        "permissions" => permissions::execute(
            permissions::PermissionsArgs {
                request: args
                    .get("request")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            },
            adapter,
        ),

        "version" => version::execute(version::VersionArgs {
            json: args.get("json").and_then(|v| v.as_bool()).unwrap_or(false),
        }),

        other => Err(AppError::invalid_input(format!(
            "Unknown batch command '{other}'"
        ))),
    }
}

fn parse_batch_surface(s: Option<&str>) -> agent_desktop_core::adapter::SnapshotSurface {
    use agent_desktop_core::adapter::SnapshotSurface;
    match s {
        Some("menu") => SnapshotSurface::Menu,
        Some("sheet") => SnapshotSurface::Sheet,
        Some("popover") => SnapshotSurface::Popover,
        Some("alert") => SnapshotSurface::Alert,
        Some("focused") => SnapshotSurface::Focused,
        _ => SnapshotSurface::Window,
    }
}
