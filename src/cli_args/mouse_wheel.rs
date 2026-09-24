use clap::Parser;
use serde::Deserialize;

fn default_wheel_delta() -> f64 {
    -3.0
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MouseWheelArgs {
    #[arg(
        long,
        allow_negative_numbers = true,
        help = "Absolute X coordinate for wheel event"
    )]
    pub x: f64,
    #[arg(
        long,
        allow_negative_numbers = true,
        help = "Absolute Y coordinate for wheel event"
    )]
    pub y: f64,
    #[arg(
        long,
        default_value = "-3",
        allow_negative_numbers = true,
        help = "Vertical wheel lines; positive scrolls up, negative scrolls down"
    )]
    #[serde(default = "default_wheel_delta")]
    pub dy: f64,
    #[arg(
        long,
        default_value = "0",
        allow_negative_numbers = true,
        help = "Horizontal wheel lines; positive scrolls left, negative scrolls right"
    )]
    #[serde(default)]
    pub dx: f64,
    #[arg(
        long,
        value_name = "MODIFIER",
        help = "Held modifiers: shift, meta, ctrl, alt (repeatable; cmd is accepted)"
    )]
    #[serde(default)]
    pub modifiers: Vec<String>,
    #[arg(
        long,
        help = "Opt-in, best-effort synthetic wheel events posted to the --window-id window's process (macOS, private SkyLight SPI): the real cursor stays put; focus preservation is best effort, see focus_change in the result; conflicts with --headed"
    )]
    #[serde(default)]
    pub background: bool,
    #[arg(
        long = "window-id",
        value_name = "WINDOW_ID",
        help = "Exact target window for --background (from list-windows, e.g. w-9555); the point must lie inside it"
    )]
    #[serde(default)]
    pub window_id: Option<String>,
}
