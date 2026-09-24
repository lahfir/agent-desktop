use serde_json::{Value, json};

use crate::{
    AdapterError, AppError, BackgroundKeyInput, DeliverySemantics, ErrorCode, InteractionLease,
    RefEntry, WindowInfo,
    action::Action,
    adapter::PlatformAdapter,
    commands::{
        background_delivery::{self, not_delivered, ref_window},
        combo::{ensure_combo_allowed, parse_combo_normalized},
        helpers, window_target,
    },
    context::CommandContext,
};

const MAX_TEXT_LEN: usize = 10_000;

/// Fixed allowance for activation, settling, and the focus guard's watch on
/// top of the resolution timeout.
const DELIVERY_BUDGET_MS: u64 = 2_000;

/// Allowance per character of background text. macOS paces text at about
/// 16 ms per character; this is generous so the deadline never compresses
/// that pacing.
const TEXT_BUDGET_PER_CHAR_MS: u64 = 25;

/// Which window receives the keys.
pub enum BackgroundKeyboardTarget {
    /// A snapshot ref. The process and exact window come from the ref, and
    /// the keys are posted only once accessibility focus on the element is
    /// confirmed.
    Ref {
        ref_id: String,
        snapshot_id: Option<String>,
    },
    /// An explicitly named window; the keys reach whatever that window has
    /// focused.
    Window { window_id: String },
}

pub enum BackgroundKeyboardInput {
    Press { combo: String, force: bool },
    Type { text: String },
}

pub struct BackgroundKeyboardArgs {
    pub input: BackgroundKeyboardInput,
    pub target: BackgroundKeyboardTarget,
    pub timeout_ms: Option<u64>,
}

/// Opt-in background keyboard delivery shared by `press` and `type` when they
/// run with `--background`.
///
/// Keys are posted to the process that owns one exact window without
/// activating the app or moving the pointer; keeping the user's frontmost
/// app in front is best effort (see the focus guard in the report). Unlike
/// the default `press --app` path this never matches app menu items.
///
/// A window target sends the keys to whatever that window has focused. A ref
/// target fails closed: the element must accept accessibility focus and the
/// read-back must confirm it, otherwise nothing is posted, because keys that
/// land in another field of the same window are worse than no keys. The app
/// decides what the keys do, so success is `delivered_unverified` and the
/// effect must be observed with a fresh snapshot. `--wait-for` observes the
/// target window, never the user's frontmost app.
pub fn execute(
    args: BackgroundKeyboardArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    background_delivery::reject_headed(context)?;
    helpers::validate_post_action_wait(context)?;
    let (input, mut response) = key_input(args.input, adapter)?;
    let deadline = delivery_deadline(args.timeout_ms, &input)?;

    let lease = adapter.acquire_interaction_lease(deadline)?;
    let (expected, entry) = resolve_target(args.target, adapter, context)?;
    let window = window_target::revalidate_window_for_mutation(adapter, &expected, &lease)?;
    let ax_focus = match &entry {
        Some(entry) => Some(confirm_ref_focus(entry, adapter, context, &lease)?),
        None => None,
    };
    let report = adapter.background_key_input(&window, &input, &lease)?;
    drop(lease);

    response = background_delivery::attach_report(response, &window, &report);
    if let Some(ax_focus) = ax_focus {
        response["background"]["ax_focus"] = ax_focus;
    }
    helpers::apply_scoped_post_action_wait(
        response,
        Some(window.app.clone()),
        Some(window.id.clone()),
        adapter,
        context,
    )
}

/// Validates the input before anything is resolved or posted and returns it
/// with the start of the success response.
fn key_input(
    input: BackgroundKeyboardInput,
    adapter: &dyn PlatformAdapter,
) -> Result<(BackgroundKeyInput, Value), AppError> {
    match input {
        BackgroundKeyboardInput::Press { combo, force } => {
            let parsed = parse_combo_normalized(&combo)?;
            ensure_combo_allowed(&parsed, &combo, force, adapter)?;
            let response = json!({ "pressed": true, "combo": combo });
            Ok((BackgroundKeyInput::Combo(parsed), response))
        }
        BackgroundKeyboardInput::Type { text } => {
            if text.is_empty() {
                return Err(AppError::invalid_input("Text to type must not be empty"));
            }
            if text.len() > MAX_TEXT_LEN {
                return Err(AppError::invalid_input(format!(
                    "Text exceeds maximum length of {MAX_TEXT_LEN} bytes"
                )));
            }
            let response = json!({ "typed": true, "characters": text.chars().count() });
            Ok((BackgroundKeyInput::Text(text), response))
        }
    }
}

fn delivery_deadline(
    timeout_ms: Option<u64>,
    input: &BackgroundKeyInput,
) -> Result<crate::Deadline, AppError> {
    let characters = match input {
        BackgroundKeyInput::Combo(_) => 0,
        BackgroundKeyInput::Text(text) => text.chars().count() as u64,
    };
    let resolution_ms = timeout_ms.unwrap_or(crate::DEFAULT_OPERATION_TIMEOUT_MS);
    let total_ms = resolution_ms
        .saturating_add(DELIVERY_BUDGET_MS)
        .saturating_add(characters.saturating_mul(TEXT_BUDGET_PER_CHAR_MS));
    crate::Deadline::after(total_ms).map_err(AppError::Adapter)
}

fn resolve_target(
    target: BackgroundKeyboardTarget,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<(WindowInfo, Option<RefEntry>), AppError> {
    match target {
        BackgroundKeyboardTarget::Ref {
            ref_id,
            snapshot_id,
        } => {
            let entry = helpers::load_ref_entry(&ref_id, snapshot_id.as_deref(), context)?;
            Ok((ref_window(&entry)?, Some(entry)))
        }
        BackgroundKeyboardTarget::Window { window_id } => {
            let mut window =
                window_target::resolve_window_for_app(None, Some(&window_id), adapter)?;
            window.title.clear();
            Ok((window, None))
        }
    }
}

/// Resolving the ref is a gate: a stale ref means the target changed. So is
/// focus: the element must accept `AXFocused` and the adapter's read-back
/// (the element's own `AXFocused` or the app's `AXFocusedUIElement`) must
/// confirm it. An unconfirmed focus means the keys would reach whatever the
/// window focused before, possibly a different field, so nothing is posted.
fn confirm_ref_focus(
    entry: &RefEntry,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    lease: &InteractionLease,
) -> Result<Value, AppError> {
    let handle = helpers::resolve_handle_within_deadline(adapter, entry, lease.deadline())?;
    match adapter.execute_action(&handle, context.request_base(Action::SetFocus), lease) {
        Ok(result) if result.disposition() == DeliverySemantics::delivered_verified() => {
            Ok(json!({ "status": "verified" }))
        }
        Ok(_) => Err(focus_unconfirmed(
            "focus was requested but the read-back did not show the element focused",
        )
        .into()),
        Err(error) => {
            Err(focus_unconfirmed(format!("{}: {}", error.code.as_str(), error.message)).into())
        }
    }
}

fn focus_unconfirmed(detail: impl Into<String>) -> AdapterError {
    not_delivered(
        ErrorCode::ActionFailed,
        "Could not confirm accessibility focus on the ref's element, so no keys were sent",
    )
    .with_suggestion(
        "Click the field with mouse-click --background, then run type --background --window-id <window> to type into the window's focused element; or snapshot again and retry with a fresh ref.",
    )
    .with_platform_detail(detail)
}

#[cfg(test)]
#[path = "background_keyboard_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "background_keyboard_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "background_keyboard_wait_tests.rs"]
mod wait_tests;
