use crate::node::AccessibilityNode;
use crate::ref_alloc::{self, RefAllocConfig};
use crate::refs::RefMap;

fn node(role: &str) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        name: None,
        value: None,
        description: None,
        native_id: None,
        hint: None,
        states: vec![],
        available_actions: vec![],
        bounds: None,
        children_count: None,
        children: vec![],
    }
}

fn drill_config<'a>(
    source_app: Option<&'a str>,
    pid: i32,
    root_ref_id: &'a str,
    interactive_only: bool,
    compact: bool,
) -> RefAllocConfig<'a> {
    RefAllocConfig {
        include_bounds: false,
        interactive_only,
        compact,
        pid,
        source_app,
        source_window_id: None,
        source_window_title: Some("Drill Window"),
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref_id: Some(root_ref_id),
        path_prefix: &[],
    }
}

#[test]
fn test_drill_alloc_tags_entries() {
    let mut btn = node("button");
    btn.name = Some("Submit".into());
    let mut root = node("group");
    root.children = vec![btn];

    let mut refmap = RefMap::new();
    let config = drill_config(Some("TestApp"), 42, "@e5", false, false);
    let tree = ref_alloc::allocate_refs(root, &mut refmap, &config);

    assert_eq!(refmap.len(), 1);
    let btn_ref = tree.children[0]
        .ref_id
        .as_deref()
        .expect("button should have ref");
    let entry = refmap.get(btn_ref).expect("entry should exist");
    assert_eq!(entry.root_ref.as_deref(), Some("@e5"));
    assert_eq!(entry.pid, 42);
    assert_eq!(entry.source_app.as_deref(), Some("TestApp"));
}

#[test]
fn test_drill_alloc_respects_interactive_only() {
    let btn = node("button");
    let text = node("statictext");
    let mut root = node("group");
    root.children = vec![btn, text];

    let mut refmap = RefMap::new();
    let config = drill_config(None, 1, "@e1", true, false);
    let tree = ref_alloc::allocate_refs(root, &mut refmap, &config);

    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].role, "button");
}

#[test]
fn test_drill_alloc_preserves_truncated_child() {
    let mut container = node("group");
    container.name = Some("Sidebar".into());
    container.children_count = Some(4);
    let mut root = node("window");
    root.children = vec![container];

    let mut refmap = RefMap::new();
    let config = drill_config(None, 1, "@e1", true, false);
    let tree = ref_alloc::allocate_refs(root, &mut refmap, &config);

    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].children_count, Some(4));
}

#[test]
fn test_drill_alloc_compact() {
    let mut btn = node("button");
    btn.name = Some("OK".into());
    let mut wrapper = node("group");
    wrapper.children = vec![btn];
    let mut root = node("window");
    root.children = vec![wrapper];

    let mut refmap = RefMap::new();
    let config = drill_config(None, 1, "@e1", false, true);
    let tree = ref_alloc::allocate_refs(root, &mut refmap, &config);

    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].role, "button");
}
