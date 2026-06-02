use agent_desktop_core::node::AccessibilityNode;
use rustc_hash::FxHashSet;

use super::AXElement;
use super::action_list::platform_available_actions;
use super::build_context::TreeBuildContext;
use super::capabilities::same_element;
use super::element::{
    ABSOLUTE_MAX_DEPTH, child_attributes, copy_ax_array, copy_ax_array_prefix, copy_bool_attr,
    copy_string_attr, count_children, element_for_pid, fetch_node_attrs,
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
            if !is_window_candidate(win) {
                continue;
            }
            let title = copy_string_attr(win, kAXTitleAttribute);
            if title.as_deref() == Some(win_title) {
                return win.clone();
            }
        }
        for win in &windows {
            if !is_window_candidate(win) {
                continue;
            }
            let title = copy_string_attr(win, kAXTitleAttribute);
            if title
                .as_deref()
                .is_some_and(|t| t.contains(win_title) || win_title.contains(t))
            {
                return win.clone();
            }
        }
        if let Some(first) = windows.into_iter().find(is_window_candidate) {
            return first;
        }
    }

    app
}

#[cfg(target_os = "macos")]
fn is_window_candidate(el: &AXElement) -> bool {
    copy_string_attr(el, kAXRoleAttribute).as_deref() == Some("AXWindow")
}

#[cfg(target_os = "macos")]
pub fn build_subtree(
    el: &AXElement,
    depth: u8,
    raw_depth: u8,
    max_depth: u8,
    ancestors: &mut FxHashSet<usize>,
    skeleton: bool,
    context: &TreeBuildContext,
) -> Option<AccessibilityNode> {
    if depth > max_depth {
        return None;
    }
    if raw_depth >= ABSOLUTE_MAX_DEPTH {
        let attrs = fetch_node_attrs(el);
        let role = attrs
            .role
            .as_deref()
            .map(crate::tree::roles::ax_role_to_str)
            .unwrap_or("unknown")
            .to_string();
        let is_secure_text = is_secure_text_role(attrs.role.as_deref());
        let value = redact_secure_value(attrs.role.as_deref(), attrs.value);
        let name = attrs.title.or(attrs.description);
        let child_count = count_children(el, attrs.role.as_deref());
        let bounds = context.read_bounds(el);
        let mut states = Vec::new();
        if is_secure_text {
            states.push("secure".into());
        }
        return Some(AccessibilityNode {
            ref_id: None,
            available_actions: platform_available_actions(el, &role),
            name,
            value,
            description: None,
            hint: None,
            states,
            role,
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

    let attrs = fetch_node_attrs(el);

    let (role, promoted_label) =
        crate::tree::roles::normalized_role_and_label(el, attrs.role.as_deref());
    let is_secure_text = is_secure_text_role(attrs.role.as_deref());
    let value = redact_secure_value(attrs.role.as_deref(), attrs.value);
    let is_promoted_item = promoted_label.is_some();
    let available_actions = if is_promoted_item {
        vec!["Click".into(), "RightClick".into()]
    } else {
        platform_available_actions(el, &role)
    };

    let name = promoted_label.or_else(|| attrs.title.clone().or_else(|| attrs.description.clone()));
    let description = if attrs.title.is_some() {
        attrs.description
    } else {
        None
    };

    let name = if name.is_none() && attrs.role.as_deref() == Some("AXStaticText") {
        value.clone().or(name)
    } else {
        name
    };

    let mut states = Vec::new();
    if context
        .focused
        .as_ref()
        .is_some_and(|focused| same_element(el, focused))
    {
        states.push("focused".into());
    }
    if !attrs.enabled {
        states.push("disabled".into());
    }
    if is_secure_text {
        states.push("secure".into());
    }
    if element_is_expanded(el) {
        states.push("expanded".into());
    }
    if super::roles::is_toggleable_role(&role) && value_is_checked(value.as_deref()) {
        states.push("checked".into());
    }

    let bounds = context.read_bounds(el);

    let is_web_wrapper = matches!(
        attrs.role.as_deref(),
        Some("AXGroup") | Some("AXGenericElement")
    ) && attrs.title.as_deref().is_none_or(str::is_empty)
        && value.as_deref().is_none_or(str::is_empty);

    let child_depth = if is_web_wrapper { depth } else { depth + 1 };
    let child_raw_depth = raw_depth + 1;

    let at_skeleton_boundary =
        skeleton && (child_depth > max_depth || child_raw_depth >= ABSOLUTE_MAX_DEPTH);

    if at_skeleton_boundary {
        let child_count = count_children(el, attrs.role.as_deref());
        let children_count = if child_count > 0 {
            Some(child_count)
        } else {
            None
        };
        let name = name.or_else(|| label_from_child_attrs(el, attrs.role.as_deref()));
        ancestors.remove(&ptr_key);
        return Some(AccessibilityNode {
            ref_id: None,
            role,
            name,
            value,
            description,
            hint: None,
            states,
            available_actions,
            bounds,
            children_count,
            children: vec![],
        });
    }

    let children_raw = copy_children(el, attrs.role.as_deref()).unwrap_or_default();
    let name = name.or_else(|| label_from_children(&children_raw));

    let children = if is_promoted_item {
        Vec::new()
    } else {
        children_raw
            .into_iter()
            .filter_map(|child| {
                build_subtree(
                    &child,
                    child_depth,
                    child_raw_depth,
                    max_depth,
                    ancestors,
                    skeleton,
                    context,
                )
            })
            .collect()
    };

    ancestors.remove(&ptr_key);

    Some(AccessibilityNode {
        ref_id: None,
        role,
        name,
        value,
        description,
        hint: None,
        states,
        available_actions,
        bounds,
        children_count: None,
        children,
    })
}

fn is_secure_text_role(ax_role: Option<&str>) -> bool {
    ax_role == Some("AXSecureTextField")
}

fn redact_secure_value(ax_role: Option<&str>, value: Option<String>) -> Option<String> {
    if is_secure_text_role(ax_role) {
        None
    } else {
        value
    }
}

fn element_is_expanded(el: &AXElement) -> bool {
    copy_bool_attr(el, "AXExpanded")
        .or_else(|| copy_bool_attr(el, "AXDisclosing"))
        .unwrap_or(false)
}

fn value_is_checked(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true"))
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
                    for gc in
                        copy_ax_array_prefix(child, kAXChildrenAttribute, 5).unwrap_or_default()
                    {
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
fn label_from_child_attrs(el: &AXElement, ax_role: Option<&str>) -> Option<String> {
    for attr in child_attributes(ax_role) {
        let children = copy_ax_array_prefix(el, attr, 5).unwrap_or_default();
        if let Some(label) = label_from_children(&children) {
            return Some(label);
        }
    }
    None
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
    _context: &TreeBuildContext,
) -> Option<AccessibilityNode> {
    None
}

#[cfg(test)]
#[path = "builder_tests.rs"]
mod tests;
