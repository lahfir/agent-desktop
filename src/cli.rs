use clap::{Parser, Subcommand};

pub use crate::cli_args::*;

#[derive(Parser, Debug)]
#[command(
    name = "agent-desktop",
    about = "Desktop automation CLI for AI agents",
    long_about = None,
    after_help = "\
OBSERVATION
  snapshot                   Accessibility tree as JSON with @ref IDs
  screenshot                 PNG screenshot of an application window
  find                       Search elements by role, name, value, or text
  get <ref> <property>       Read element property: text, value, title, bounds, role, states
  is <ref> <property>        Check state: visible, enabled, checked, focused, expanded
  list-surfaces              Available surfaces for an app

INTERACTION
  click <ref>                Click element (kAXPress)
  double-click <ref>         Double-click element
  triple-click <ref>         Triple-click element (select line/paragraph)
  right-click <ref>          Right-click and open context menu
  type <ref> <text>          Focus element and type text
  set-value <ref> <value>    Set value attribute directly
  clear <ref>                Clear element value to empty string
  focus <ref>                Set keyboard focus
  select <ref> <value>       Select option in list or dropdown
  toggle <ref>               Toggle checkbox or switch
  check <ref>                Set checkbox/switch to checked (idempotent)
  uncheck <ref>              Set checkbox/switch to unchecked (idempotent)
  expand <ref>               Expand disclosure triangle or tree item
  collapse <ref>             Collapse disclosure triangle or tree item
  scroll <ref>               Scroll element (--direction, --amount)
  scroll-to <ref>            Scroll element into visible area

KEYBOARD
  press <combo>              Key combo: return, escape, cmd+c, shift+tab ...
  key-down <combo>           Hold a key or modifier down
  key-up <combo>             Release a held key or modifier

MOUSE
  hover <ref|--xy>           Move cursor to element or coordinates
  drag                       Drag from one element/point to another
  mouse-move --xy x,y        Move cursor to absolute coordinates
  mouse-click --xy x,y       Click at coordinates (--button, --count)
  mouse-down --xy x,y        Press mouse button at coordinates
  mouse-up --xy x,y          Release mouse button at coordinates

APP & WINDOW
  launch <app>               Launch app and wait until window is visible
  close-app <app>            Quit app gracefully (--force to kill)
  list-windows               All visible windows (--app to filter)
  list-apps                  All running GUI applications
  focus-window               Bring window to front
  resize-window              Resize window (--width, --height)
  move-window                Move window (--x, --y)
  minimize                   Minimize window
  maximize                   Maximize/zoom window
  restore                    Restore minimized/maximized window

NOTIFICATIONS
  list-notifications         List notifications from Notification Center
  dismiss-notification <n>   Dismiss notification by index
  dismiss-all-notifications  Dismiss all notifications
  notification-action <n> <action>  Click action button on notification

CLIPBOARD
  clipboard-get              Read plain-text clipboard
  clipboard-set <text>       Write text to clipboard
  clipboard-clear            Clear the clipboard

WAIT
  wait [ms]                  Pause for N milliseconds
  wait --element <ref>       Block until element appears (--timeout ms)
  wait --window <title>      Block until window appears
  wait --text <text>         Block until text appears in app
  wait --notification        Block until a new notification arrives

SYSTEM
  status                     Adapter health, platform, and permission state
  permissions                Check accessibility permission (--request to prompt)
  version                    Version string (--json for machine-readable)

BATCH
  batch <json>               Run commands from a JSON array (--stop-on-error)

REF IDs
  snapshot assigns @e1, @e2, ... to interactive elements in depth-first order.
  Use a ref wherever <ref> appears. Refs are snapshot-scoped; run snapshot
  again after UI changes.

KEY COMBOS
  Single keys:               return, escape, tab, space, delete, up, down, left, right
  Function keys:             f1 - f12
  With modifiers:            cmd+c, cmd+v, cmd+z, cmd+shift+z, ctrl+a, shift+tab
  Modifiers:                 cmd, ctrl, alt, shift

EXAMPLES
  agent-desktop snapshot --app \"System Settings\" -i
  agent-desktop find --role button --name \"OK\"
  agent-desktop click @e5
  agent-desktop check @e3
  agent-desktop type @e2 \"hello@example.com\"
  agent-desktop press cmd+z
  agent-desktop drag --from @e1 --to @e5
  agent-desktop hover @e5
  agent-desktop minimize --app TextEdit
  agent-desktop resize-window --app TextEdit --width 800 --height 600
  agent-desktop mouse-click --xy 500,300
  agent-desktop wait --text \"Loading complete\" --app Safari --timeout 5000
  agent-desktop batch '[{\"command\":\"click\",\"args\":{\"ref_id\":\"@e1\"}}]'"
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
    ListNotifications(ListNotificationsArgs),
    #[command(about = "Dismiss a notification by index")]
    DismissNotification(DismissNotificationCliArgs),
    #[command(about = "Dismiss all notifications (--app to filter)")]
    DismissAllNotifications(DismissAllNotificationsArgs),
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
        }
    }
}
