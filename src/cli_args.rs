use clap::{Parser, ValueEnum};

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum Surface {
    #[default]
    Window,
    Focused,
    Menu,
    Sheet,
    Popover,
    Alert,
}

#[derive(Parser, Debug)]
pub struct SnapshotArgs {
    #[arg(long, help = "Filter to application by name")]
    pub app: Option<String>,
    #[arg(long, name = "window-id", help = "Filter to window ID (from list-windows)")]
    pub window_id: Option<String>,
    #[arg(long, default_value = "10", help = "Maximum tree depth")]
    pub max_depth: u8,
    #[arg(long, help = "Include element bounds (x, y, width, height)")]
    pub include_bounds: bool,
    #[arg(long, short = 'i', help = "Include interactive elements only")]
    pub interactive_only: bool,
    #[arg(long, help = "Omit empty structural nodes from output")]
    pub compact: bool,
    #[arg(long, value_enum, default_value_t = Surface::Window, help = "Surface to snapshot")]
    pub surface: Surface,
}

#[derive(Parser, Debug)]
pub struct FindArgs {
    #[arg(long, help = "Filter to application by name")]
    pub app: Option<String>,
    #[arg(long, help = "Match by accessibility role (button, textfield, checkbox ...)")]
    pub role: Option<String>,
    #[arg(long, help = "Match by accessible name or label")]
    pub name: Option<String>,
    #[arg(long, help = "Match by current value")]
    pub value: Option<String>,
    #[arg(long, help = "Match by text in name, value, title, or description")]
    pub text: Option<String>,
    #[arg(long, help = "Return match count only")]
    pub count: bool,
    #[arg(long, help = "Return first match only")]
    pub first: bool,
    #[arg(long, help = "Return last match only")]
    pub last: bool,
    #[arg(long, help = "Return Nth match (0-indexed)")]
    pub nth: Option<usize>,
}

#[derive(Parser, Debug)]
pub struct ScreenshotArgs {
    #[arg(long, help = "Filter to application by name")]
    pub app: Option<String>,
    #[arg(long, name = "window-id", help = "Filter to window ID (from list-windows)")]
    pub window_id: Option<String>,
    #[arg(value_name = "PATH", help = "Save to file instead of returning base64")]
    pub output_path: Option<std::path::PathBuf>,
}

#[derive(Parser, Debug)]
pub struct GetArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(long, default_value = "text", help = "Property: text, value, title, bounds, role, states")]
    pub property: String,
}

#[derive(Parser, Debug)]
pub struct IsArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(long, default_value = "visible", help = "State: visible, enabled, checked, focused, expanded")]
    pub property: String,
}

#[derive(Parser, Debug)]
pub struct RefArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
}

#[derive(Parser, Debug)]
pub struct TypeArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(value_name = "TEXT", allow_hyphen_values = true, help = "Text to type")]
    pub text: String,
}

#[derive(Parser, Debug)]
pub struct SetValueArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(value_name = "VALUE", allow_hyphen_values = true, help = "Value to set")]
    pub value: String,
}

#[derive(Parser, Debug)]
pub struct SelectArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(value_name = "VALUE", help = "Option to select")]
    pub value: String,
}

#[derive(Parser, Debug)]
pub struct ScrollArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(long, default_value = "down", help = "Direction: up, down, left, right")]
    pub direction: String,
    #[arg(long, default_value = "3", help = "Number of scroll units")]
    pub amount: u32,
}

#[derive(Parser, Debug)]
pub struct PressArgs {
    #[arg(value_name = "COMBO", help = "Key combo: return, escape, cmd+c, shift+tab ...")]
    pub combo: String,
    #[arg(long, help = "Target application name (focuses app before pressing)")]
    pub app: Option<String>,
}

#[derive(Parser, Debug)]
pub struct KeyComboArgs {
    #[arg(value_name = "COMBO", help = "Key or modifier to hold/release: shift, cmd, ctrl ...")]
    pub combo: String,
}

#[derive(Parser, Debug)]
pub struct HoverArgs {
    #[arg(value_name = "REF", help = "Element ref to hover over")]
    pub ref_id: Option<String>,
    #[arg(long, help = "Absolute coordinates as x,y")]
    pub xy: Option<String>,
    #[arg(long, help = "Hold hover position for N milliseconds")]
    pub duration: Option<u64>,
}

#[derive(Parser, Debug)]
pub struct DragCliArgs {
    #[arg(long, help = "Source element ref")]
    pub from: Option<String>,
    #[arg(long, name = "from-xy", help = "Source coordinates as x,y")]
    pub from_xy: Option<String>,
    #[arg(long, help = "Destination element ref")]
    pub to: Option<String>,
    #[arg(long, name = "to-xy", help = "Destination coordinates as x,y")]
    pub to_xy: Option<String>,
    #[arg(long, help = "Drag duration in milliseconds")]
    pub duration: Option<u64>,
}

#[derive(Parser, Debug)]
pub struct MouseMoveArgs {
    #[arg(long, help = "Absolute coordinates as x,y")]
    pub xy: String,
}

#[derive(Parser, Debug)]
pub struct MouseClickArgs {
    #[arg(long, help = "Absolute coordinates as x,y")]
    pub xy: String,
    #[arg(long, default_value = "left", help = "Mouse button: left, right, middle")]
    pub button: String,
    #[arg(long, default_value = "1", help = "Number of clicks")]
    pub count: u32,
}

#[derive(Parser, Debug)]
pub struct MousePointArgs {
    #[arg(long, help = "Absolute coordinates as x,y")]
    pub xy: String,
    #[arg(long, default_value = "left", help = "Mouse button: left, right, middle")]
    pub button: String,
}

#[derive(Parser, Debug)]
pub struct LaunchArgs {
    #[arg(value_name = "APP", help = "Application name or bundle ID")]
    pub app: String,
    #[arg(long, default_value = "30000", help = "Max time in ms to wait for the window to appear")]
    pub timeout: u64,
}

#[derive(Parser, Debug)]
pub struct CloseAppArgs {
    #[arg(value_name = "APP", help = "Application name")]
    pub app: String,
    #[arg(long, help = "Force-kill the process instead of quitting gracefully")]
    pub force: bool,
}

#[derive(Parser, Debug)]
pub struct ListWindowsArgs {
    #[arg(long, help = "Filter to application by name")]
    pub app: Option<String>,
}

#[derive(Parser, Debug)]
pub struct FocusWindowArgs {
    #[arg(long, name = "window-id", help = "Window ID from list-windows")]
    pub window_id: Option<String>,
    #[arg(long, help = "Application name")]
    pub app: Option<String>,
    #[arg(long, help = "Window title (partial match accepted)")]
    pub title: Option<String>,
}

#[derive(Parser, Debug)]
pub struct ResizeWindowCliArgs {
    #[arg(long, help = "Application name")]
    pub app: Option<String>,
    #[arg(long, help = "New window width in pixels")]
    pub width: f64,
    #[arg(long, help = "New window height in pixels")]
    pub height: f64,
}

#[derive(Parser, Debug)]
pub struct MoveWindowCliArgs {
    #[arg(long, help = "Application name")]
    pub app: Option<String>,
    #[arg(long, help = "New window X position")]
    pub x: f64,
    #[arg(long, help = "New window Y position")]
    pub y: f64,
}

#[derive(Parser, Debug)]
pub struct AppRefArgs {
    #[arg(long, help = "Application name")]
    pub app: Option<String>,
}

#[derive(Parser, Debug)]
pub struct ClipboardSetArgs {
    #[arg(value_name = "TEXT", help = "Text to write to the clipboard")]
    pub text: String,
}

#[derive(Parser, Debug)]
pub struct WaitArgs {
    #[arg(value_name = "MS", help = "Milliseconds to pause")]
    pub ms: Option<u64>,
    #[arg(long, help = "Block until this element ref appears in the tree")]
    pub element: Option<String>,
    #[arg(long, help = "Block until a window with this title appears")]
    pub window: Option<String>,
    #[arg(long, help = "Block until text appears in the app's accessibility tree")]
    pub text: Option<String>,
    #[arg(long, default_value = "30000", help = "Timeout in milliseconds for element/window/text waits")]
    pub timeout: u64,
    #[arg(long, help = "Block until a context menu is open")]
    pub menu: bool,
    #[arg(long, help = "Block until the context menu is dismissed")]
    pub menu_closed: bool,
    #[arg(long, help = "Scope element, window, or text wait to this application")]
    pub app: Option<String>,
}

#[derive(Parser, Debug)]
pub struct ListSurfacesArgs {
    #[arg(long, help = "Filter to application by name")]
    pub app: Option<String>,
}

#[derive(Parser, Debug)]
pub struct PermissionsArgs {
    #[arg(long, help = "Trigger the system accessibility permission dialog")]
    pub request: bool,
}

#[derive(Parser, Debug)]
pub struct VersionArgs {
    #[arg(long, help = "Output version as JSON object")]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct BatchArgs {
    #[arg(value_name = "JSON", help = "JSON array of {command, args} objects")]
    pub commands_json: String,
    #[arg(long, help = "Halt the batch on the first failed command")]
    pub stop_on_error: bool,
}
