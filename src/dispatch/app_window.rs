use agent_desktop_core::{
    commands::{
        close_app, focus_window, helpers, launch, list_apps, list_displays, list_surfaces,
        list_windows, maximize, minimize, move_window, resize_window, restore,
    },
    error::{AppError, ErrorCode},
};
use serde_json::Value;

use crate::cli::Commands;
use crate::dispatch::parse::build_launch_options;

pub(super) fn dispatch(
    cmd: Commands,
    adapter: &dyn agent_desktop_core::adapter::PlatformAdapter,
) -> Result<Value, AppError> {
    match cmd {
        Commands::Launch(a) => launch::execute(
            launch::LaunchArgs {
                app: a.app,
                timeout_ms: a.timeout,
                options: build_launch_options(&a.args, &a.env, a.cwd, a.no_attach)?,
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

        Commands::ListDisplays => list_displays::execute(adapter),

        Commands::ListApps(a) => {
            list_apps::execute(list_apps::ListAppsArgs { app: a.app }, adapter)
        }

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

        Commands::Minimize(a) => minimize::execute(helpers::AppArgs { app: a.app }, adapter),

        Commands::Maximize(a) => maximize::execute(helpers::AppArgs { app: a.app }, adapter),

        Commands::Restore(a) => restore::execute(helpers::AppArgs { app: a.app }, adapter),

        _ => Err(AppError::Adapter(
            agent_desktop_core::error::AdapterError::new(
                ErrorCode::InvalidArgs,
                "app_window::dispatch received a non-app/window command",
            ),
        )),
    }
}
