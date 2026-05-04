use clap::{Parser, Subcommand};

pub use crate::cli_args::*;
pub use crate::cli_args_notifications::*;
pub use crate::cli_args_skills::*;

const BEFORE_HELP: &str = include_str!("help_before.txt");
const AFTER_HELP: &str = include_str!("help_after.txt");

#[derive(Parser, Debug)]
#[command(
    name = "agent-desktop",
    about = "Desktop automation CLI for AI agents",
    long_about = None,
    before_help = BEFORE_HELP,
    after_help = AFTER_HELP,
)]
pub struct Cli {
    #[arg(
        long,
        short = 'v',
        global = true,
        help = "Enable debug logging to stderr"
    )]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Capture accessibility tree as structured JSON with @ref IDs")]
    Snapshot(SnapshotArgs),
    #[command(about = "Search elements by role, name, value, or text content")]
    Find(FindArgs),
    #[command(about = "Take a PNG screenshot of an application window")]
    Screenshot(ScreenshotArgs),
    #[command(about = "Read an element property (text, value, title, bounds, role, states)")]
    Get(GetArgs),
    #[command(about = "Check element state (visible, enabled, checked, focused, expanded)")]
    Is(IsArgs),
    #[command(about = "Click element via accessibility press action")]
    Click(RefArgs),
    #[command(about = "Double-click element")]
    DoubleClick(RefArgs),
    #[command(about = "Triple-click element to select line or paragraph")]
    TripleClick(RefArgs),
    #[command(about = "Right-click element to open context menu")]
    RightClick(RefArgs),
    #[command(about = "Focus element and type text")]
    Type(TypeArgs),
    #[command(about = "Set element value directly via accessibility attribute")]
    SetValue(SetValueArgs),
    #[command(about = "Clear element value to empty string")]
    Clear(RefArgs),
    #[command(about = "Set keyboard focus on element")]
    Focus(RefArgs),
    #[command(about = "Select an option in a list or dropdown")]
    Select(SelectArgs),
    #[command(about = "Toggle a checkbox or switch")]
    Toggle(RefArgs),
    #[command(about = "Set checkbox or switch to checked state (idempotent)")]
    Check(RefArgs),
    #[command(about = "Set checkbox or switch to unchecked state (idempotent)")]
    Uncheck(RefArgs),
    #[command(about = "Expand a disclosure triangle or tree item")]
    Expand(RefArgs),
    #[command(about = "Collapse a disclosure triangle or tree item")]
    Collapse(RefArgs),
    #[command(about = "Scroll element (--direction up/down/left/right, --amount N)")]
    Scroll(ScrollArgs),
    #[command(about = "Scroll element into visible area")]
    ScrollTo(RefArgs),
    #[command(about = "Send a key combo: return, escape, cmd+c, shift+tab ...")]
    Press(PressArgs),
    #[command(about = "Hold a key or modifier down")]
    KeyDown(KeyComboArgs),
    #[command(about = "Release a held key or modifier")]
    KeyUp(KeyComboArgs),
    #[command(about = "Move cursor to element center or coordinates")]
    Hover(HoverArgs),
    #[command(about = "Drag from one element or point to another")]
    Drag(DragCliArgs),
    #[command(about = "Move cursor to absolute screen coordinates")]
    MouseMove(MouseMoveArgs),
    #[command(about = "Click at absolute screen coordinates")]
    MouseClick(MouseClickArgs),
    #[command(about = "Press mouse button at coordinates (without releasing)")]
    MouseDown(MousePointArgs),
    #[command(about = "Release mouse button at coordinates")]
    MouseUp(MousePointArgs),
    #[command(about = "Launch application and wait until its window is visible")]
    Launch(LaunchArgs),
    #[command(about = "Quit an application gracefully (--force to kill)")]
    CloseApp(CloseAppArgs),
    #[command(about = "List all visible windows (--app to filter by application)")]
    ListWindows(ListWindowsArgs),
    #[command(about = "List all running GUI applications")]
    ListApps,
    #[command(about = "Bring a window to front (--app, --title, or --window-id)")]
    FocusWindow(FocusWindowArgs),
    #[command(about = "Resize application window")]
    ResizeWindow(ResizeWindowCliArgs),
    #[command(about = "Move application window to new position")]
    MoveWindow(MoveWindowCliArgs),
    #[command(about = "Minimize application window")]
    Minimize(AppRefArgs),
    #[command(about = "Maximize or zoom application window")]
    Maximize(AppRefArgs),
    #[command(about = "Restore minimized or maximized window")]
    Restore(AppRefArgs),
    #[command(about = "List accessibility surfaces for an app (window, menu, sheet ...)")]
    ListSurfaces(ListSurfacesArgs),
    #[command(about = "List notifications from Notification Center")]
    ListNotifications(ListNotificationsCliArgs),
    #[command(about = "Dismiss a notification by index")]
    DismissNotification(DismissNotificationCliArgs),
    #[command(about = "Dismiss all notifications (--app to filter)")]
    DismissAllNotifications(DismissAllNotificationsCliArgs),
    #[command(about = "Click an action button on a notification")]
    NotificationAction(NotificationActionCliArgs),
    #[command(about = "Read plain-text clipboard contents")]
    ClipboardGet,
    #[command(about = "Write text to the clipboard")]
    ClipboardSet(ClipboardSetArgs),
    #[command(about = "Clear the clipboard")]
    ClipboardClear,
    #[command(about = "Wait for time (ms), element presence, text, or window appearance")]
    Wait(WaitArgs),
    #[command(about = "Show adapter health, platform info, and permission state")]
    Status,
    #[command(about = "Check accessibility permission status (--request to prompt system dialog)")]
    Permissions(PermissionsArgs),
    #[command(about = "Show version (--json for machine-readable output)")]
    Version(VersionArgs),
    #[command(about = "Execute multiple commands from a JSON array (--stop-on-error)")]
    Batch(BatchArgs),
    #[command(about = "Bundled skill docs for AI agents (list, get, path)")]
    Skills(SkillsArgs),
}

impl Commands {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Snapshot(_) => "snapshot",
            Self::Find(_) => "find",
            Self::Screenshot(_) => "screenshot",
            Self::Get(_) => "get",
            Self::Is(_) => "is",
            Self::Click(_) => "click",
            Self::DoubleClick(_) => "double-click",
            Self::TripleClick(_) => "triple-click",
            Self::RightClick(_) => "right-click",
            Self::Type(_) => "type",
            Self::SetValue(_) => "set-value",
            Self::Clear(_) => "clear",
            Self::Focus(_) => "focus",
            Self::Select(_) => "select",
            Self::Toggle(_) => "toggle",
            Self::Check(_) => "check",
            Self::Uncheck(_) => "uncheck",
            Self::Expand(_) => "expand",
            Self::Collapse(_) => "collapse",
            Self::Scroll(_) => "scroll",
            Self::ScrollTo(_) => "scroll-to",
            Self::Press(_) => "press",
            Self::KeyDown(_) => "key-down",
            Self::KeyUp(_) => "key-up",
            Self::Hover(_) => "hover",
            Self::Drag(_) => "drag",
            Self::MouseMove(_) => "mouse-move",
            Self::MouseClick(_) => "mouse-click",
            Self::MouseDown(_) => "mouse-down",
            Self::MouseUp(_) => "mouse-up",
            Self::Launch(_) => "launch",
            Self::CloseApp(_) => "close-app",
            Self::ListWindows(_) => "list-windows",
            Self::ListApps => "list-apps",
            Self::FocusWindow(_) => "focus-window",
            Self::ResizeWindow(_) => "resize-window",
            Self::MoveWindow(_) => "move-window",
            Self::Minimize(_) => "minimize",
            Self::Maximize(_) => "maximize",
            Self::Restore(_) => "restore",
            Self::ListSurfaces(_) => "list-surfaces",
            Self::ListNotifications(_) => "list-notifications",
            Self::DismissNotification(_) => "dismiss-notification",
            Self::DismissAllNotifications(_) => "dismiss-all-notifications",
            Self::NotificationAction(_) => "notification-action",
            Self::ClipboardGet => "clipboard-get",
            Self::ClipboardSet(_) => "clipboard-set",
            Self::ClipboardClear => "clipboard-clear",
            Self::Wait(_) => "wait",
            Self::Status => "status",
            Self::Permissions(_) => "permissions",
            Self::Version(_) => "version",
            Self::Batch(_) => "batch",
            Self::Skills(_) => "skills",
        }
    }
}
