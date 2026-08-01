use agent_desktop_core::{Deadline, ObservationRoot, ProcessId, WindowInfo, WindowState};
use agent_desktop_windows::tree::element::UIAElement;
use agent_desktop_windows::tree::name_evidence::{LabelOutcome, read_label};
use agent_desktop_windows::tree::properties::{read_live, read_one};
use agent_desktop_windows::tree::property_ids::TreeProperty;
use agent_desktop_windows::tree::walker::{NodeKey, TreeSource, WalkBudget, walk_vocabulary};
use agent_desktop_windows::tree::walker_source::{UiaTreeSource, walk_uia_subtree};
use serde_json::{Value, json};
use std::collections::BTreeSet;

use super::render_census::{control_type_census, provider_census, sample, vocabulary_summary};
use super::render_slots::{field_list, field_presence, role_of, slot, text_presence};
use super::select::Options;

fn node(element: &UIAElement, parent: Option<usize>, index: usize) -> Value {
    let (properties, errors) = read_live(element);
    let label = read_label(element, false);
    let vocabulary = walk_vocabulary(&properties, &label);
    json!({
        "parent_index": parent,
        "child_index": index,
        "control_type": slot(&properties.get(TreeProperty::ControlType)),
        "role": role_of(&vocabulary.role),
        "available_actions": field_list(&vocabulary.available_actions),
        "states": field_list(&vocabulary.states),
        "name": field_presence(&vocabulary.name),
        "description": field_presence(&vocabulary.description),
        "labelled": !matches!(label, LabelOutcome::Unlabelled),
        "class_name": slot(&properties.get(TreeProperty::ClassName)),
        "automation_id": text_presence(&properties.get(TreeProperty::AutomationId)),
        "help_text": text_presence(&properties.get(TreeProperty::HelpText)),
        "full_description": text_presence(&properties.get(TreeProperty::FullDescription)),
        "legacy_default_action": text_presence(&properties.get(TreeProperty::LegacyDefaultAction)),
        "bounds": slot(&properties.get(TreeProperty::BoundingRectangle)),
        "is_password": slot(&properties.get(TreeProperty::IsPassword)),
        "is_offscreen": slot(&properties.get(TreeProperty::IsOffscreen)),
        "is_control_element": slot(&properties.get(TreeProperty::IsControlElement)),
        "is_content_element": slot(&properties.get(TreeProperty::IsContentElement)),
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

/// What bounds the census recursion.
///
/// `--max-depth` alone is adequate for a census of a fixture and is not
/// adequate for a tool a human points at an arbitrary window: a provider that
/// publishes a cyclic child graph or an endless sibling list would hang it.
/// This carries the same three bounds the shipped walker has - a deadline, an
/// ancestor-path cycle guard keyed on the identity the provider publishes, and
/// a sibling cap.
struct Bounds {
    max_depth: u8,
    deadline: Deadline,
    ancestors: Vec<Vec<i32>>,
    truncated: bool,
}

const MAX_CENSUS_SIBLINGS: usize = 10_000;

fn collect(
    source: &UiaTreeSource,
    element: &UIAElement,
    at: Position,
    bounds: &mut Bounds,
    nodes: &mut Vec<Value>,
) {
    if bounds.deadline.is_expired() {
        bounds.truncated = true;
        return;
    }
    let key = match source.identity(element) {
        NodeKey::Runtime(runtime_id) => Some(runtime_id),
        NodeKey::Unavailable => None,
    };
    if let Some(key) = &key {
        if bounds.ancestors.contains(key) {
            bounds.truncated = true;
            return;
        }
    }
    let position = nodes.len();
    nodes.push(node(element, at.parent, at.index));
    if at.depth >= bounds.max_depth {
        bounds.truncated = true;
        return;
    }
    if let Some(key) = key.clone() {
        bounds.ancestors.push(key);
    }
    let mut child = match source.first_child(element) {
        Ok(first) => first,
        Err(_) => {
            if key.is_some() {
                bounds.ancestors.pop();
            }
            return;
        }
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
            bounds,
            nodes,
        );
        child_index += 1;
        if child_index >= MAX_CENSUS_SIBLINGS {
            bounds.truncated = true;
            break;
        }
        match next {
            Ok(sibling) => child = sibling,
            Err(_) => break,
        }
    }
    if key.is_some() {
        bounds.ancestors.pop();
    }
}

/// Walks the target and renders the capture.
///
/// The traversal runs on the shipped `TreeSource`, so the dump exercises the
/// same enumeration surface and cache policy the adapter uses and can never
/// reach the banned `get_children`.
pub fn dump(
    root: &UIAElement,
    hwnd: isize,
    options: &Options,
    deadline: Deadline,
) -> Result<String, String> {
    let source = UiaTreeSource::for_root(root).map_err(|error| error.message.clone())?;
    let prepared = source
        .prepare_root(root)
        .map_err(|error| error.message.clone())?;
    let mut nodes = Vec::new();
    let mut bounds = Bounds {
        max_depth: options.max_depth,
        deadline,
        ancestors: Vec::new(),
        truncated: false,
    };
    collect(
        &source,
        &prepared,
        Position {
            parent: None,
            index: 0,
            depth: 0,
        },
        &mut bounds,
        &mut nodes,
    );
    let document = json!({
        "capture": "uia-vocabulary-census",
        "client_stack": "uia3-com",
        "crate": "uiautomation 0.25.0",
        "target_class": options.class_name,
        "target_variant": options.variant,
        "os_build": os_build(),
        "max_depth": options.max_depth,
        "census_truncated": bounds.truncated,
        "node_count": nodes.len(),
        "walk": walk_verdict(&prepared, deadline),
        "vocabulary": vocabulary_summary(&nodes),
        "view_delta": view_delta(hwnd, options.max_depth, deadline),
        "control_types": control_type_census(&nodes),
        "providers": provider_census(&nodes),
        "sample": sample(&nodes),
    });
    serde_json::to_string_pretty(&document).map_err(|error| error.to_string())
}

/// The RawView-versus-ControlView delta KTD11 leaves unreconciled: the code
/// opens the raw view walker while the document mandates the control view.
///
/// A15-10 measured them identical on a Win32 fixture and five nodes apart on
/// WPF, so the delta is provider-dependent. Reporting it per real target is
/// what lets 2.4 decide on data rather than on an argument.
fn view_delta(hwnd: isize, max_depth: u8, deadline: Deadline) -> Value {
    let count = |control_view: bool| -> Value {
        match count_view(hwnd, control_view, max_depth, deadline) {
            Err(reason) => json!({ "failed": reason }),
            Ok((nodes, control_types)) => json!({
                "nodes": nodes,
                "control_types": control_types.into_iter().collect::<Vec<_>>(),
            }),
        }
    };
    json!({ "raw_view": count(false), "control_view": count(true) })
}

/// Counts the nodes and distinct control types one tree view presents.
///
/// Enumerated with `get_first_child`/`get_next_sibling` rather than the
/// crate's `get_children`, which retires end-of-siblings and a real
/// cross-process fault through the same arm.
fn count_view(
    hwnd: isize,
    control_view: bool,
    max_depth: u8,
    deadline: Deadline,
) -> Result<(usize, BTreeSet<i32>), String> {
    use agent_desktop_windows::tree::automation::automation_client;
    let client = automation_client().map_err(|error| error.message.clone())?;
    let walker = if control_view {
        client.get_control_view_walker()
    } else {
        client.get_raw_view_walker()
    }
    .map_err(|error| error.to_string())?;
    let mut control_types = BTreeSet::new();
    let mut nodes = 0;
    let root = client
        .element_from_handle(uiautomation::types::Handle::from(hwnd))
        .map_err(|error| error.to_string())?;
    let mut stack = vec![(root, 0_u8)];
    while let Some((element, depth)) = stack.pop() {
        if deadline.is_expired() {
            break;
        }
        nodes += 1;
        if let Ok(uiautomation::variants::Value::I4(id)) = element
            .get_property_value(uiautomation::types::UIProperty::ControlType)
            .and_then(|variant| variant.get_value())
        {
            control_types.insert(id);
        }
        if depth >= max_depth {
            continue;
        }
        let Ok(mut child) = walker.get_first_child(&element) else {
            continue;
        };
        loop {
            let next = walker.get_next_sibling(&child);
            stack.push((child, depth + 1));
            match next {
                Ok(sibling) => child = sibling,
                Err(_) => break,
            }
        }
    }
    Ok((nodes, control_types))
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
