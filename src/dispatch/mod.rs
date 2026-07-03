mod app_window;
mod clipboard;
mod interaction;
mod keyboard_mouse;
mod notifications;
mod observation;
mod parse;
mod session;
mod system;
mod trace;

use agent_desktop_core::{
    PermissionReport, adapter::PlatformAdapter, context::CommandContext, error::AppError,
};
use serde_json::Value;

use crate::cli::Commands;

pub(crate) fn dispatch(
    cmd: Commands,
    adapter: &dyn PlatformAdapter,
    permission_report: &PermissionReport,
    context: &CommandContext,
) -> Result<Value, AppError> {
    tracing::debug!("dispatch: {}", cmd.name());
    let scope = context.command_scope(cmd.name());
    let result = dispatch_inner(cmd, adapter, permission_report, context);
    scope.complete(&result);
    result
}

fn dispatch_inner(
    cmd: Commands,
    adapter: &dyn PlatformAdapter,
    permission_report: &PermissionReport,
    context: &CommandContext,
) -> Result<Value, AppError> {
    match cmd {
        Commands::Snapshot(_)
        | Commands::Find(_)
        | Commands::Screenshot(_)
        | Commands::Get(_)
        | Commands::Is(_) => observation::dispatch(cmd, adapter, context),

        Commands::Click(_)
        | Commands::DoubleClick(_)
        | Commands::TripleClick(_)
        | Commands::RightClick(_)
        | Commands::Type(_)
        | Commands::SetValue(_)
        | Commands::Clear(_)
        | Commands::Focus(_)
        | Commands::Toggle(_)
        | Commands::Check(_)
        | Commands::Uncheck(_)
        | Commands::Expand(_)
        | Commands::Collapse(_)
        | Commands::Select(_)
        | Commands::Scroll(_)
        | Commands::ScrollTo(_) => interaction::dispatch(cmd, adapter, context),

        Commands::Press(_)
        | Commands::KeyDown(_)
        | Commands::KeyUp(_)
        | Commands::Hover(_)
        | Commands::Drag(_)
        | Commands::MouseMove(_)
        | Commands::MouseClick(_)
        | Commands::MouseDown(_)
        | Commands::MouseUp(_)
        | Commands::MouseWheel(_) => keyboard_mouse::dispatch(cmd, adapter, context),

        Commands::Launch(_)
        | Commands::CloseApp(_)
        | Commands::ListWindows(_)
        | Commands::ListDisplays
        | Commands::ListApps(_)
        | Commands::ListSurfaces(_)
        | Commands::FocusWindow(_)
        | Commands::ResizeWindow(_)
        | Commands::MoveWindow(_)
        | Commands::Minimize(_)
        | Commands::Maximize(_)
        | Commands::Restore(_) => app_window::dispatch(cmd, adapter),

        Commands::ListNotifications(_)
        | Commands::DismissNotification(_)
        | Commands::DismissAllNotifications(_)
        | Commands::NotificationAction(_) => notifications::dispatch_notification(cmd, adapter),

        Commands::ClipboardGet(_) | Commands::ClipboardSet(_) | Commands::ClipboardClear => {
            clipboard::dispatch(cmd, adapter, context)
        }

        Commands::Wait(_)
        | Commands::Status
        | Commands::Permissions(_)
        | Commands::Version
        | Commands::Skills(_)
        | Commands::Session(_)
        | Commands::Trace(_)
        | Commands::Batch(_) => system::dispatch(cmd, adapter, permission_report, context),
    }
}
