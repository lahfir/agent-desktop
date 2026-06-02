use clap::Parser;
use serde::Deserialize;

fn default_launch_timeout() -> u64 {
    30000
}

fn default_wait_timeout() -> u64 {
    30000
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LaunchArgs {
    #[arg(value_name = "APP", help = "Application name or bundle ID")]
    pub app: String,
    #[arg(
        long,
        default_value = "30000",
        help = "Max time in ms to wait for the window to appear"
    )]
    #[serde(default = "default_launch_timeout")]
    pub timeout: u64,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CloseAppArgs {
    #[arg(value_name = "APP", help = "Application name")]
    pub app: String,
    #[arg(long, help = "Force-kill the process instead of quitting gracefully")]
    #[serde(default)]
    pub force: bool,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListWindowsArgs {
    #[arg(long, help = "Filter to application by exact case-insensitive name")]
    pub app: Option<String>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListAppsArgs {
    #[arg(long, help = "Filter to application by name")]
    pub app: Option<String>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FocusWindowArgs {
    #[arg(long, name = "window-id", help = "Window ID from list-windows")]
    pub window_id: Option<String>,
    #[arg(long, help = "Application name")]
    pub app: Option<String>,
    #[arg(long, help = "Window title (partial match accepted)")]
    pub title: Option<String>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResizeWindowCliArgs {
    #[arg(long, help = "Application name")]
    pub app: Option<String>,
    #[arg(long, help = "New window width in pixels")]
    pub width: f64,
    #[arg(long, help = "New window height in pixels")]
    pub height: f64,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MoveWindowCliArgs {
    #[arg(long, help = "Application name")]
    pub app: Option<String>,
    #[arg(long, help = "New window X position")]
    pub x: f64,
    #[arg(long, help = "New window Y position")]
    pub y: f64,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AppRefArgs {
    #[arg(long, help = "Application name")]
    pub app: Option<String>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClipboardSetArgs {
    #[arg(value_name = "TEXT", help = "Text to write to the clipboard")]
    pub text: String,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WaitArgs {
    #[arg(value_name = "MS", help = "Milliseconds to pause")]
    pub ms: Option<u64>,
    #[arg(long, help = "Block until this element ref appears in the tree")]
    pub element: Option<String>,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot for --element waits; omit to use latest"
    )]
    pub snapshot: Option<String>,
    #[arg(
        long,
        value_name = "PREDICATE",
        help = "Element wait predicate: exists, enabled, visible, actionable, or value"
    )]
    pub predicate: Option<String>,
    #[arg(
        long,
        value_name = "VALUE",
        help = "Expected value for --predicate value"
    )]
    pub value: Option<String>,
    #[arg(
        long,
        value_name = "COUNT",
        help = "Expected match count for --text waits"
    )]
    pub count: Option<usize>,
    #[arg(long, help = "Block until a window with this title appears")]
    pub window: Option<String>,
    #[arg(
        long,
        help = "Block until text appears in the app's accessibility tree; with --notification, filter notification text"
    )]
    pub text: Option<String>,
    #[arg(
        long,
        default_value = "30000",
        help = "Timeout in milliseconds for element/window/text waits"
    )]
    #[serde(default = "default_wait_timeout")]
    pub timeout: u64,
    #[arg(long, help = "Block until a menu surface is open")]
    #[serde(default)]
    pub menu: bool,
    #[arg(long, help = "Block until the menu surface is dismissed")]
    #[serde(default)]
    pub menu_closed: bool,
    #[arg(long, help = "Block until a new notification arrives")]
    #[serde(default)]
    pub notification: bool,
    #[arg(long, help = "Scope element, window, or text wait to this application")]
    pub app: Option<String>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PermissionsArgs {
    #[arg(long, help = "Trigger the system accessibility permission dialog")]
    #[serde(default)]
    pub request: bool,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VersionArgs {
    #[arg(long, help = "Output version as JSON object")]
    #[serde(default)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub(crate) struct BatchArgs {
    #[arg(value_name = "JSON", help = "JSON array of {command, args} objects")]
    pub commands_json: String,
    #[arg(long, help = "Halt the batch on the first failed command")]
    pub stop_on_error: bool,
}
