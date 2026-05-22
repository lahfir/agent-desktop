use super::*;
use crate::node::{AccessibilityNode, Rect};

fn node(role: &str, name: Option<&str>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        name: name.map(str::to_string),
        value: None,
        description: None,
        hint: None,
        states: vec![],
        available_actions: vec![],
        bounds: Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }),
        children_count: None,
        children: vec![],
    }
}

#[test]
fn transform_tree_include_bounds_false_strips_bounds() {
    let n = node("group", None);
    let out = transform_tree(n, false, false, false);
    assert!(out.bounds.is_none());
}

#[test]
fn transform_tree_include_bounds_true_preserves_bounds() {
    let n = node("group", None);
    let out = transform_tree(n, true, false, false);
    assert!(out.bounds.is_some());
}

#[test]
fn transform_tree_interactive_only_prunes_noninteractive_leaves() {
    let mut root = node("window", Some("w"));
    root.children = vec![node("group", None), node("button", Some("OK"))];
    let out = transform_tree(root, true, true, false);
    assert_eq!(out.children.len(), 1);
    assert_eq!(out.children[0].role, "button");
}

#[test]
fn transform_tree_interactive_only_keeps_named_containers_with_children() {
    let mut labeled = node("group", Some("Toolbar"));
    labeled.children = vec![node("button", Some("Save"))];
    let mut root = node("window", Some("w"));
    root.children = vec![labeled];
    let out = transform_tree(root, true, true, false);
    assert_eq!(out.children.len(), 1);
    assert_eq!(out.children[0].children.len(), 1);
}

#[test]
fn transform_tree_compact_collapses_empty_single_child_chain() {
    let mut outer = node("group", None);
    let mut inner = node("group", None);
    inner.children = vec![node("button", Some("Go"))];
    outer.children = vec![inner];
    let mut root = node("window", Some("w"));
    root.children = vec![outer];
    let out = transform_tree(root, true, false, true);
    assert_eq!(out.children.len(), 1);
    assert_eq!(out.children[0].role, "button");
}

#[test]
fn transform_tree_compact_preserves_labeled_containers() {
    let mut named = node("group", Some("Toolbar"));
    named.children = vec![node("button", Some("Save"))];
    let mut root = node("window", Some("w"));
    root.children = vec![named];
    let out = transform_tree(root, true, false, true);
    assert_eq!(out.children.len(), 1);
    assert_eq!(out.children[0].role, "group");
    assert_eq!(out.children[0].name.as_deref(), Some("Toolbar"));
}

#[test]
fn ref_entry_prefers_platform_actions() {
    let mut button = node("button", Some("Save"));
    button.available_actions = vec!["SetFocus".into()];

    let entry = ref_entry_from_node(&button, 7, None, None, None, None, &[0]);

    assert_eq!(entry.available_actions, vec!["SetFocus"]);
}

#[test]
fn ref_entry_drops_empty_identity_text() {
    let mut button = node("button", Some(""));
    button.value = Some(String::new());

    let entry = ref_entry_from_node(&button, 7, None, None, None, None, &[0]);

    assert!(entry.name.is_none());
    assert!(entry.value.is_none());
}

#[test]
fn ref_entry_preserves_meaningful_identity_text() {
    let mut button = node("button", Some("Save"));
    button.value = Some("Primary".into());
    button.description = Some("Commits changes".into());

    let entry = ref_entry_from_node(&button, 7, None, None, None, None, &[0]);

    assert_eq!(entry.name.as_deref(), Some("Save"));
    assert_eq!(entry.value.as_deref(), Some("Primary"));
    assert_eq!(entry.description.as_deref(), Some("Commits changes"));
}

#[test]
fn allocate_refs_records_structural_paths() {
    let mut root = node("window", Some("w"));
    let mut group = node("group", Some("List"));
    group.children = vec![node("button", Some("Open"))];
    root.children = vec![node("button", Some("Save")), group];

    let mut refmap = RefMap::new();
    let config = RefAllocConfig {
        include_bounds: true,
        interactive_only: false,
        compact: false,
        pid: 7,
        source_app: Some("Finder"),
        source_window_id: Some("w-42"),
        source_window_title: Some("Documents"),
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref_id: None,
        path_prefix: &[],
    };
    let out = allocate_refs(root, &mut refmap, &config);

    let save_ref = out.children[0].ref_id.as_deref().unwrap();
    let open_ref = out.children[1].children[0].ref_id.as_deref().unwrap();
    assert_eq!(refmap.get(save_ref).unwrap().path.as_slice(), [0]);
    assert_eq!(refmap.get(open_ref).unwrap().path.as_slice(), [1, 0]);
    assert_eq!(
        refmap.get(open_ref).unwrap().source_window_id.as_deref(),
        Some("w-42")
    );
}

#[test]
fn allocate_refs_hides_bounds_from_refmap_when_snapshot_hides_bounds() {
    let mut root = node("window", Some("w"));
    root.children = vec![node("button", Some("Open"))];
    let mut refmap = RefMap::new();
    let config = RefAllocConfig {
        include_bounds: false,
        interactive_only: false,
        compact: false,
        pid: 7,
        source_app: Some("Finder"),
        source_window_id: Some("w-42"),
        source_window_title: Some("Documents"),
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref_id: None,
        path_prefix: &[],
    };

    let out = allocate_refs(root, &mut refmap, &config);
    let open_ref = out.children[0].ref_id.as_deref().unwrap();
    let entry = refmap.get(open_ref).unwrap();

    assert!(out.children[0].bounds.is_none());
    assert!(entry.bounds.is_none());
    assert!(entry.bounds_hash.is_none());
    assert_eq!(entry.path.as_slice(), [0]);
    assert_eq!(entry.source_window_id.as_deref(), Some("w-42"));
    assert_eq!(entry.source_window_title.as_deref(), Some("Documents"));
}

#[test]
fn allocate_refs_keeps_bounds_in_refmap_when_snapshot_includes_bounds() {
    let mut root = node("window", Some("w"));
    root.children = vec![node("button", Some("Open"))];
    let mut refmap = RefMap::new();
    let config = RefAllocConfig {
        include_bounds: true,
        interactive_only: false,
        compact: false,
        pid: 7,
        source_app: Some("Finder"),
        source_window_id: Some("w-42"),
        source_window_title: Some("Documents"),
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref_id: None,
        path_prefix: &[],
    };

    let out = allocate_refs(root, &mut refmap, &config);
    let open_ref = out.children[0].ref_id.as_deref().unwrap();
    let entry = refmap.get(open_ref).unwrap();

    assert!(out.children[0].bounds.is_some());
    assert!(entry.bounds.is_some());
    assert!(entry.bounds_hash.is_some());
}
