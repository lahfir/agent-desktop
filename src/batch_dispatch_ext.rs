use agent_desktop_core::{
    commands::{
        clipboard_clear, clipboard_get, clipboard_set, close_app, dismiss_all_notifications,
        dismiss_notification, focus_window, launch, list_apps, list_notifications, list_surfaces,
        list_windows, maximize, minimize, move_window, notification_action, permissions,
        resize_window, restore, status, version, wait,
    },
    error::AppError,
};
use serde_json::Value;

use crate::batch_dispatch::{req_str, str_field};

pub fn dispatch(
    command: &str,
    args: Value,
    adapter: &dyn agent_desktop_core::adapter::PlatformAdapter,
) -> Result<Value, AppError> {
    match command {
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

        "list-notifications" => list_notifications::execute(
            list_notifications::ListNotificationsArgs {
                app: str_field(&args, "app"),
                text: str_field(&args, "text"),
                limit: args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize),
            },
            adapter,
        ),

        "dismiss-notification" => {
            let index = args
                .get("index")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .ok_or_else(|| AppError::invalid_input("Batch: missing required field 'index'"))?;
            if index == 0 {
                return Err(AppError::invalid_input("Index must be >= 1"));
            }
            dismiss_notification::execute(
                dismiss_notification::DismissNotificationArgs {
                    index,
                    app: str_field(&args, "app"),
                },
                adapter,
            )
        }

        "dismiss-all-notifications" => dismiss_all_notifications::execute(
            dismiss_all_notifications::DismissAllNotificationsArgs {
                app: str_field(&args, "app"),
            },
            adapter,
        ),

        "notification-action" => {
            let index = args
                .get("index")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .ok_or_else(|| AppError::invalid_input("Batch: missing required field 'index'"))?;
            if index == 0 {
                return Err(AppError::invalid_input("Index must be >= 1"));
            }
            notification_action::execute(
                notification_action::NotificationActionArgs {
                    index,
                    action: req_str(&args, "action")?,
                },
                adapter,
            )
        }

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
                notification: args
                    .get("notification")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                poll_interval_ms: args
                    .get("poll_interval_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(3000),
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
