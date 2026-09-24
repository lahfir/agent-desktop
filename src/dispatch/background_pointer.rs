use agent_desktop_core::{
    AppError, PlatformAdapter,
    commands::{
        background_pointer::{
            BackgroundPointerAction, BackgroundPointerArgs, BackgroundPointerTarget, execute,
        },
        helpers,
    },
    context::CommandContext,
};
use serde_json::Value;

use crate::cli_args::actions::{HoverArgs, MouseClickArgs, MouseMoveArgs, ScrollArgs};
use crate::cli_args::mouse_wheel::MouseWheelArgs;
use crate::dispatch::parse::{parse_direction, parse_modifiers, parse_mouse_button, parse_xy};

/// `--window-id` only names the target of a background `--xy` event; on the
/// default path it would be silently ignored, so it is rejected instead. This
/// check lives here rather than in clap so batch JSON gets the same rule.
pub(super) fn reject_window_id_without_background(window_id: Option<&str>) -> Result<(), AppError> {
    if window_id.is_none() {
        return Ok(());
    }
    Err(AppError::invalid_input_with_suggestion(
        "--window-id requires --background",
        "Add --background to post to that window without moving the cursor, or drop --window-id.",
    ))
}

pub(super) fn hover(
    args: HoverArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    if args.duration.is_some_and(|duration| duration > 0) {
        return Err(AppError::invalid_input_with_suggestion(
            "Hover duration is unavailable in stateless mode",
            "Run hover without --duration, then use `wait <ms>` for an explicit post-hover pause.",
        ));
    }
    let target = match (args.ref_id, args.xy) {
        (Some(_), Some(_)) => {
            return Err(AppError::invalid_input(
                "Provide either a ref or --xy for a background hover, not both",
            ));
        }
        (Some(ref_id), None) => {
            if args.window_id.is_some() {
                return Err(AppError::invalid_input_with_suggestion(
                    "--window-id cannot be combined with a ref",
                    "A ref already identifies its exact window; drop --window-id.",
                ));
            }
            BackgroundPointerTarget::Ref {
                ref_id,
                snapshot_id: args.snapshot,
            }
        }
        (None, Some(xy)) => point_target(&xy, args.window_id)?,
        (None, None) => {
            return Err(AppError::invalid_input(
                "Provide a ref (@e1) or --xy x,y with --window-id",
            ));
        }
    };
    execute(
        BackgroundPointerArgs {
            action: BackgroundPointerAction::Hover,
            target,
            timeout_ms: helpers::normalize_action_timeout_ms(args.timeout_ms),
        },
        adapter,
        context,
    )
}

pub(super) fn mouse_move(
    args: MouseMoveArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    execute(
        BackgroundPointerArgs {
            action: BackgroundPointerAction::Move,
            target: point_target(&args.xy, args.window_id)?,
            timeout_ms: None,
        },
        adapter,
        context,
    )
}

pub(super) fn mouse_click(
    args: MouseClickArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    execute(
        BackgroundPointerArgs {
            action: BackgroundPointerAction::Click {
                button: parse_mouse_button(&args.button)?,
                count: args.count,
                modifiers: parse_modifiers(&args.modifiers)?,
            },
            target: point_target(&args.xy, args.window_id)?,
            timeout_ms: None,
        },
        adapter,
        context,
    )
}

pub(super) fn mouse_wheel(
    args: MouseWheelArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    execute(
        BackgroundPointerArgs {
            action: BackgroundPointerAction::Wheel {
                dx: args.dx,
                dy: args.dy,
                modifiers: parse_modifiers(&args.modifiers)?,
            },
            target: BackgroundPointerTarget::Point {
                x: args.x,
                y: args.y,
                window_id: required_window_id(args.window_id)?,
            },
            timeout_ms: None,
        },
        adapter,
        context,
    )
}

/// `scroll <ref> --background` is a wheel at the ref's center, not the
/// semantic AX scroll, so it does not require the ref to advertise `Scroll`.
pub(super) fn scroll(
    args: ScrollArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    execute(
        BackgroundPointerArgs {
            action: BackgroundPointerAction::scroll(parse_direction(&args.direction)?, args.amount),
            target: BackgroundPointerTarget::Ref {
                ref_id: args.ref_id,
                snapshot_id: args.snapshot,
            },
            timeout_ms: helpers::normalize_action_timeout_ms(args.timeout_ms),
        },
        adapter,
        context,
    )
}

fn point_target(xy: &str, window_id: Option<String>) -> Result<BackgroundPointerTarget, AppError> {
    let window_id = required_window_id(window_id)?;
    let (x, y) = parse_xy(xy)?;
    Ok(BackgroundPointerTarget::Point { x, y, window_id })
}

/// Coordinates alone cannot name a process, so background coordinate
/// delivery always needs the exact window it is aimed at.
fn required_window_id(window_id: Option<String>) -> Result<String, AppError> {
    window_id.filter(|id| !id.is_empty()).ok_or_else(|| {
        AppError::invalid_input_with_suggestion(
            "--background with coordinates requires --window-id",
            "Run 'list-windows' to find the target window id, then pass --window-id w-<number>.",
        )
    })
}

#[cfg(test)]
#[path = "background_pointer_tests.rs"]
mod tests;
