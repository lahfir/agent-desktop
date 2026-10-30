//! `type --interval-ms`: types text one keystroke unit at a time on a steady
//! clock, inside one command. The first unit is a full ref action: strict
//! resolution, actionability, cursor travel and verification, and a headed one
//! also focuses the target's window. The burst then keeps that element for the
//! remaining units and takes the interaction lease only while each unit is
//! delivered, so other callers act between keystrokes and per-keystroke
//! resolution cannot make the rhythm uneven. Each unit still goes through the
//! adapter's own delivery checks, which for physical keystrokes confirm that the
//! target's process and focused element are unchanged before a key is posted.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::{
    AdapterError, AppError, DeliverySemantics,
    actions::action::Action,
    adapter::PlatformAdapter,
    commands::helpers::{RefArgs, execute_ref_action_with_context},
    commands::type_text::{TypeArgs, default_timeout_ms},
    context::CommandContext,
};

pub(crate) fn execute(
    args: TypeArgs,
    interval_ms: u64,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let units = keystroke_units(&args.text);
    let mut clock = Clock {
        started: Instant::now(),
        interval: Duration::from_millis(interval_ms),
        previous: None,
    };
    let waits = args.timeout_ms != Some(0);
    let budget = |unit: &str| {
        args.timeout_ms
            .filter(|timeout_ms| *timeout_ms > 0)
            .unwrap_or_else(|| default_timeout_ms(unit))
    };
    let mut last = Value::Null;
    let mut burst: Option<Burst> = None;
    for (index, unit) in units.iter().enumerate() {
        clock.wait_for(index);
        let request = context.request_base(Action::TypeText(unit.clone()));
        let delivered = match &burst {
            Some(burst) => deliver(adapter, burst, request, budget(unit)).map(|_| ()),
            None => execute_ref_action_with_context(
                RefArgs {
                    ref_id: args.ref_id.clone(),
                    timeout_ms: waits.then(|| budget(unit)),
                },
                adapter,
                request,
                context,
            )
            .map(|value| last = value),
        };
        delivered.map_err(|error| after_partial_delivery(error, index, units.len()))?;
        if burst.is_none() && index + 1 < units.len() {
            burst = Some(
                resolve_for_burst(adapter, &args.ref_id, context, budget(unit))
                    .map_err(|error| after_partial_delivery(error, index + 1, units.len()))?,
            );
        }
    }
    if let Some(burst) = &burst {
        report_final_state(&mut last, adapter, &burst.handle, budget(&args.text));
    }
    Ok(with_keystrokes(last, units.len(), interval_ms))
}

/// The first keystroke's result carries the field as it was after that
/// keystroke; after a burst it reports the field as the last keystroke left it.
fn report_final_state(
    value: &mut Value,
    adapter: &dyn PlatformAdapter,
    handle: &crate::NativeHandle,
    timeout_ms: u64,
) {
    let Ok(deadline) = crate::Deadline::after(timeout_ms) else {
        return;
    };
    if let (Some(object), Ok(live)) = (
        value.as_object_mut(),
        adapter.get_live_element(handle, deadline),
    ) {
        object.insert("post_state".into(), json!(live.state));
    }
}

/// Keeps keystrokes on the interval's grid, and after a late keystroke never
/// sends the next one sooner than half an interval later, so a slow delivery
/// is not followed by two characters landing in the same frame.
struct Clock {
    started: Instant,
    interval: Duration,
    previous: Option<Instant>,
}

impl Clock {
    fn wait_for(&mut self, index: usize) {
        let offset = self
            .interval
            .saturating_mul(u32::try_from(index).unwrap_or(u32::MAX));
        let on_grid = self.started + offset;
        let due = self.previous.map_or(on_grid, |previous| {
            on_grid.max(previous + self.interval / 2)
        });
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        self.previous = Some(Instant::now());
    }
}

/// The element the first keystroke resolved, and the process instance it
/// belongs to, which every later keystroke must still reach.
struct Burst {
    handle: crate::NativeHandle,
    process: Option<crate::ProcessIdentity>,
}

fn resolve_for_burst(
    adapter: &dyn PlatformAdapter,
    ref_id: &str,
    context: &CommandContext,
    timeout_ms: u64,
) -> Result<Burst, AppError> {
    let entry = crate::commands::helpers::load_ref_entry(ref_id, context)?;
    let deadline = crate::Deadline::after(timeout_ms)?;
    let handle = adapter.resolve_element_strict(&entry, deadline)?;
    let process = entry
        .process
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
        .map(|instance| crate::ProcessIdentity::new(entry.process.pid, instance));
    Ok(Burst { handle, process })
}

fn deliver(
    adapter: &dyn PlatformAdapter,
    burst: &Burst,
    request: crate::ActionRequest,
    timeout_ms: u64,
) -> Result<crate::ActionResult, AppError> {
    let request = match &burst.process {
        Some(process) => request.with_expected_process(process.clone()),
        None => request,
    };
    let deadline = crate::Deadline::after(timeout_ms)?;
    let lease = adapter.acquire_interaction_lease(deadline)?;
    Ok(adapter.execute_action(&burst.handle, request, &lease)?)
}

/// Splits text into the units one keystroke produces: a character together
/// with the combining marks, variation selectors, emoji modifiers and
/// zero-width-joined characters that render with it, and CR LF as one unit.
pub(crate) fn keystroke_units(text: &str) -> Vec<String> {
    let mut units: Vec<String> = Vec::new();
    let mut joining = false;
    for character in text.chars() {
        let attaches = joining
            || extends_previous(character)
            || (character == '\n' && units.last().is_some_and(|unit| unit == "\r"));
        match units.last_mut() {
            Some(unit) if attaches => unit.push(character),
            _ => units.push(character.to_string()),
        }
        joining = character == '\u{200D}';
    }
    units
}

fn extends_previous(character: char) -> bool {
    matches!(
        u32::from(character),
        0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE00..=0xFE0F
            | 0xFE20..=0xFE2F
            | 0x200D
            | 0x1F3FB..=0x1F3FF
            | 0xE0020..=0xE007F
    )
}

fn after_partial_delivery(error: AppError, typed: usize, total: usize) -> AppError {
    if typed == 0 {
        return error;
    }
    let adapter_error = match error {
        AppError::Adapter(error) => error,
        other => AdapterError::internal(other.to_string()),
    };
    let mut details = adapter_error.details.clone().unwrap_or_else(|| json!({}));
    if let Some(object) = details.as_object_mut() {
        object.insert("typed_keystrokes".into(), json!(typed));
        object.insert("keystrokes".into(), json!(total));
    }
    AppError::Adapter(
        adapter_error
            .with_details(details)
            .with_disposition(DeliverySemantics::delivered_unverified()),
    )
}

fn with_keystrokes(mut value: Value, keystrokes: usize, interval_ms: u64) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.insert("keystrokes".into(), json!(keystrokes));
        object.insert("interval_ms".into(), json!(interval_ms));
    }
    value
}

#[cfg(test)]
#[path = "type_text_interval_tests.rs"]
mod tests;
