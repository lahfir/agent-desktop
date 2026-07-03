use clap::Parser;
use serde::Deserialize;

fn default_wheel_delta() -> i32 {
    -120
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MouseWheelArgs {
    #[arg(long, help = "Absolute X coordinate for wheel event")]
    pub x: f64,
    #[arg(long, help = "Absolute Y coordinate for wheel event")]
    pub y: f64,
    #[arg(long, default_value = "-120", help = "Vertical scroll delta")]
    #[serde(default = "default_wheel_delta")]
    pub dy: i32,
    #[arg(long, default_value = "0", help = "Horizontal scroll delta")]
    #[serde(default)]
    pub dx: i32,
    #[arg(
        long,
        value_name = "MODIFIER",
        help = "Held modifiers: shift, cmd, ctrl, alt (repeatable)"
    )]
    #[serde(default)]
    pub modifiers: Vec<String>,
}
