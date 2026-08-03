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

fn walk_cost(automation: &UIAutomation, root: &UIElement, properties: &[UIProperty]) -> u128 {
    let walker = automation
        .get_raw_view_walker()
        .expect("raw-view walker must resolve for the cost walk");
    let started = Instant::now();
    let elements = collect_descendants(&walker, root, 24);
    for element in &elements {
        for property in properties {
            let _ = element.get_property_value(*property);
        }
    }
    started.elapsed().as_micros()
}

fn min_of_repeats(automation: &UIAutomation, root: &UIElement, properties: &[UIProperty]) -> Value {
    let mut samples = Vec::with_capacity(MEASURED_REPEATS);
    for _ in 0..WARMUP_REPEATS {
        let _ = walk_cost(automation, root, properties);
    }
    for _ in 0..MEASURED_REPEATS {
        samples.push(walk_cost(automation, root, properties));
    }
    let mut sorted = samples.clone();
    sorted.sort_unstable();
    json!({
        "warmup_discarded": WARMUP_REPEATS,
        "repeats": MEASURED_REPEATS,
        "min_us": sorted[0],
        "median_us": sorted[sorted.len() / 2],
        "max_us": sorted[sorted.len() - 1],
        "spread_ratio": if sorted[0] > 0 { (sorted[sorted.len() - 1] as f64 / sorted[0] as f64 * 100.0).round() / 100.0 } else { 0.0 },
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
    json!({
        "base": base,
        "extra": extra,
        "combined": combined,
        "overhead_ratio": {
            "base": base["min_us"].as_u64().unwrap_or(0),
            "combined": combined["min_us"].as_u64().unwrap_or(0),
            "ratio": if base["min_us"].as_u64().unwrap_or(0) > 0 {
                (combined["min_us"].as_u64().unwrap() as f64 / base["min_us"].as_u64().unwrap() as f64 * 100.0).round() / 100.0
            } else { 0.0 },
        },
    })
}
