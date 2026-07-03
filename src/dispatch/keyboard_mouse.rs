use agent_desktop_core::{
    commands::{
        drag, helpers, hover, key_down, key_up, mouse_click, mouse_down, mouse_move, mouse_up,
        mouse_wheel, press,
    },
    context::CommandContext,
    error::{AppError, ErrorCode},
};
use serde_json::Value;

use crate::cli::Commands;
use crate::dispatch::parse::{parse_modifiers, parse_mouse_button, parse_xy, parse_xy_opt};

pub(super) fn dispatch(
    cmd: Commands,
    adapter: &dyn agent_desktop_core::adapter::PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    match cmd {
        Commands::Press(a) => press::execute(
            press::PressArgs {
                combo: a.combo,
                app: a.app,
                force: a.force,
            },
            adapter,
        ),

        Commands::KeyDown(a) => key_down::execute(
            key_down::KeyDownArgs {
                combo: a.combo,
                force: a.force,
            },
            adapter,
        ),

        Commands::KeyUp(a) => key_up::execute(
            key_up::KeyUpArgs {
                combo: a.combo,
                force: a.force,
            },
            adapter,
        ),

        Commands::Hover(a) => hover::execute(
            hover::HoverArgs {
                ref_id: a.ref_id,
                snapshot_id: a.snapshot,
                xy: parse_xy_opt(a.xy.as_deref())?,
                duration_ms: a.duration,
                timeout_ms: helpers::normalize_action_timeout_ms(a.timeout_ms),
            },
            adapter,
            context,
        ),

        Commands::Drag(a) => drag::execute(
            drag::DragArgs {
                from_ref: a.from,
                from_xy: parse_xy_opt(a.from_xy.as_deref())?,
                to_ref: a.to,
                to_xy: parse_xy_opt(a.to_xy.as_deref())?,
                snapshot_id: a.snapshot,
                duration_ms: a.duration,
                drop_delay_ms: a.drop_delay,
                timeout_ms: helpers::normalize_action_timeout_ms(a.timeout_ms),
            },
            adapter,
            context,
        ),

        Commands::MouseMove(a) => {
            let (x, y) = parse_xy(&a.xy)?;
            mouse_move::execute(mouse_move::MouseMoveArgs { x, y }, adapter, context)
        }

        Commands::MouseClick(a) => {
            let (x, y) = parse_xy(&a.xy)?;
            mouse_click::execute(
                mouse_click::MouseClickArgs {
                    x,
                    y,
                    button: parse_mouse_button(&a.button)?,
                    count: a.count,
                    modifiers: parse_modifiers(&a.modifiers)?,
                },
                adapter,
                context,
            )
        }

        Commands::MouseDown(a) => {
            let (x, y) = parse_xy(&a.xy)?;
            mouse_down::execute(
                mouse_down::MouseDownArgs {
                    x,
                    y,
                    button: parse_mouse_button(&a.button)?,
                    modifiers: parse_modifiers(&a.modifiers)?,
                },
                adapter,
                context,
            )
        }

        Commands::MouseUp(a) => {
            let (x, y) = parse_xy(&a.xy)?;
            mouse_up::execute(
                mouse_up::MouseUpArgs {
                    x,
                    y,
                    button: parse_mouse_button(&a.button)?,
                    modifiers: parse_modifiers(&a.modifiers)?,
                },
                adapter,
                context,
            )
        }

        Commands::MouseWheel(a) => mouse_wheel::execute(
            mouse_wheel::MouseWheelArgs {
                x: a.x,
                y: a.y,
                dy: a.dy,
                dx: a.dx,
                modifiers: parse_modifiers(&a.modifiers)?,
            },
            adapter,
        ),

        _ => Err(AppError::Adapter(
            agent_desktop_core::error::AdapterError::new(
                ErrorCode::InvalidArgs,
                "keyboard_mouse::dispatch received a non-keyboard/mouse command",
            ),
        )),
    }
}
