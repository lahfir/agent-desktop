use crate::{
    adapter::{PlatformAdapter, SnapshotSurface, TreeOptions, WindowFilter},
    error::AppError,
    node::{AccessibilityNode, WindowInfo},
    ref_alloc::{is_collapsible, ref_entry_from_node, INTERACTIVE_ROLES},
    refs::RefMap,
};

pub struct SnapshotResult {
    pub tree: AccessibilityNode,
    pub refmap: RefMap,
    pub window: WindowInfo,
}

pub fn build(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    app_name: Option<&str>,
    window_id: Option<&str>,
) -> Result<SnapshotResult, AppError> {
    let filter = WindowFilter {
        focused_only: app_name.is_none() && window_id.is_none(),
        app: app_name.map(str::to_string),
    };

    let windows = adapter.list_windows(&filter)?;

    let window = if let Some(wid) = window_id {
        windows.into_iter().find(|w| w.id == wid).ok_or_else(|| {
            AppError::Adapter(
                crate::error::AdapterError::new(
                    crate::error::ErrorCode::WindowNotFound,
                    format!("No window with id {wid}"),
                )
                .with_suggestion("Run 'list-windows' to see available window IDs."),
            )
        })?
    } else if let Some(app) = app_name {
        windows
            .into_iter()
            .find(|w| w.app.eq_ignore_ascii_case(app) && w.is_focused)
            .or_else(|| {
                adapter
                    .list_windows(&WindowFilter {
                        focused_only: false,
                        app: Some(app.to_string()),
                    })
                    .ok()
                    .and_then(|ws| ws.into_iter().next())
            })
            .ok_or_else(|| {
                AppError::Adapter(
                    crate::error::AdapterError::new(
                        crate::error::ErrorCode::AppNotFound,
                        format!("No window found for app '{app}'"),
                    )
                    .with_suggestion(
                        "Verify the app is running. Use 'list-apps' to see running applications.",
                    ),
                )
            })?
    } else {
        windows.into_iter().find(|w| w.is_focused).ok_or_else(|| {
            AppError::Adapter(
                crate::error::AdapterError::new(
                    crate::error::ErrorCode::WindowNotFound,
                    "No focused window found",
                )
                .with_suggestion(
                    "Use --app to specify an application, or click a window to focus it.",
                ),
            )
        })?
    };

    let raw_tree = adapter.get_tree(&window, opts)?;

    let mut refmap = if opts.skeleton {
        let mut loaded = RefMap::load().unwrap_or_default();
        loaded.remove_skeleton_refs();
        loaded
    } else {
        RefMap::new()
    };
    let mut tree = allocate_refs(
        raw_tree,
        &mut refmap,
        opts.include_bounds,
        opts.interactive_only,
        opts.compact,
        window.pid,
        Some(window.app.as_str()),
    );

    crate::hints::add_structural_hints(&mut tree);

    Ok(SnapshotResult {
        tree,
        refmap,
        window,
    })
}

pub fn run(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    app_name: Option<&str>,
    window_id: Option<&str>,
) -> Result<SnapshotResult, AppError> {
    let result = build(adapter, opts, app_name, window_id)?;
    result.refmap.save()?;
    Ok(result)
}

pub fn append_surface_refs(
    adapter: &dyn PlatformAdapter,
    pid: i32,
    source_app: Option<&str>,
    surface: SnapshotSurface,
) -> Option<AccessibilityNode> {
    let filter = WindowFilter {
        focused_only: false,
        app: None,
    };
    let windows = adapter.list_windows(&filter).ok()?;
    let window = windows.into_iter().find(|w| w.pid == pid)?;
    let opts = TreeOptions {
        surface,
        interactive_only: true,
        ..Default::default()
    };
    let raw_tree = adapter.get_tree(&window, &opts).ok()?;
    let mut refmap = RefMap::load().ok()?;
    let tree = allocate_refs(raw_tree, &mut refmap, false, true, false, pid, source_app);
    refmap.save().ok()?;
    Some(tree)
}

fn allocate_refs(
    mut node: AccessibilityNode,
    refmap: &mut RefMap,
    include_bounds: bool,
    interactive_only: bool,
    compact: bool,
    window_pid: i32,
    source_app: Option<&str>,
) -> AccessibilityNode {
    let is_interactive = INTERACTIVE_ROLES.contains(&node.role.as_str());

    if is_interactive {
        let entry = ref_entry_from_node(&node, window_pid, source_app, None);
        node.ref_id = Some(refmap.allocate(entry));
    }

    let has_label = node.name.as_deref().is_some_and(|n| !n.is_empty())
        || node.description.as_deref().is_some_and(|d| !d.is_empty());
    let is_skeleton_anchor = !is_interactive && node.children_count.is_some() && has_label;

    if is_skeleton_anchor {
        let mut entry = ref_entry_from_node(&node, window_pid, source_app, None);
        entry.available_actions = vec![];
        node.ref_id = Some(refmap.allocate(entry));
    }

    if !include_bounds {
        node.bounds = None;
    }

    node.children = node
        .children
        .into_iter()
        .filter_map(|child| {
            let child = allocate_refs(
                child,
                refmap,
                include_bounds,
                interactive_only,
                compact,
                window_pid,
                source_app,
            );
            if compact && is_collapsible(&child) {
                return child.children.into_iter().next();
            }
            if interactive_only
                && child.ref_id.is_none()
                && child.children.is_empty()
                && child.children_count.is_none()
            {
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

    fn run_compact(tree: AccessibilityNode) -> AccessibilityNode {
        let mut refmap = RefMap::new();
        allocate_refs(tree, &mut refmap, false, false, true, 1, Some("Test"))
    }

    fn run_compact_interactive(tree: AccessibilityNode) -> AccessibilityNode {
        let mut refmap = RefMap::new();
        allocate_refs(tree, &mut refmap, false, true, true, 1, Some("Test"))
    }

    #[test]
    fn test_compact_collapses_single_child_chain() {
        let mut btn = node("button");
        btn.name = Some("Send".into());
        let mut g1 = node("group");
        g1.children = vec![btn];
        let mut g2 = node("group");
        g2.children = vec![g1];
        let mut root = node("window");
        root.children = vec![g2];

        let result = run_compact(root);
        assert_eq!(result.role, "window");
        assert_eq!(result.children.len(), 1);
        assert_eq!(result.children[0].role, "button");
        assert_eq!(result.children[0].name.as_deref(), Some("Send"));
    }

    #[test]
    fn test_compact_preserves_named_containers() {
        let btn = node("button");
        let mut named = node("group");
        named.name = Some("Sidebar".into());
        named.children = vec![btn];
        let mut root = node("window");
        root.children = vec![named];

        let result = run_compact(root);
        assert_eq!(result.children.len(), 1);
        assert_eq!(result.children[0].role, "group");
        assert_eq!(result.children[0].name.as_deref(), Some("Sidebar"));
    }

    #[test]
    fn test_compact_preserves_description() {
        let btn = node("button");
        let mut desc_node = node("group");
        desc_node.description = Some("toolbar".into());
        desc_node.children = vec![btn];
        let mut root = node("window");
        root.children = vec![desc_node];

        let result = run_compact(root);
        assert_eq!(result.children.len(), 1);
        assert_eq!(result.children[0].role, "group");
        assert_eq!(result.children[0].description.as_deref(), Some("toolbar"));
    }

    #[test]
    fn test_compact_preserves_states() {
        let btn = node("button");
        let mut disabled = node("group");
        disabled.states = vec!["disabled".into()];
        disabled.children = vec![btn];
        let mut root = node("window");
        root.children = vec![disabled];

        let result = run_compact(root);
        assert_eq!(result.children.len(), 1);
        assert_eq!(result.children[0].role, "group");
        assert_eq!(result.children[0].states, vec!["disabled"]);
    }

    #[test]
    fn test_compact_preserves_multi_child() {
        let btn = node("button");
        let tf = node("textfield");
        let mut group = node("group");
        group.children = vec![btn, tf];
        let mut root = node("window");
        root.children = vec![group];

        let result = run_compact(root);
        assert_eq!(result.children.len(), 1);
        assert_eq!(result.children[0].role, "group");
        assert_eq!(result.children[0].children.len(), 2);
    }

    #[test]
    fn test_compact_with_interactive_only() {
        let mut btn = node("button");
        btn.name = Some("OK".into());
        let text = node("statictext");
        let mut g1 = node("group");
        g1.children = vec![btn];
        let mut g2 = node("group");
        g2.children = vec![text];
        let mut root = node("window");
        root.children = vec![g1, g2];

        let result = run_compact_interactive(root);
        assert_eq!(result.children.len(), 1);
        assert_eq!(result.children[0].role, "button");
        assert!(result.children[0].ref_id.is_some());
    }

    #[test]
    fn test_skeleton_named_container_gets_ref() {
        let mut container = node("group");
        container.name = Some("Sidebar".into());
        container.children_count = Some(5);
        let mut root = node("window");
        root.children = vec![container];

        let mut refmap = RefMap::new();
        let result = allocate_refs(root, &mut refmap, false, false, false, 1, Some("Test"));

        assert!(result.children[0].ref_id.is_some());
        assert_eq!(refmap.len(), 1);
        let entry = refmap
            .get(result.children[0].ref_id.as_deref().unwrap())
            .unwrap();
        assert!(entry.available_actions.is_empty());
    }

    #[test]
    fn test_skeleton_unnamed_container_no_ref() {
        let mut container = node("group");
        container.children_count = Some(5);
        let mut root = node("window");
        root.children = vec![container];

        let mut refmap = RefMap::new();
        let result = allocate_refs(root, &mut refmap, false, false, false, 1, Some("Test"));

        assert!(result.children[0].ref_id.is_none());
        assert_eq!(refmap.len(), 0);
    }

    #[test]
    fn test_skeleton_described_container_gets_ref() {
        let mut container = node("group");
        container.description = Some("Channels and direct messages".into());
        container.children_count = Some(12);
        let mut root = node("window");
        root.children = vec![container];

        let mut refmap = RefMap::new();
        let result = allocate_refs(root, &mut refmap, false, false, false, 1, Some("Test"));

        assert!(result.children[0].ref_id.is_some());
        assert_eq!(refmap.len(), 1);
    }

    #[test]
    fn test_skeleton_truncated_node_survives_interactive_only() {
        let mut container = node("group");
        container.name = Some("Content".into());
        container.children_count = Some(10);
        let mut root = node("window");
        root.children = vec![container];

        let mut refmap = RefMap::new();
        let result = allocate_refs(root, &mut refmap, false, true, false, 1, Some("Test"));

        assert_eq!(result.children.len(), 1);
        assert_eq!(result.children[0].children_count, Some(10));
    }
}
