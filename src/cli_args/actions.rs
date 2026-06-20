use clap::Parser;
use serde::Deserialize;

fn default_scroll_amount() -> u32 {
    3
}

fn default_mouse_button() -> String {
    "left".to_string()
}

fn default_mouse_click_count() -> u32 {
    1
}

fn default_scroll_direction() -> String {
    "down".to_string()
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypeArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use active session latest"
    )]
    pub snapshot: Option<String>,
    #[arg(value_name = "TEXT", allow_hyphen_values = true, help = "Text to type")]
    pub text: String,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetValueArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use active session latest"
    )]
    pub snapshot: Option<String>,
    #[arg(
        value_name = "VALUE",
        allow_hyphen_values = true,
        help = "Value to set"
    )]
    pub value: String,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use active session latest"
    )]
    pub snapshot: Option<String>,
    #[arg(value_name = "VALUE", help = "Option to select")]
    pub value: String,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScrollArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use active session latest"
    )]
    pub snapshot: Option<String>,
    #[arg(
        long,
        default_value = "down",
        help = "Direction: up, down, left, right"
    )]
    #[serde(default = "default_scroll_direction")]
    pub direction: String,
    #[arg(long, default_value = "3", help = "Number of scroll units")]
    #[serde(default = "default_scroll_amount")]
    pub amount: u32,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PressArgs {
    #[arg(
        value_name = "COMBO",
        help = "Key combo: return, escape, cmd+c, shift+tab ..."
    )]
    pub combo: String,
    #[arg(long, help = "Target application name (focuses app before pressing)")]
    pub app: Option<String>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeyComboArgs {
    #[arg(
        value_name = "COMBO",
        help = "Key or modifier to hold/release: shift, cmd, ctrl ..."
    )]
    pub combo: String,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HoverArgs {
    #[arg(
        value_name = "REF",
        help = "Element ref to hover over; requires --headed"
    )]
    pub ref_id: Option<String>,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use active session latest"
    )]
    pub snapshot: Option<String>,
    #[arg(long, help = "Absolute coordinates as x,y; requires --headed")]
    pub xy: Option<String>,
    #[arg(long, help = "Hold hover position for N milliseconds")]
    pub duration: Option<u64>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DragCliArgs {
    #[arg(long, help = "Source element ref; requires --headed")]
    pub from: Option<String>,
    #[arg(
        long,
        name = "from-xy",
        help = "Source coordinates as x,y; requires --headed"
    )]
    pub from_xy: Option<String>,
    #[arg(long, help = "Destination element ref; requires --headed")]
    pub to: Option<String>,
    #[arg(
        long,
        name = "to-xy",
        help = "Destination coordinates as x,y; requires --headed"
    )]
    pub to_xy: Option<String>,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use active session latest"
    )]
    pub snapshot: Option<String>,
    #[arg(long, help = "Drag duration in milliseconds")]
    pub duration: Option<u64>,
    #[arg(
        long = "drop-delay",
        value_name = "MS",
        help = "Hold over the destination this many ms before releasing, so the drop target activates (macOS); default 500"
    )]
    pub drop_delay: Option<u64>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MouseMoveArgs {
    #[arg(long, help = "Absolute coordinates as x,y; requires --headed")]
    pub xy: String,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MouseClickArgs {
    #[arg(long, help = "Absolute coordinates as x,y; requires --headed")]
    pub xy: String,
    #[arg(
        long,
        default_value = "left",
        help = "Mouse button: left, right, middle"
    )]
    #[serde(default = "default_mouse_button")]
    pub button: String,
    #[arg(long, default_value = "1", help = "Number of clicks")]
    #[serde(default = "default_mouse_click_count")]
    pub count: u32,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MousePointArgs {
    #[arg(long, help = "Absolute coordinates as x,y; requires --headed")]
    pub xy: String,
    #[arg(
        long,
        default_value = "left",
        help = "Mouse button: left, right, middle"
    )]
    #[serde(default = "default_mouse_button")]
    pub button: String,
}
