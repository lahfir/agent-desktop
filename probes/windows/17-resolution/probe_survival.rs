//! Electron path/geometry survival arm (U1-8).
//!
//! A7-2 measured the positional child-index path surviving 100% across
//! process restart on the **native** stacks; the identifier-free fallback
//! (U3) rests on that path surviving on Electron too, which no probe has ever
//! measured. This arm walks an attached tree, finds the repo-controlled
//! marker notes (`probe-*.md`) the orchestrator staged in a scratch Obsidian
//! vault, and records each one's identity evidence plus its child-index path
//! from the window root. Path and geometry survival across app relaunch and
//! in-page mutation are computed by the orchestrator from two captures.
//!
//! Shape and count only. A marker's `Name` never reaches the capture: it is
//! reduced at the point of record to a stable content-free digest plus its
//! length, which is all the orchestrator needs to pair the same element across
//! two captures and all the ledger needs to report survival.

use serde_json::{Value, json};
use uiautomation::types::UIProperty;
use uiautomation::{UIElement, UITreeWalker};
use uiautomation::UIAutomation;

use crate::measure::{boolean_of, read_evidence, WALK_DEPTH_LIMIT};

const MARKER_PREFIX: &str = "probe-note";
const MAX_MARKERS: usize = 60;

#[derive(Clone, Debug)]
struct MarkerRecord {
    name_digest: String,
    name_length: usize,
    control_type: i32,
    path: Vec<i32>,
    positive_area: bool,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// FNV-1a over the UTF-8 bytes, rendered hex. Deterministic across processes
/// and runs, which is what lets two captures taken minutes apart be paired
/// element by element, and one-way, which is what keeps the text out of the
/// committed evidence.
fn name_digest(name: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in name.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Obsidian strips the `.md` extension for display; the marker prefix is the
/// authoritative token either way.
fn is_marker_name(name: &str) -> bool {
    let text = name.trim().trim_end_matches(".md");
    text == MARKER_PREFIX || text.starts_with(&format!("{MARKER_PREFIX}-"))
}

fn walk_paths(
    walker: &UITreeWalker,
    element: &UIElement,
    depth: u32,
    path: &mut Vec<i32>,
    out: &mut Vec<MarkerRecord>,
) {
    if depth >= WALK_DEPTH_LIMIT || out.len() >= MAX_MARKERS {
        return;
    }
    if !path.is_empty() {
        let evidence = read_evidence(element);
        let name = evidence.name.as_deref().unwrap_or("");
        if is_marker_name(name) {
            out.push(MarkerRecord {
                name_digest: name_digest(name),
                name_length: name.chars().count(),
                control_type: evidence.control_type,
                path: path.clone(),
                positive_area: !evidence.zero_extent(),
                left: evidence.left,
                top: evidence.top,
                right: evidence.right,
                bottom: evidence.bottom,
            });
            return;
        }
    }
    let children = crate::measure::enumerate_children(walker, element);
    for (index, child) in children.iter().enumerate() {
        path.push(index as i32);
        walk_paths(walker, child, depth + 1, path, out);
        path.pop();
    }
}

/// Walks the attached root and reports every marker-named element's digest,
/// path and bounds. `markers_found` records how many matched, so a capture in
/// which nothing matched is distinguishable from an environment where the
/// target answer came back empty.
pub fn measure_survival(automation: &UIAutomation, root: &UIElement) -> Value {
    let walker = match automation.get_raw_view_walker() {
        Ok(walker) => walker,
        Err(error) => return json!({ "walker_failed": format!("{error}") }),
    };
    let mut records = Vec::new();
    let mut path = Vec::new();
    walk_paths(&walker, root, 0, &mut path, &mut records);
    records.truncate(MAX_MARKERS);
    records.sort_by(|a, b| a.name_digest.cmp(&b.name_digest));

    let rows: Vec<Value> = records
        .iter()
        .map(|record| {
            json!({
                "name_digest": record.name_digest,
                "name_length": record.name_length,
                "marker": true,
                "control_type": record.control_type,
                "path": record.path,
                "path_depth": record.path.len(),
                "positive_area": record.positive_area,
                "bounds": [record.left, record.top, record.right, record.bottom],
            })
        })
        .collect();

    let password_control_types: Vec<i32> = secure_shape_census(&walker, root);

    json!({
        "markers_found": records.len(),
        "marker_rows": rows,
        "password_element_control_types": password_control_types,
    })
}

/// The secure-live-read gate rides the same read set on this target: the
/// shared read's `IsPassword` flag, reported as a shape census.
fn secure_shape_census(walker: &UITreeWalker, root: &UIElement) -> Vec<i32> {
    let mut seen = Vec::new();
    for element in crate::measure::collect_descendants(walker, root, WALK_DEPTH_LIMIT) {
        if element
            .get_property_value(UIProperty::IsPassword)
            .ok()
            .and_then(|variant| boolean_of(&variant))
            .unwrap_or(false)
        {
            let control_type = crate::measure::control_type_of(&element);
            if !seen.contains(&control_type) {
                seen.push(control_type);
            }
        }
    }
    seen
}
