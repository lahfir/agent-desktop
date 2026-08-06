//! The marginal-cost measurement for the three new properties 2.4 adds to the
//! walk (`LocalizedControlType`, `AriaRole`, `AriaProperties`).
//!
//! A15-13's methodology: min of seven repeats after a discarded warm-up, with
//! the median and max reported beside it so a single `min` sample is never
//! read as the answer. The base set mirrors A15-11's comparison - a
//! representative flat set - against the same set plus the three new
//! properties, so the measured envelope generalizes to the walk's shape.

use std::time::Instant;

use serde_json::{Value, json};
use uiautomation::UIAutomation;
use uiautomation::types::UIProperty;
use uiautomation::UIElement;

use crate::properties::{BASE_SET, EXTRA_SET};
use crate::measure::collect_descendants;

const WARMUP_REPEATS: usize = 1;
const MEASURED_REPEATS: usize = 7;

fn walk_cost(
    automation: &UIAutomation,
    root: &UIElement,
    properties: &[UIProperty],
) -> Result<u128, String> {
    let walker = automation
        .get_raw_view_walker()
        .map_err(|error| format!("{error}"))?;
    let started = Instant::now();
    let elements = collect_descendants(&walker, root, 24);
    for element in &elements {
        for property in properties {
            let _ = element.get_property_value(*property);
        }
    }
    Ok(started.elapsed().as_micros())
}

fn min_of_repeats(automation: &UIAutomation, root: &UIElement, properties: &[UIProperty]) -> Value {
    let mut samples = Vec::with_capacity(MEASURED_REPEATS);
    for _ in 0..WARMUP_REPEATS {
        if let Err(error) = walk_cost(automation, root, properties) {
            return not_measured(&error);
        }
    }
    for _ in 0..MEASURED_REPEATS {
        match walk_cost(automation, root, properties) {
            Ok(sample) => samples.push(sample),
            Err(error) => return not_measured(&error),
        }
    }
    let mut sorted = samples;
    sorted.sort_unstable();
    let (min, median, max) = match (sorted.first(), sorted.get(sorted.len() / 2), sorted.last()) {
        (Some(min), Some(median), Some(max)) => (*min, *median, *max),
        _ => return not_measured("the cost walk produced no samples"),
    };
    json!({
        "warmup_discarded": WARMUP_REPEATS,
        "repeats": MEASURED_REPEATS,
        "min_us": min,
        "median_us": median,
        "max_us": max,
        "spread_ratio": if min > 0 { (max as f64 / min as f64 * 100.0).round() / 100.0 } else { 0.0 },
    })
}

/// A cost walk that could not run is recorded as a placeholder the orchestrator's
/// mandatory-measurement gate reads as "not measured", never as a zero-cost walk
/// and never as a panic that would leave the capture unwritten.
fn not_measured(reason: &str) -> Value {
    json!({
        "not_measured": true,
        "skipped": reason,
        "warmup_discarded": WARMUP_REPEATS,
        "repeats": MEASURED_REPEATS,
    })
}

/// Measures the marginal cost of the three new properties as the ratio of the
/// full walk (with the extras) over the base walk, both min-of-seven.
pub fn measure_extra_cost(automation: &UIAutomation, root: &UIElement) -> Value {
    let base = min_of_repeats(automation, root, &BASE_SET);
    let extra = min_of_repeats(automation, root, &EXTRA_SET);
    let mut all = BASE_SET.to_vec();
    all.extend_from_slice(&EXTRA_SET);
    let combined = min_of_repeats(automation, root, &all);
    let base_min = base["min_us"].as_u64();
    let combined_min = combined["min_us"].as_u64();
    let ratio = match (base_min, combined_min) {
        (Some(base_us), Some(combined_us)) if base_us > 0 => {
            json!((combined_us as f64 / base_us as f64 * 100.0).round() / 100.0)
        }
        _ => Value::Null,
    };
    json!({
        "base": base,
        "extra": extra,
        "combined": combined,
        "overhead_ratio": {
            "base": base_min,
            "combined": combined_min,
            "ratio": ratio,
            "measured": !ratio.is_null(),
        },
    })
}
