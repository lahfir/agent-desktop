use clap::Parser;
use serde::Deserialize;

fn default_wheel_delta() -> f64 {
    -3.0
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MouseWheelArgs {
    #[arg(long, help = "Absolute X coordinate for wheel event")]
    pub x: f64,
    #[arg(long, help = "Absolute Y coordinate for wheel event")]
    pub y: f64,
    #[arg(
        long,
        default_value = "-3",
        help = "Vertical wheel lines; positive scrolls up, negative scrolls down"
    )]
    #[serde(default = "default_wheel_delta")]
    pub dy: f64,
    #[arg(
        long,
        default_value = "0",
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
}
