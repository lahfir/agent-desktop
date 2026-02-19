use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "agent-desktop",
    version,
    about = "Desktop automation for AI agents",
    after_help = "\
CATEGORIES:
  Observation:  snapshot, find, screenshot, get, is
  Interaction:  click, double-click, right-click, type, set-value, focus, select,
                toggle, expand, collapse, scroll, press
  App/Window:   launch, close-app, list-windows, list-apps, focus-window
  Clipboard:    clipboard-get, clipboard-set
  Wait:         wait
  System:       status, permissions, version
  Batch:        batch"
)]
pub struct Cli {
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Snapshot(SnapshotArgs),
    Find(FindArgs),
    Screenshot(ScreenshotArgs),
    Get(GetArgs),
    Is(IsArgs),
    Click(RefArgs),
    DoubleClick(RefArgs),
    RightClick(RefArgs),
    Type(TypeArgs),
    SetValue(SetValueArgs),
    Focus(RefArgs),
    Select(SelectArgs),
    Toggle(RefArgs),
    Expand(RefArgs),
    Collapse(RefArgs),
    Scroll(ScrollArgs),
    Press(PressArgs),
    Launch(LaunchArgs),
    CloseApp(CloseAppArgs),
    ListWindows(ListWindowsArgs),
    ListApps,
    FocusWindow(FocusWindowArgs),
    ClipboardGet,
    ClipboardSet(ClipboardSetArgs),
    Wait(WaitArgs),
    Status,
    Permissions(PermissionsArgs),
    Version(VersionArgs),
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
            Self::RightClick(_) => "right-click",
            Self::Type(_) => "type",
            Self::SetValue(_) => "set-value",
            Self::Focus(_) => "focus",
            Self::Select(_) => "select",
            Self::Toggle(_) => "toggle",
            Self::Expand(_) => "expand",
            Self::Collapse(_) => "collapse",
            Self::Scroll(_) => "scroll",
            Self::Press(_) => "press",
            Self::Launch(_) => "launch",
            Self::CloseApp(_) => "close-app",
            Self::ListWindows(_) => "list-windows",
            Self::ListApps => "list-apps",
            Self::FocusWindow(_) => "focus-window",
            Self::ClipboardGet => "clipboard-get",
            Self::ClipboardSet(_) => "clipboard-set",
            Self::Wait(_) => "wait",
            Self::Status => "status",
            Self::Permissions(_) => "permissions",
            Self::Version(_) => "version",
            Self::Batch(_) => "batch",
        }
    }
}

#[derive(Parser, Debug)]
pub struct SnapshotArgs {
    #[arg(long)]
    pub app: Option<String>,
    #[arg(long, name = "window-id")]
    pub window_id: Option<String>,
    #[arg(long, default_value = "10")]
    pub max_depth: u8,
    #[arg(long)]
    pub include_bounds: bool,
    #[arg(long, short = 'i')]
    pub interactive_only: bool,
    #[arg(long)]
    pub compact: bool,
}

#[derive(Parser, Debug)]
pub struct FindArgs {
    #[arg(long)]
    pub app: Option<String>,
    #[arg(long)]
    pub role: Option<String>,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub value: Option<String>,
}

#[derive(Parser, Debug)]
pub struct ScreenshotArgs {
    #[arg(long)]
    pub app: Option<String>,
    #[arg(long, name = "window-id")]
    pub window_id: Option<String>,
    #[arg(value_name = "PATH")]
    pub output_path: Option<std::path::PathBuf>,
}

#[derive(Parser, Debug)]
pub struct GetArgs {
    #[arg(value_name = "REF")]
    pub ref_id: String,
    #[arg(long, default_value = "text")]
    pub property: String,
}

#[derive(Parser, Debug)]
pub struct IsArgs {
    #[arg(value_name = "REF")]
    pub ref_id: String,
    #[arg(long, default_value = "visible")]
    pub property: String,
}

#[derive(Parser, Debug)]
pub struct RefArgs {
    #[arg(value_name = "REF")]
    pub ref_id: String,
}

#[derive(Parser, Debug)]
pub struct TypeArgs {
    #[arg(value_name = "REF")]
    pub ref_id: String,
    #[arg(value_name = "TEXT")]
    pub text: String,
}

#[derive(Parser, Debug)]
pub struct SetValueArgs {
    #[arg(value_name = "REF")]
    pub ref_id: String,
    #[arg(value_name = "VALUE")]
    pub value: String,
}

#[derive(Parser, Debug)]
pub struct SelectArgs {
    #[arg(value_name = "REF")]
    pub ref_id: String,
    #[arg(value_name = "VALUE")]
    pub value: String,
}

#[derive(Parser, Debug)]
pub struct ScrollArgs {
    #[arg(value_name = "REF")]
    pub ref_id: String,
    #[arg(long, default_value = "down")]
    pub direction: String,
    #[arg(long, default_value = "3")]
    pub amount: u32,
}

#[derive(Parser, Debug)]
pub struct PressArgs {
    #[arg(value_name = "COMBO")]
    pub combo: String,
}

#[derive(Parser, Debug)]
pub struct LaunchArgs {
    #[arg(value_name = "APP")]
    pub app: String,
    #[arg(long)]
    pub wait: bool,
}

#[derive(Parser, Debug)]
pub struct CloseAppArgs {
    #[arg(value_name = "APP")]
    pub app: String,
    #[arg(long)]
    pub force: bool,
}

#[derive(Parser, Debug)]
pub struct ListWindowsArgs {
    #[arg(long)]
    pub app: Option<String>,
}

#[derive(Parser, Debug)]
pub struct FocusWindowArgs {
    #[arg(long, name = "window-id")]
    pub window_id: Option<String>,
    #[arg(long)]
    pub app: Option<String>,
    #[arg(long)]
    pub title: Option<String>,
}

#[derive(Parser, Debug)]
pub struct ClipboardSetArgs {
    #[arg(value_name = "TEXT")]
    pub text: String,
}

#[derive(Parser, Debug)]
pub struct WaitArgs {
    #[arg(value_name = "MS")]
    pub ms: Option<u64>,
    #[arg(long)]
    pub element: Option<String>,
    #[arg(long)]
    pub window: Option<String>,
    #[arg(long, default_value = "30000")]
    pub timeout: u64,
}

#[derive(Parser, Debug)]
pub struct PermissionsArgs {
    #[arg(long)]
    pub request: bool,
}

#[derive(Parser, Debug)]
pub struct VersionArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct BatchArgs {
    #[arg(value_name = "JSON")]
    pub commands_json: String,
    #[arg(long)]
    pub stop_on_error: bool,
}
