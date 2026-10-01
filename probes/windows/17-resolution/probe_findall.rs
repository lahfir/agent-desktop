//! FindAll versus the shipped `search_under` walk (U1-1 / KTD8).
//!
//! The measure is the architectural choice per stored ref: with a known
//! AutomationId, `find_all(Descendants, property-condition(id))` reaches the
//! candidates without walking the tree. The probe measures three things on
//! the probe's own fixture and the attached targets:
//!
//! - correctness: for a set of concrete, probe-known identifiers, does
//!   `find_all` return exactly the elements the walk's AutomationId-first
//!   search would collect (count equality on the fixture, per-id), what it
//!   returns for an absent id, and whether the union over distinct ids of
//!   `find_all` equals the walk's id-bearing set;
//! - cost: min-of-seven of one `find_all` for a stored id versus one full
//!   resolve-scoped walk;
//! - the discipline question: `find_all` is a single blocking call - it can
//!   neither be interrupted at a deadline mid-call nor signal a partial
//!   result; it is all-or-nothing (full Vec or Err). That is recorded as a
//!   property of the API, not measured.

use std::time::Instant;

use serde_json::{Value, json};
use uiautomation::types::{PropertyConditionFlags, TreeScope, UIProperty};
use uiautomation::UIElement;
use uiautomation::UIAutomation;

use crate::measure::collect_descendants;

pub const FINDALL_REPEATS: usize = 7;

fn min_of(operation: impl FnMut() -> Result<(), ()>) -> (f64, f64, f64) {
    let mut samples = Vec::with_capacity(FINDALL_REPEATS);
    let mut closure = operation;
    let _ = closure();
    for _ in 0..FINDALL_REPEATS {
        let started = Instant::now();
        let _ = closure();
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    (
        samples[0],
        samples[samples.len() / 2],
        samples[samples.len() - 1],
    )
}


fn find_all_id(
    automation: &UIAutomation,
    root: &UIElement,
    id: &str,
) -> Result<Vec<UIElement>, uiautomation::Error> {
    let condition = automation.create_property_condition(
        UIProperty::AutomationId,
        id.to_string().into(),
        Some(PropertyConditionFlags::None),
    )?;
    root.find_all(TreeScope::Descendants, &condition)
}

/// The window's ever-present button and the duplicate pair carry ids so a
/// per-id count assertion is possible on the own fixture. On attached targets
/// this collapses to the whole-tree union equality arm.
pub fn measure_findall(automation: &UIAutomation, root: &UIElement, target: &str) -> Value {
    let walker = match automation.get_raw_view_walker() {
        Ok(walker) => walker,
        Err(error) => return json!({ "walker_failed": format!("{error}") }),
    };
    let walked = collect_descendants(&walker, root, crate::measure::WALK_DEPTH_LIMIT);
    let walked_evidence: Vec<super::measure::Evidence> = walked.iter().map(super::measure::read_evidence).collect();

    let walk_time = min_of(|| {
        let mut walked_vec = Vec::new();
        collect_descendants(&walker, root, crate::measure::WALK_DEPTH_LIMIT)
            .into_iter()
            .for_each(|element| walked_vec.push(element));
        let _ = walked_vec;
        Ok(())
    });

    let mut distinct_ids: Vec<&str> = Vec::new();
    for evidence in &walked_evidence {
        if let Some(id) = evidence.native_id.as_deref() {
            if !distinct_ids.contains(&id) {
                distinct_ids.push(id);
            }
        }
    }

    let mut per_id: Vec<Value> = Vec::new();
    let mut findall_union_count = 0usize;
    let mut walk_id_union_count = 0usize;
    for id in &distinct_ids {
        match find_all_id(automation, root, id) {
            Ok(found) => {
                let walked_count = walked_evidence
                    .iter()
                    .filter(|evidence| evidence.native_id.as_deref() == Some(*id))
                    .count();
                findall_union_count += found.len();
                walk_id_union_count += walked_count;
                per_id.push(json!({
                    "id_length": id.chars().count(),
                    "findall_count": found.len(),
                    "walk_count": walked_count,
                    "counts_agree": found.len() == walked_count,
                }));
            }
            Err(error) => {
                per_id.push(json!({
                    "id_length": id.chars().count(),
                    "findall_failed_hresult": format!("{error}"),
                }));
            }
        }
    }

    let known_id = distinct_ids.first().copied().unwrap_or("absent-id");
    let find_time = min_of(|| {
        find_all_id(automation, root, known_id).map(|_| ()).map_err(|_| ())
    });

    let absent_time = min_of(|| {
        find_all_id(automation, root, "zz-probe-absent-id").map(|_| ()).map_err(|_| ())
    });
    let mut absent_count = None;
    if let Ok(found) = find_all_id(automation, root, "zz-probe-absent-id") {
        absent_count = Some(found.len());
    }

    json!({
        "target": target,
        "walked_elements": walked.len(),
        "walk_candidate_reads": walked.len(),
        "findall": {
            "distinct_ids": distinct_ids.len(),
            "union_counts_agree": findall_union_count == walk_id_union_count,
            "findall_union_count": findall_union_count,
            "walk_union_count": walk_id_union_count,
            "per_id": per_id,
            "absent_id_returns": absent_count,
        },
        "timing_ms": {
            "resolve_scoped_walk": {
                "min": walk_time.0,
                "median": walk_time.1,
                "max": walk_time.2,
            },
            "findall_one_id": {
                "min": find_time.0,
                "median": find_time.1,
                "max": find_time.2,
            },
            "findall_absent_id": {
                "min": absent_time.0,
                "median": absent_time.1,
                "max": absent_time.2,
            },
        },
        "discipline": {
            "single_blocking_call": true,
            "caller_boundable": false,
            "partial_result_signal": "none - all-or-nothing Vec or Err",
        },
    })
}