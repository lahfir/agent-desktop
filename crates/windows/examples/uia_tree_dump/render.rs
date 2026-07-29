use agent_desktop_core::{Deadline, ObservationRoot, ProcessId, WindowInfo, WindowState};
use agent_desktop_windows::tree::element::UIAElement;
use agent_desktop_windows::tree::properties::{
    PropertyOutcome, PropertyValue, read_live, read_one,
};
use agent_desktop_windows::tree::property_ids::TreeProperty;
use agent_desktop_windows::tree::walker::{TreeSource, WalkBudget};
use agent_desktop_windows::tree::walker_source::{UiaTreeSource, walk_uia_subtree};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use super::select::Options;

/// Substituted for every run-varying host value, matching the placeholders the
/// 2.0 captures already use.
const REDACTED_PID: &str = "<pid>";
const REDACTED_PROVIDER: &str = "<providerid>";
const REDACTED_PATH: &str = "<userprofile>";

/// Reports an unresolvable target as a structured skip.
///
/// Never a silent empty dump: a capture containing nothing reads as evidence
/// that the tree was empty.
pub fn skipped(reason: &str) -> String {
    json!({ "status": "skipped", "reason": reason }).to_string()
}

/// Replaces every run-varying or host-identifying value in a provider string.
///
/// `ProviderDescription` embeds a process id and a hexadecimal provider id,
/// and a user path can appear in a module path, so the substitution runs
/// before anything reaches the capture file. Each key consumes its **whole**
/// value: a substitution that stops at the first non-digit leaves the tail of
/// a `0x`-prefixed id in the file, which is a leak that reads as redacted.
fn normalise(text: &str) -> String {
    let substituted = substitute_after(text, "pid:", REDACTED_PID);
    let substituted = substitute_after(&substituted, "providerId:", REDACTED_PROVIDER);
    redact_paths(&substituted)
}

fn substitute_after(text: &str, key: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(key) {
        out.push_str(&rest[..at + key.len()]);
        let value = &rest[at + key.len()..];
        let consumed = value
            .find(|character: char| !(character.is_ascii_alphanumeric()))
            .unwrap_or(value.len());
        if consumed == 0 {
            rest = value;
            continue;
        }
        out.push_str(replacement);
        rest = &value[consumed..];
    }
    out.push_str(rest);
    out
}

fn redact_paths(text: &str) -> String {
    let lowered = text.to_ascii_lowercase();
    match lowered.find(r"\users\") {
        None => text.to_string(),
        Some(at) => {
            let tail = &text[at + r"\users\".len()..];
            let end = tail.find('\\').unwrap_or(tail.len());
            format!("{}{}{}", &text[..at], REDACTED_PATH, &tail[end..])
        }
    }
}

fn text_of(outcome: &PropertyOutcome) -> Option<String> {
    match outcome {
        PropertyOutcome::Known(PropertyValue::Text(value)) => Some(value.clone()),
        _ => None,
    }
}

fn slot(outcome: &PropertyOutcome) -> Value {
    match outcome {
        PropertyOutcome::Known(PropertyValue::Text(value)) => json!(normalise(value)),
        PropertyOutcome::Known(PropertyValue::Flag(value)) => json!(value),
        PropertyOutcome::Known(PropertyValue::Number(value)) => json!(value),
        PropertyOutcome::Known(PropertyValue::Bounds(bounds)) => json!({
            "x": bounds.x, "y": bounds.y, "width": bounds.width, "height": bounds.height
        }),
        PropertyOutcome::Absent => json!("<absent>"),
        PropertyOutcome::Unknown => json!("<unknown>"),
    }
}

/// `Name` is recorded as presence and length only, per the corpus redaction
/// rule: a document node's name is user content.
fn name_presence(outcome: &PropertyOutcome) -> Value {
    match text_of(outcome) {
        Some(value) if value.is_empty() => json!({ "present": true, "chars": 0 }),
        Some(value) => json!({ "present": true, "chars": value.chars().count() }),
        None => json!({ "present": false, "outcome": slot(outcome) }),
    }
}

fn node(element: &UIAElement, parent: Option<usize>, index: usize) -> Value {
    let (properties, errors) = read_live(element);
    json!({
        "parent_index": parent,
        "child_index": index,
        "control_type": slot(&read_one(element, TreeProperty::ControlType)),
        "class_name": slot(&properties.get(TreeProperty::ClassName)),
        "automation_id": slot(&properties.get(TreeProperty::AutomationId)),
        "name": name_presence(&properties.get(TreeProperty::Name)),
        "bounds": slot(&properties.get(TreeProperty::BoundingRectangle)),
        "is_password": slot(&properties.get(TreeProperty::IsPassword)),
        "is_offscreen": slot(&properties.get(TreeProperty::IsOffscreen)),
        "provider_description": slot(&read_one(element, TreeProperty::ProviderDescription)),
        "failed_reads": errors.len(),
    })
}

/// Where one node sits in the dump being built.
struct Position {
    parent: Option<usize>,
    index: usize,
    depth: u8,
}

fn collect(
    source: &UiaTreeSource,
    element: &UIAElement,
    at: Position,
    max_depth: u8,
    nodes: &mut Vec<Value>,
) {
    let position = nodes.len();
    nodes.push(node(element, at.parent, at.index));
    if at.depth >= max_depth {
        return;
    }
    let mut child = match source.first_child(element) {
        Ok(first) => first,
        Err(_) => return,
    };
    let mut child_index = 0;
    loop {
        let next = source.next_sibling(&child);
        collect(
            source,
            &child,
            Position {
                parent: Some(position),
                index: child_index,
                depth: at.depth + 1,
            },
            max_depth,
            nodes,
        );
        child_index += 1;
        match next {
            Ok(sibling) => child = sibling,
            Err(_) => return,
        }
    }
}

/// Walks the target and renders the capture.
///
/// The traversal runs on the shipped `TreeSource`, so the dump exercises the
/// same enumeration surface and cache policy the adapter uses and can never
/// reach the banned `get_children`.
pub fn dump(root: &UIAElement, options: &Options, deadline: Deadline) -> Result<String, String> {
    let source = UiaTreeSource::for_root(root).map_err(|error| error.message.clone())?;
    let prepared = source
        .prepare_root(root)
        .map_err(|error| error.message.clone())?;
    let mut nodes = Vec::new();
    collect(
        &source,
        &prepared,
        Position {
            parent: None,
            index: 0,
            depth: 0,
        },
        options.max_depth,
        &mut nodes,
    );
    let document = json!({
        "capture": "uia-tree-census",
        "client_stack": "uia3-com",
        "crate": "uiautomation 0.25.0",
        "target_class": options.class_name,
        "target_variant": options.variant,
        "os_build": os_build(),
        "max_depth": options.max_depth,
        "node_count": nodes.len(),
        "walk": walk_verdict(&prepared, deadline),
        "control_types": control_type_census(&nodes),
        "providers": provider_census(&nodes),
        "sample": sample(&nodes),
    });
    serde_json::to_string_pretty(&document).map_err(|error| error.to_string())
}

/// One row per `ControlType` the target exposes.
///
/// This is what 2.3's role map and 2.4's surface detection actually read. A
/// full per-node dump carried the same information a hundred times over -
/// measured on Explorer, 100 nodes spread across 23 control types, 40 class
/// names and 15 provider strings, with 85 of the provider strings verbatim
/// repeats and every bounds value unassertable by rule (R9, KTD8).
fn control_type_census(nodes: &[Value]) -> Value {
    let mut rows: BTreeMap<String, (usize, BTreeSet<String>, usize, usize)> = BTreeMap::new();
    for node in nodes {
        let key = node["control_type"].to_string();
        let row = rows.entry(key).or_insert((0, BTreeSet::new(), 0, 0));
        row.0 += 1;
        row.1.insert(node["class_name"].to_string());
        row.2 += usize::from(node["automation_id"].as_str() != Some("<absent>"));
        row.3 += usize::from(node["name"]["present"].as_bool().unwrap_or(false));
    }
    Value::Array(
        rows.into_iter()
            .map(|(control_type, (count, class_names, with_id, with_name))| {
                json!({
                    "control_type": control_type,
                    "nodes": count,
                    "class_names": class_names.into_iter().collect::<Vec<_>>(),
                    "with_automation_id": with_id,
                    "with_name": with_name,
                })
            })
            .collect(),
    )
}

/// One row per distinct provider, which is what the cache policy keys on.
fn provider_census(nodes: &[Value]) -> Value {
    let mut rows: BTreeMap<String, usize> = BTreeMap::new();
    for node in nodes {
        *rows
            .entry(node["provider_description"].to_string())
            .or_default() += 1;
    }
    Value::Array(
        rows.into_iter()
            .map(|(provider, count)| json!({ "provider": provider, "nodes": count }))
            .collect(),
    )
}

/// One fully-detailed node per control type, so the shape stays inspectable
/// without repeating it for every sibling.
fn sample(nodes: &[Value]) -> Value {
    let mut seen = BTreeSet::new();
    Value::Array(
        nodes
            .iter()
            .filter(|node| seen.insert(node["control_type"].to_string()))
            .cloned()
            .collect(),
    )
}

fn os_build() -> String {
    std::process::Command::new("cmd")
        .args(["/c", "ver"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_string())
        .unwrap_or_else(|| String::from("<unrecorded>"))
}

/// Runs the shipped walker over the same target and reports its verdict.
///
/// This is the evidence that matters beyond the node list: whether the
/// traversal the adapter ships reports a real application's tree **complete**.
/// An inverted discriminator or a swallowed enumeration fault shows up here as
/// a false `true`, which is why the capture records the failure count beside
/// it rather than the verdict alone.
fn walk_verdict(root: &UIAElement, deadline: Deadline) -> Value {
    let window = WindowInfo {
        id: String::from("<hwnd>"),
        title: String::new(),
        app: String::new(),
        pid: ProcessId::from(0_u32),
        process_instance: None,
        bounds: None,
        state: WindowState {
            is_focused: false,
            minimized: None,
            visible: Some(true),
        },
    };
    match walk_uia_subtree(
        root,
        &ObservationRoot::Window(&window),
        WalkBudget::new(u8::MAX, deadline),
    ) {
        Ok(outcome) => json!({
            "structurally_complete": outcome.tree.is_complete(),
            "nodes_observed": outcome.tree.node_count(),
            "enumeration_failures": outcome.failures.len(),
            "cycles_skipped": outcome.stats.traversal.cycles_skipped,
        }),
        Err(error) => json!({ "failed": error.message }),
    }
}
