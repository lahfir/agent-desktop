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
    AppError, PermissionReport, adapter::PlatformAdapter, context::CommandContext,
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
    let scope = if cmd.is_mutating() {
        context.mutating_command_scope(cmd.name())?
    } else {
        context.command_scope(cmd.name())?
    };
    let result = match cmd {
        Commands::Snapshot(args) => observation::snapshot(args, adapter, context),
        Commands::Find(args) => observation::find(args, adapter, context),
        Commands::Screenshot(args) => observation::screenshot(args, adapter),
        Commands::Get(args) => observation::get(args, adapter, context),
        Commands::Is(args) => observation::is(args, adapter, context),
        Commands::Click(args) => interaction::click(args, adapter, context),
        Commands::DoubleClick(args) => interaction::double_click(args, adapter, context),
        Commands::TripleClick(args) => interaction::triple_click(args, adapter, context),
        Commands::RightClick(args) => interaction::right_click(args, adapter, context),
        Commands::Type(args) => interaction::type_text(args, adapter, context),
        Commands::SetValue(args) => interaction::set_value(args, adapter, context),
        Commands::Clear(args) => interaction::clear(args, adapter, context),
        Commands::Focus(args) => interaction::focus(args, adapter, context),
        Commands::Select(args) => interaction::select(args, adapter, context),
        Commands::Toggle(args) => interaction::toggle(args, adapter, context),
        Commands::Check(args) => interaction::check(args, adapter, context),
        Commands::Uncheck(args) => interaction::uncheck(args, adapter, context),
        Commands::Expand(args) => interaction::expand(args, adapter, context),
        Commands::Collapse(args) => interaction::collapse(args, adapter, context),
        Commands::Scroll(args) => interaction::scroll(args, adapter, context),
        Commands::ScrollTo(args) => interaction::scroll_to(args, adapter, context),
        Commands::Press(args) => keyboard_mouse::press(args, adapter, context),
        Commands::KeyDown(args) => keyboard_mouse::key_down(args, adapter),
        Commands::KeyUp(args) => keyboard_mouse::key_up(args, adapter),
        Commands::Hover(args) => keyboard_mouse::hover(args, adapter, context),
        Commands::Drag(args) => keyboard_mouse::drag(args, adapter, context),
        Commands::MouseMove(args) => keyboard_mouse::mouse_move(args, adapter, context),
        Commands::MouseClick(args) => keyboard_mouse::mouse_click(args, adapter, context),
        Commands::MouseDown(args) => keyboard_mouse::mouse_down(args, adapter, context),
        Commands::MouseUp(args) => keyboard_mouse::mouse_up(args, adapter, context),
        Commands::MouseWheel(args) => keyboard_mouse::mouse_wheel(args, adapter, context),
        Commands::Launch(args) => app_window::launch(args, adapter),
        Commands::CloseApp(args) => app_window::close_app(args, adapter),
        Commands::ListWindows(args) => app_window::list_windows(args, adapter),
        Commands::ListDisplays => app_window::list_displays(adapter),
        Commands::ListApps(args) => app_window::list_apps(args, adapter),
        Commands::FocusWindow(args) => app_window::focus_window(args, adapter),
        Commands::ResizeWindow(args) => app_window::resize_window(args, adapter),
        Commands::MoveWindow(args) => app_window::move_window(args, adapter),
        Commands::Minimize(args) => app_window::minimize(args, adapter),
        Commands::Maximize(args) => app_window::maximize(args, adapter),
        Commands::Restore(args) => app_window::restore(args, adapter),
        Commands::ListSurfaces(args) => app_window::list_surfaces(args, adapter),
        Commands::ListNotifications(args) => notifications::list(args, adapter, context),
        Commands::DismissNotification(args) => notifications::dismiss(args, adapter, context),
        Commands::DismissAllNotifications(args) => {
            notifications::dismiss_all(args, adapter, context)
        }
        Commands::NotificationAction(args) => notifications::action(args, adapter, context),
        Commands::ClipboardGet(args) => clipboard::get(args, adapter, context),
        Commands::ClipboardSet(args) => clipboard::set(args, adapter),
        Commands::ClipboardClear => clipboard::clear(adapter),
        Commands::Wait(args) => system::wait(args, adapter, context),
        Commands::Status => system::status(adapter, permission_report, context),
        Commands::Permissions(args) => system::permissions(args, adapter, permission_report),
        Commands::Version => system::version(),
        Commands::Batch(args) => system::batch(args, adapter, permission_report, context),
        Commands::Skills(args) => system::skills(args),
        Commands::Session(args) => system::session(args, context),
        Commands::Trace(args) => system::trace(args, context),
    };
    scope.complete(&result)?;
    result
}
