use clap::{Parser, ValueEnum};
use serde::Deserialize;

fn default_max_depth() -> u8 {
    10
}

fn default_get_property() -> String {
    "text".to_string()
}

fn default_is_property() -> String {
    "visible".to_string()
}

#[derive(ValueEnum, Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Surface {
    #[default]
    Window,
    Focused,
    Menu,
    Menubar,
    Sheet,
    Popover,
    Alert,
}

impl Surface {
    pub(crate) fn to_core(&self) -> agent_desktop_core::adapter::SnapshotSurface {
        use agent_desktop_core::adapter::SnapshotSurface;
        match self {
            Self::Window => SnapshotSurface::Window,
            Self::Focused => SnapshotSurface::Focused,
            Self::Menu => SnapshotSurface::Menu,
            Self::Menubar => SnapshotSurface::Menubar,
            Self::Sheet => SnapshotSurface::Sheet,
            Self::Popover => SnapshotSurface::Popover,
            Self::Alert => SnapshotSurface::Alert,
        }
    }
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SnapshotArgs {
    #[arg(long, help = "Filter to application by name")]
    pub app: Option<String>,
    #[arg(
        long,
        name = "window-id",
        help = "Filter to window ID (from list-windows)"
    )]
    pub window_id: Option<String>,
    #[arg(long, default_value = "10", help = "Maximum tree depth")]
    #[serde(default = "default_max_depth")]
    pub max_depth: u8,
    #[arg(long, help = "Include element bounds (x, y, width, height)")]
    #[serde(default)]
    pub include_bounds: bool,
    #[arg(long, short = 'i', help = "Include interactive elements only")]
    #[serde(default)]
    pub interactive_only: bool,
    #[arg(
        long,
        help = "Collapse single-child unnamed nodes to reduce tree depth"
    )]
    #[serde(default)]
    pub compact: bool,
    #[arg(
        long,
        value_enum,
        default_value_t = Surface::Window,
        help = "Surface to snapshot"
    )]
    #[serde(default)]
    pub surface: Surface,
    #[arg(
        long,
        help = "Shallow overview with children_count on truncated containers"
    )]
    #[serde(default)]
    pub skeleton: bool,
    #[arg(long, help = "Start traversal from this ref instead of window root")]
    pub root: Option<String>,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID to use when resolving --root"
    )]
    pub snapshot: Option<String>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FindArgs {
    #[arg(long, help = "Filter to application by name")]
    pub app: Option<String>,
    #[arg(
        long,
        help = "Match by accessibility role (button, textfield, checkbox ...)"
    )]
    pub role: Option<String>,
    #[arg(long, help = "Match by accessible name or label")]
    pub name: Option<String>,
    #[arg(long, help = "Match by current value")]
    pub value: Option<String>,
    #[arg(long, help = "Match by text in name, value, title, or description")]
    pub text: Option<String>,
    #[arg(
        long,
        help = "Return match count only",
        conflicts_with_all = ["first", "last", "nth", "limit"]
    )]
    #[serde(default)]
    pub count: bool,
    #[arg(
        long,
        help = "Return first match only",
        conflicts_with_all = ["count", "last", "nth", "limit"]
    )]
    #[serde(default)]
    pub first: bool,
    #[arg(
        long,
        help = "Return last match only",
        conflicts_with_all = ["count", "first", "nth", "limit"]
    )]
    #[serde(default)]
    pub last: bool,
    #[arg(
        long,
        help = "Return Nth match (0-indexed)",
        conflicts_with_all = ["count", "first", "last", "limit"]
    )]
    pub nth: Option<usize>,
    #[arg(
        long,
        help = "Return at most N matches; defaults to 50 when omitted, use 0 for all",
        conflicts_with_all = ["count", "first", "last", "nth"]
    )]
    pub limit: Option<usize>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScreenshotArgs {
    #[arg(long, help = "Filter to application by name")]
    pub app: Option<String>,
    #[arg(
        long,
        name = "window-id",
        help = "Filter to window ID (from list-windows)"
    )]
    pub window_id: Option<String>,
    #[arg(value_name = "PATH", help = "Save to file instead of returning base64")]
    pub output_path: Option<std::path::PathBuf>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use latest"
    )]
    pub snapshot: Option<String>,
    #[arg(
        long,
        default_value = "text",
        help = "Property: text, value, title, bounds, role, states"
    )]
    #[serde(default = "default_get_property")]
    pub property: String,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IsArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use latest"
    )]
    pub snapshot: Option<String>,
    #[arg(
        long,
        default_value = "visible",
        help = "State: visible, enabled, checked, focused, expanded"
    )]
    #[serde(default = "default_is_property")]
    pub property: String,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RefArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(
        long = "snapshot",
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use latest"
    )]
    #[serde(rename = "snapshot", alias = "snapshot_id")]
    pub snapshot_id: Option<String>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListSurfacesArgs {
    #[arg(long, help = "Filter to application by name")]
    pub app: Option<String>,
}
