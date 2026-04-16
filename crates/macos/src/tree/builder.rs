use agent_desktop_core::node::AccessibilityNode;
use rustc_hash::FxHashSet;

use super::element::{
    child_attributes, copy_ax_array, copy_string_attr, count_children, element_for_pid,
    fetch_node_attrs, read_bounds, AXElement, ABSOLUTE_MAX_DEPTH,
};

#[cfg(target_os = "macos")]
use accessibility_sys::{
    kAXChildrenAttribute, kAXRoleAttribute, kAXTitleAttribute, kAXValueAttribute,
    kAXWindowsAttribute,
};

#[cfg(target_os = "macos")]
pub fn window_element_for(pid: i32, win_title: &str) -> AXElement {
    let app = element_for_pid(pid);

    if let Some(windows) = copy_ax_array(&app, kAXWindowsAttribute) {
        for win in &windows {
            let title = copy_string_attr(win, kAXTitleAttribute);
            if title.as_deref() == Some(win_title) {
                return win.clone();
            }
        }
        for win in &windows {
            let title = copy_string_attr(win, kAXTitleAttribute);
            if title
                .as_deref()
                .is_some_and(|t| t.contains(win_title) || win_title.contains(t))
            {
                return win.clone();
            }
        }
        if let Some(first) = windows.into_iter().next() {
            return first;
        }
    }

    app
}

#[cfg(target_os = "macos")]
pub fn build_subtree(
    el: &AXElement,
    depth: u8,
    raw_depth: u8,
    max_depth: u8,
    ancestors: &mut FxHashSet<usize>,
    skeleton: bool,
) -> Option<AccessibilityNode> {
    if depth > max_depth {
        return None;
    }
    if raw_depth >= ABSOLUTE_MAX_DEPTH {
        let (ax_role, title, ax_desc, value, _, _) = fetch_node_attrs(el);
        let role = ax_role
            .as_deref()
            .map(crate::tree::roles::ax_role_to_str)
            .unwrap_or("unknown")
            .to_string();
        let name = title.or(ax_desc);
        let child_count = count_children(el, ax_role.as_deref());
        let bounds = read_bounds(el);
        return Some(AccessibilityNode {
            ref_id: None,
            role,
            name,
            value,
            description: None,
            hint: None,
            states: vec![],
            bounds,
            children_count: if child_count > 0 {
                Some(child_count)
            } else {
                None
            },
            children: vec![],
        });
    }
    let ptr_key = el.0 as usize;
    if !ancestors.insert(ptr_key) {
        return None;
    }

    let (ax_role, title, ax_desc, value, enabled, focused) = fetch_node_attrs(el);

    let role = ax_role
        .as_deref()
        .map(crate::tree::roles::ax_role_to_str)
        .unwrap_or("unknown")
        .to_string();

    let name = title.clone().or_else(|| ax_desc.clone());
    let description = if title.is_some() { ax_desc } else { None };

    let name = if name.is_none() && ax_role.as_deref() == Some("AXStaticText") {
        value.clone().or(name)
    } else {
        name
    };

    let mut states = Vec::new();
    if focused {
        states.push("focused".into());
    }
    if !enabled {
        states.push("disabled".into());
    }

    let bounds = read_bounds(el);

    let is_web_wrapper = matches!(
        ax_role.as_deref(),
        Some("AXGroup") | Some("AXGenericElement")
    ) && title.as_deref().is_none_or(str::is_empty)
        && value.as_deref().is_none_or(str::is_empty);

    // Web wrappers do not consume a logical depth slot so that Electron/Chromium
    // structural layers (AXGroup/AXGenericElement with no label) are transparent to
    // agents. A chain of wrappers only stops at ABSOLUTE_MAX_DEPTH, not max_depth.
    // This is intentional: skeleton depth tracks semantic content depth, not raw DOM depth.
    let child_depth = if is_web_wrapper { depth } else { depth + 1 };
    let child_raw_depth = raw_depth + 1;

    let at_skeleton_boundary =
        skeleton && (child_depth > max_depth || child_raw_depth >= ABSOLUTE_MAX_DEPTH);

    if at_skeleton_boundary {
        let child_count = count_children(el, ax_role.as_deref());
        let children_count = if child_count > 0 {
            Some(child_count)
        } else {
            None
        };
        let name = name.or_else(|| {
            let children_raw = copy_children(el, ax_role.as_deref()).unwrap_or_default();
            label_from_children(&children_raw)
        });
        ancestors.remove(&ptr_key);
        return Some(AccessibilityNode {
            ref_id: None,
            role,
            name,
            value,
            description,
            hint: None,
            states,
            bounds,
            children_count,
            children: vec![],
        });
    }

    let children_raw = copy_children(el, ax_role.as_deref()).unwrap_or_default();
    let name = name.or_else(|| label_from_children(&children_raw));

    let children = children_raw
        .into_iter()
        .filter_map(|child| {
            build_subtree(
                &child,
                child_depth,
                child_raw_depth,
                max_depth,
                ancestors,
                skeleton,
            )
        })
        .collect();

    ancestors.remove(&ptr_key);

    Some(AccessibilityNode {
        ref_id: None,
        role,
        name,
        value,
        description,
        hint: None,
        states,
        bounds,
        children_count: None,
        children,
    })
}

pub fn label_from_children(children: &[AXElement]) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        fn text_of(el: &AXElement) -> Option<String> {
            copy_string_attr(el, kAXValueAttribute)
                .or_else(|| copy_string_attr(el, kAXTitleAttribute))
                .filter(|s| !s.is_empty())
        }

        for child in children.iter().take(5) {
            match copy_string_attr(child, kAXRoleAttribute).as_deref() {
                Some("AXStaticText") => {
                    if let Some(s) = text_of(child) {
                        return Some(s);
                    }
                }
                Some("AXCell") | Some("AXGroup") => {
                    for gc in copy_ax_array(child, kAXChildrenAttribute).unwrap_or_default() {
                        if copy_string_attr(&gc, kAXRoleAttribute).as_deref()
                            == Some("AXStaticText")
                        {
                            if let Some(s) = text_of(&gc) {
                                return Some(s);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = children;
        None
    }
}

#[cfg(target_os = "macos")]
fn copy_children(el: &AXElement, ax_role: Option<&str>) -> Option<Vec<AXElement>> {
    for attr in child_attributes(ax_role) {
        if let Some(v) = copy_ax_array(el, attr) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
pub fn window_element_for(_pid: i32, _win_title: &str) -> AXElement {
    element_for_pid(0)
}

#[cfg(not(target_os = "macos"))]
pub fn build_subtree(
    _el: &AXElement,
    _depth: u8,
    _raw_depth: u8,
    _max_depth: u8,
    _visited: &mut FxHashSet<usize>,
    _skeleton: bool,
) -> Option<AccessibilityNode> {
    None
}

#[cfg(test)]
mod tests {
    use super::child_attributes;

    #[test]
    fn test_browser_children_use_columns() {
        assert_eq!(
            child_attributes(Some("AXBrowser")),
            ["AXColumns", "AXContents"]
        );
    }

    #[test]
    fn test_default_children_follow_fallback_order() {
        assert_eq!(
            child_attributes(Some("AXGroup")),
            ["AXChildren", "AXContents", "AXChildrenInNavigationOrder"]
        );
    }
}
