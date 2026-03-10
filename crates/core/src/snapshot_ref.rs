use crate::{
    adapter::{PlatformAdapter, TreeOptions},
    error::AppError,
    node::{AccessibilityNode, WindowInfo},
    ref_alloc::{is_collapsible, ref_entry_from_node, INTERACTIVE_ROLES},
    refs::RefMap,
    snapshot::SnapshotResult,
};

struct DrillDownConfig<'a> {
    include_bounds: bool,
    interactive_only: bool,
    compact: bool,
    pid: i32,
    source_app: Option<&'a str>,
    root_ref_id: &'a str,
}

pub fn run_from_ref(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    root_ref_id: &str,
) -> Result<SnapshotResult, AppError> {
    let mut refmap = RefMap::load()?;

    let entry = refmap
        .get(root_ref_id)
        .ok_or_else(|| AppError::stale_ref(root_ref_id))?
        .clone();

    let handle = adapter.resolve_element(&entry)?;

    let raw_tree = adapter.get_subtree(&handle, opts)?;

    refmap.remove_by_root_ref(root_ref_id);

    let config = DrillDownConfig {
        include_bounds: opts.include_bounds,
        interactive_only: opts.interactive_only,
        compact: opts.compact,
        pid: entry.pid,
        source_app: entry.source_app.as_deref(),
        root_ref_id,
    };

    let mut tree = allocate_refs_with_root(raw_tree, &mut refmap, &config);

    crate::hints::add_structural_hints(&mut tree);

    refmap.save()?;

    let window = WindowInfo {
        id: String::new(),
        title: format!("subtree from {root_ref_id}"),
        app: entry.source_app.unwrap_or_default(),
        pid: entry.pid,
        bounds: None,
        is_focused: true,
    };

    Ok(SnapshotResult {
        tree,
        refmap,
        window,
    })
}

fn allocate_refs_with_root(
    mut node: AccessibilityNode,
    refmap: &mut RefMap,
    config: &DrillDownConfig,
) -> AccessibilityNode {
    let is_interactive = INTERACTIVE_ROLES.contains(&node.role.as_str());

    if is_interactive {
        let entry = ref_entry_from_node(
            &node,
            config.pid,
            config.source_app,
            Some(config.root_ref_id.to_string()),
        );
        node.ref_id = Some(refmap.allocate(entry));
    }

    if !config.include_bounds {
        node.bounds = None;
    }

    node.children = node
        .children
        .into_iter()
        .filter_map(|child| {
            let child = allocate_refs_with_root(child, refmap, config);
            if config.compact && is_collapsible(&child) {
                return child.children.into_iter().next();
            }
            if config.interactive_only && child.ref_id.is_none() && child.children.is_empty() {
                None
            } else {
                Some(child)
            }
        })
        .collect();

    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::AccessibilityNode;

    fn node(role: &str) -> AccessibilityNode {
        AccessibilityNode {
            ref_id: None,
            role: role.into(),
            name: None,
            value: None,
            description: None,
            hint: None,
            states: vec![],
            bounds: None,
            children_count: None,
            children: vec![],
        }
    }

    #[test]
    fn test_allocate_refs_with_root_tags_entries() {
        let mut btn = node("button");
        btn.name = Some("Submit".into());
        let mut root = node("group");
        root.children = vec![btn];

        let mut refmap = RefMap::new();
        let config = DrillDownConfig {
            include_bounds: false,
            interactive_only: false,
            compact: false,
            pid: 42,
            source_app: Some("TestApp"),
            root_ref_id: "@e5",
        };
        let tree = allocate_refs_with_root(root, &mut refmap, &config);

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
    fn test_allocate_refs_with_root_respects_interactive_only() {
        let btn = node("button");
        let text = node("statictext");
        let mut root = node("group");
        root.children = vec![btn, text];

        let mut refmap = RefMap::new();
        let config = DrillDownConfig {
            include_bounds: false,
            interactive_only: true,
            compact: false,
            pid: 1,
            source_app: None,
            root_ref_id: "@e1",
        };
        let tree = allocate_refs_with_root(root, &mut refmap, &config);

        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].role, "button");
    }

    #[test]
    fn test_allocate_refs_with_root_compact() {
        let mut btn = node("button");
        btn.name = Some("OK".into());
        let mut wrapper = node("group");
        wrapper.children = vec![btn];
        let mut root = node("window");
        root.children = vec![wrapper];

        let mut refmap = RefMap::new();
        let config = DrillDownConfig {
            include_bounds: false,
            interactive_only: false,
            compact: true,
            pid: 1,
            source_app: None,
            root_ref_id: "@e1",
        };
        let tree = allocate_refs_with_root(root, &mut refmap, &config);

        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].role, "button");
    }
}
