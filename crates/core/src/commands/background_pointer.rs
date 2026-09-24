use serde_json::{Value, json};

use crate::{
    AppError, BackgroundDeliveryReport, ErrorCode, Modifier, MouseButton, MouseEvent,
    MouseEventKind, Point, WindowInfo,
    adapter::PlatformAdapter,
    commands::{
        background_delivery::{self, not_delivered, ref_window},
        helpers,
        pointer_action::point_deadline,
        window_target,
    },
    context::CommandContext,
};

/// Where a background pointer event lands.
pub enum BackgroundPointerTarget {
    /// Center of a snapshot ref; the owning process and exact window come
    /// from the ref itself.
    Ref {
        ref_id: String,
        snapshot_id: Option<String>,
    },
    /// Global screen coordinates inside an explicitly named window.
    Point { x: f64, y: f64, window_id: String },
}

pub enum BackgroundPointerAction {
    Hover,
    Move,
    Click {
        button: MouseButton,
        count: u32,
        modifiers: Vec<Modifier>,
    },
}

pub struct BackgroundPointerArgs {
    pub action: BackgroundPointerAction,
    pub target: BackgroundPointerTarget,
    pub timeout_ms: Option<u64>,
}

struct ResolvedTarget {
    window: WindowInfo,
    point: Point,
}

/// Opt-in background pointer delivery shared by `hover`, `mouse-move`, and
/// `mouse-click` when they run with `--background`.
///
/// The event is posted straight to the process that owns one exact window, so
/// the user's real cursor never moves and the window is never raised. Keeping
/// the user's frontmost app and keyboard focus is best effort: the target
/// process may ignore the event or react to it by activating itself, so
/// success is reported as `delivered_unverified` together with the frontmost
/// application observed before and after. Because the window server's hit
/// test is bypassed, the target window may be offscreen or covered by other
/// windows; the only geometric requirement is that the point lies inside the
/// target window's bounds.
///
/// `--wait-for` observes the target window, never the user's frontmost app,
/// for both ref and coordinate targets.
///
/// The cursor overlay is intentionally skipped: it would draw a cursor where
/// the real cursor is not, over a window that may not even be visible.
pub fn execute(
    args: BackgroundPointerArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    background_delivery::reject_headed(context)?;
    if let BackgroundPointerAction::Click { count, .. } = &args.action {
        crate::validate_mouse_click_count(*count)?;
    }
    helpers::validate_post_action_wait(context)?;
    let deadline = point_deadline(args.timeout_ms)?;

    let lease = adapter.acquire_interaction_lease(deadline)?;
    let target = resolve_target(args.target, deadline, adapter, context)?;
    let window = window_target::revalidate_window_for_mutation(adapter, &target.window, &lease)?;
    ensure_point_in_window(&target.point, &window)?;

    let event = mouse_event(&args.action, target.point.clone());
    let report = adapter.background_mouse_event(&window, event, &lease)?;
    drop(lease);

    let response = response(&args.action, &target.point, &window, report);
    helpers::apply_scoped_post_action_wait(
        response,
        Some(window.app.clone()),
        Some(window.id.clone()),
        adapter,
        context,
    )
}

fn resolve_target(
    target: BackgroundPointerTarget,
    deadline: crate::Deadline,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<ResolvedTarget, AppError> {
    match target {
        BackgroundPointerTarget::Ref {
            ref_id,
            snapshot_id,
        } => {
            let entry = helpers::load_ref_entry(&ref_id, snapshot_id.as_deref(), context)?;
            let window = ref_window(&entry)?;
            let handle = helpers::resolve_handle_within_deadline(adapter, &entry, deadline)?;
            let bounds = adapter
                .get_element_bounds(&handle, deadline)?
                .ok_or_else(|| {
                    not_delivered(
                        ErrorCode::ActionNotSupported,
                        "The ref has no bounds to aim a background pointer event at",
                    )
                })?;
            Ok(ResolvedTarget {
                window,
                point: Point {
                    x: bounds.x + bounds.width / 2.0,
                    y: bounds.y + bounds.height / 2.0,
                },
            })
        }
        BackgroundPointerTarget::Point { x, y, window_id } => {
            let point = Point { x, y };
            point.validate()?;
            let mut window =
                window_target::resolve_window_for_app(None, Some(&window_id), adapter)?;
            window.title.clear();
            Ok(ResolvedTarget { window, point })
        }
    }
}

fn ensure_point_in_window(point: &Point, window: &WindowInfo) -> Result<(), AppError> {
    let Some(bounds) = window.bounds else {
        return Err(not_delivered(
            ErrorCode::WindowNotFound,
            format!("Window {} reported no bounds", window.id),
        )
        .into());
    };
    let inside = point.x >= bounds.x
        && point.x < bounds.x + bounds.width
        && point.y >= bounds.y
        && point.y < bounds.y + bounds.height;
    if inside {
        return Ok(());
    }
    Err(not_delivered(
        ErrorCode::InvalidArgs,
        format!(
            "Point ({}, {}) lies outside window {}",
            point.x, point.y, window.id
        ),
    )
    .with_details(json!({ "point": point, "window_bounds": bounds }))
    .with_suggestion(
        "Use coordinates inside the window bounds reported by list-windows; background delivery never retargets another window.",
    )
    .into())
}

fn mouse_event(action: &BackgroundPointerAction, point: Point) -> MouseEvent {
    match action {
        BackgroundPointerAction::Hover | BackgroundPointerAction::Move => MouseEvent {
            kind: MouseEventKind::Move,
            point,
            button: MouseButton::Left,
            modifiers: Vec::new(),
        },
        BackgroundPointerAction::Click {
            button,
            count,
            modifiers,
        } => MouseEvent {
            kind: MouseEventKind::Click { count: *count },
            point,
            button: button.clone(),
            modifiers: modifiers.clone(),
        },
    }
}

/// Every successful delivery is `delivered_unverified`, which the disposition
/// contract pairs with `retry: unsafe`. For clicks that is essential. For
/// hover a repeat is benign, but the contract cannot express
/// delivered-yet-safe, and the right follow-up is a snapshot that observes
/// the hover effect rather than a blind retry.
fn response(
    action: &BackgroundPointerAction,
    point: &Point,
    window: &WindowInfo,
    report: BackgroundDeliveryReport,
) -> Value {
    let mut response = match action {
        BackgroundPointerAction::Hover => json!({ "hovered": true }),
        BackgroundPointerAction::Move => json!({ "moved": true }),
        BackgroundPointerAction::Click { count, .. } => json!({ "clicked": true, "count": count }),
    };
    response["x"] = json!(point.x);
    response["y"] = json!(point.y);
    background_delivery::attach_report(response, window, &report)
}

#[cfg(test)]
#[path = "background_pointer_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "background_pointer_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "background_pointer_wait_tests.rs"]
mod wait_tests;
