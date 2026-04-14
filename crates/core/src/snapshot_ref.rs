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
            if config.interactive_only
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
    use crate::action::Action;
    use crate::adapter::{NativeHandle, PermissionStatus, PlatformAdapter};
    use crate::error::AdapterError;
    use crate::node::AccessibilityNode;
    use crate::refs::HomeGuard;
    use std::cell::Cell;

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

    fn named(role: &str, name: &str) -> AccessibilityNode {
        let mut n = node(role);
        n.name = Some(name.into());
        n
    }

    struct StubAdapter {
        subtree: AccessibilityNode,
        resolve_calls: Cell<u32>,
    }

    impl StubAdapter {
        fn new(subtree: AccessibilityNode) -> Self {
            Self {
                subtree,
                resolve_calls: Cell::new(0),
            }
        }
    }

    unsafe impl Send for StubAdapter {}
    unsafe impl Sync for StubAdapter {}

    impl PlatformAdapter for StubAdapter {
        fn check_permissions(&self) -> PermissionStatus {
            PermissionStatus::Granted
        }

        fn resolve_element(
            &self,
            _entry: &crate::refs::RefEntry,
        ) -> Result<NativeHandle, AdapterError> {
            self.resolve_calls.set(self.resolve_calls.get() + 1);
            Ok(NativeHandle::null())
        }

        fn get_subtree(
            &self,
            _handle: &NativeHandle,
            _opts: &TreeOptions,
        ) -> Result<AccessibilityNode, AdapterError> {
            Ok(self.subtree.clone())
        }

        fn execute_action(
            &self,
            _handle: &NativeHandle,
            _action: Action,
        ) -> Result<crate::action::ActionResult, AdapterError> {
            Err(AdapterError::not_supported("execute_action"))
        }
    }

    fn seed_skeleton_refmap() -> RefMap {
        let mut map = RefMap::new();
        let anchor = ref_entry_from_node(&named("group", "Sidebar"), 42, Some("TestApp"), None);
        let _ = map.allocate(anchor);
        let other = ref_entry_from_node(&named("button", "Toolbar"), 42, Some("TestApp"), None);
        let _ = map.allocate(other);
        map
    }

    fn drill_opts() -> TreeOptions {
        TreeOptions {
            interactive_only: false,
            ..Default::default()
        }
    }

    #[test]
    fn test_run_from_ref_returns_subtree_and_persists_refs() {
        let _guard = HomeGuard::new();
        seed_skeleton_refmap().save().unwrap();

        let mut child_btn = named("button", "Save");
        child_btn.children = vec![];
        let mut subtree_root = named("group", "Sidebar");
        subtree_root.children = vec![child_btn];

        let adapter = StubAdapter::new(subtree_root);
        let result = run_from_ref(&adapter, &drill_opts(), "@e1").expect("drill should succeed");

        let on_disk = RefMap::load().unwrap();
        assert_eq!(on_disk.len(), result.refmap.len());
        assert!(
            result.refmap.len() >= 3,
            "expected at least 2 skeleton + 1 drill ref, got {}",
            result.refmap.len()
        );

        let drill_ref = result
            .tree
            .children
            .iter()
            .find(|c| c.role == "button")
            .and_then(|c| c.ref_id.as_deref())
            .expect("button child should carry a ref");
        let drill_entry = on_disk.get(drill_ref).expect("entry persisted");
        assert_eq!(drill_entry.root_ref.as_deref(), Some("@e1"));
        assert_eq!(adapter.resolve_calls.get(), 1);
    }

    #[test]
    fn test_run_from_ref_stale_root_returns_stale_ref() {
        let _guard = HomeGuard::new();
        RefMap::new().save().unwrap();

        let adapter = StubAdapter::new(named("group", "Sidebar"));
        let result = run_from_ref(&adapter, &drill_opts(), "@e99");
        let err = match result {
            Ok(_) => panic!("stale root must error"),
            Err(e) => e,
        };
        match err {
            AppError::Adapter(adapter_err) => {
                assert_eq!(adapter_err.code, crate::error::ErrorCode::StaleRef);
                let suggestion = adapter_err.suggestion.as_deref().unwrap_or("");
                assert!(
                    suggestion.contains("skeleton"),
                    "stale-ref suggestion should mention skeleton, got: {suggestion}"
                );
            }
            other => panic!("expected Adapter(StaleRef), got {other:?}"),
        }
    }

    #[test]
    fn test_run_from_ref_re_drill_replaces_drill_refs_only() {
        let _guard = HomeGuard::new();
        seed_skeleton_refmap().save().unwrap();

        let subtree = named("button", "Save");
        let adapter = StubAdapter::new(subtree);

        let first = run_from_ref(&adapter, &drill_opts(), "@e1").unwrap();
        let first_count = first.refmap.len();
        let first_button_ref = first.tree.ref_id.clone().expect("button should get a ref");

        let second = run_from_ref(&adapter, &drill_opts(), "@e1").unwrap();
        let second_count = second.refmap.len();
        let second_button_ref = second.tree.ref_id.clone().expect("button should get a ref");

        assert_eq!(
            first_count, second_count,
            "ref count stable across re-drill"
        );
        assert_ne!(
            first_button_ref, second_button_ref,
            "re-drill should issue a fresh ref id (counter continues)"
        );
        let on_disk = RefMap::load().unwrap();
        assert!(on_disk.get("@e1").is_some(), "skeleton anchor preserved");
        assert!(on_disk.get(&second_button_ref).is_some());
        assert!(
            on_disk.get(&first_button_ref).is_none(),
            "first drill ref must be invalidated by remove_by_root_ref"
        );
    }

    #[test]
    fn test_run_from_ref_multiple_drill_downs_accumulate() {
        let _guard = HomeGuard::new();
        seed_skeleton_refmap().save().unwrap();

        let adapter_one = StubAdapter::new(named("button", "FromE1"));
        let first = run_from_ref(&adapter_one, &drill_opts(), "@e1").unwrap();
        let from_e1_ref = first.tree.ref_id.clone().expect("first drill ref");

        let adapter_two = StubAdapter::new(named("button", "FromE2"));
        let second = run_from_ref(&adapter_two, &drill_opts(), "@e2").unwrap();
        let from_e2_ref = second.tree.ref_id.clone().expect("second drill ref");

        let on_disk = RefMap::load().unwrap();
        assert!(on_disk.get("@e1").is_some(), "skeleton @e1 preserved");
        assert!(on_disk.get("@e2").is_some(), "skeleton @e2 preserved");
        let entry_one = on_disk.get(&from_e1_ref).expect("@e1 drill survives");
        assert_eq!(entry_one.root_ref.as_deref(), Some("@e1"));
        let entry_two = on_disk.get(&from_e2_ref).expect("@e2 drill survives");
        assert_eq!(entry_two.root_ref.as_deref(), Some("@e2"));
    }

    #[test]
    fn test_run_from_ref_empty_subtree() {
        let _guard = HomeGuard::new();
        seed_skeleton_refmap().save().unwrap();

        let adapter = StubAdapter::new(node("group"));
        let result = run_from_ref(&adapter, &drill_opts(), "@e1").unwrap();

        assert!(result.tree.children.is_empty());
        assert_eq!(
            result.refmap.len(),
            2,
            "no new refs added for empty subtree"
        );
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
    fn test_allocate_refs_with_root_preserves_truncated_child() {
        let mut container = node("group");
        container.name = Some("Sidebar".into());
        container.children_count = Some(4);
        let mut root = node("window");
        root.children = vec![container];

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
        assert_eq!(tree.children[0].children_count, Some(4));
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
