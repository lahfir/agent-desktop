use crate::{
    adapter::{PlatformAdapter, TreeOptions, WindowFilter},
    error::AppError,
    node::{AccessibilityNode, WindowInfo},
    refs::{RefEntry, RefMap},
};

const INTERACTIVE_ROLES: &[&str] = &[
    "button", "textfield", "checkbox", "link", "menuitem", "tab", "slider",
    "combobox", "treeitem", "cell", "radiobutton", "incrementor",
    "menubutton", "switch", "colorwell", "dockitem",
];

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
            AppError::Adapter(crate::error::AdapterError::new(
                crate::error::ErrorCode::WindowNotFound,
                format!("No window with id {wid}"),
            ))
        })?
    } else if let Some(app) = app_name {
        windows
            .into_iter()
            .find(|w| w.app.eq_ignore_ascii_case(app) && w.is_focused)
            .or_else(|| {
                adapter
                    .list_windows(&WindowFilter { focused_only: false, app: Some(app.to_string()) })
                    .ok()
                    .and_then(|ws| ws.into_iter().next())
            })
            .ok_or_else(|| {
                AppError::Adapter(crate::error::AdapterError::new(
                    crate::error::ErrorCode::AppNotFound,
                    format!("No window found for app '{app}'"),
                ))
            })?
    } else {
        windows
            .into_iter()
            .find(|w| w.is_focused)
            .ok_or_else(|| {
                AppError::Adapter(crate::error::AdapterError::new(
                    crate::error::ErrorCode::WindowNotFound,
                    "No focused window found. Use --app to specify an application.",
                ))
            })?
    };

    let tree_opts = TreeOptions {
        max_depth: opts.max_depth,
        include_bounds: opts.include_bounds,
        interactive_only: opts.interactive_only,
        compact: opts.compact,
    };

    let raw_tree = adapter.get_tree(&window, &tree_opts)?;

    let mut refmap = RefMap::new();
    let tree = allocate_refs(
        raw_tree,
        &mut refmap,
        opts.include_bounds,
        opts.interactive_only,
        window.pid,
        Some(window.app.as_str()),
    );

    Ok(SnapshotResult { tree, refmap, window })
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

fn allocate_refs(
    mut node: AccessibilityNode,
    refmap: &mut RefMap,
    include_bounds: bool,
    interactive_only: bool,
    window_pid: i32,
    source_app: Option<&str>,
) -> AccessibilityNode {
    let is_interactive = INTERACTIVE_ROLES.contains(&node.role.as_str());

    if is_interactive {
        let entry = RefEntry {
            pid: window_pid,
            role: node.role.clone(),
            name: node.name.clone(),
            value: node.value.clone(),
            states: node.states.clone(),
            bounds: node.bounds,
            bounds_hash: node.bounds.as_ref().map(|b| b.bounds_hash()),
            available_actions: actions_for_role(&node.role),
            source_app: source_app.map(str::to_string),
        };
        node.ref_id = Some(refmap.allocate(entry));
    }

    if !include_bounds {
        node.bounds = None;
    }

    node.children = node
        .children
        .into_iter()
        .filter_map(|child| {
            let child = allocate_refs(child, refmap, include_bounds, interactive_only, window_pid, source_app);
            if interactive_only && child.ref_id.is_none() && child.children.is_empty() {
                None
            } else {
                Some(child)
            }
        })
        .collect();

    node
}

fn actions_for_role(role: &str) -> Vec<String> {
    match role {
        "button" | "link" | "menuitem" | "tab" | "radiobutton" => vec!["Click".into()],
        "textfield" | "incrementor" => vec!["Click".into(), "SetValue".into(), "SetFocus".into()],
        "checkbox" => vec!["Click".into(), "Toggle".into()],
        "combobox" => vec!["Click".into(), "Select".into()],
        "treeitem" => vec!["Click".into(), "Expand".into(), "Collapse".into()],
        "slider" => vec!["SetValue".into()],
        "cell" => vec!["Click".into()],
        _ => vec!["Click".into()],
    }
}
