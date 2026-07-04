use agent_desktop_core::capability;
use agent_desktop_core::node::AccessibilityNode;
use rustc_hash::FxHashSet;

use super::AXElement;
use super::action_list::platform_available_actions;
use super::attributes::{copy_ax_array, copy_ax_array_prefix, copy_string_attr};
use super::build_context::TreeBuildContext;
use super::element::{
    ABSOLUTE_MAX_DEPTH, child_attributes, count_children, element_for_pid, fetch_node_attrs,
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
        let mut first_candidate = None;
        let mut child_candidate = None;
        let mut partial_candidate = None;
        for win in &windows {
            if copy_string_attr(win, kAXRoleAttribute).as_deref() != Some("AXWindow") {
                continue;
            }
            first_candidate.get_or_insert_with(|| win.clone());
            let title = copy_string_attr(win, kAXTitleAttribute);
            if title
                .as_deref()
                .is_some_and(|title| window_titles_are_exact_match(title, win_title))
            {
                return win.clone();
            }
            if title
                .as_deref()
                .is_some_and(|title| window_titles_are_partial_match(title, win_title))
            {
                partial_candidate.get_or_insert_with(|| win.clone());
            }
            if child_candidate.is_none() && count_children(win, None) > 0 {
                child_candidate = Some(win.clone());
            }
        }
        if let Some(candidate) = partial_candidate.or(child_candidate).or(first_candidate) {
            return candidate;
        }
    }

    app
}

#[cfg(target_os = "macos")]
fn window_titles_are_exact_match(candidate_title: &str, requested_title: &str) -> bool {
    !candidate_title.is_empty() && !requested_title.is_empty() && candidate_title == requested_title
}

#[cfg(target_os = "macos")]
fn window_titles_are_partial_match(candidate_title: &str, requested_title: &str) -> bool {
    !candidate_title.is_empty()
        && !requested_title.is_empty()
        && (candidate_title.contains(requested_title) || requested_title.contains(candidate_title))
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
        if ancestors.contains(&(el.0 as usize)) {
            return None;
        }
        let attrs = fetch_node_attrs(el);
        let role = attrs
            .role
            .as_deref()
            .map(crate::tree::roles::ax_role_to_str)
            .unwrap_or("unknown")
            .to_string();
        let is_secure_text = is_secure_text_role(attrs.role.as_deref());
        let name = accessible_name(el, &attrs);
        let value = redact_secure_value(attrs.role.as_deref(), attrs.value);
        let child_count = count_children(el, attrs.role.as_deref());
        let bounds = context.bounds_for(attrs.bounds);
        let mut states = Vec::new();
        if is_secure_text {
            states.push("secure".into());
        }
        return Some(AccessibilityNode {
            ref_id: None,
            available_actions: platform_available_actions(el, &role, attrs.has_scrollbars),
            name,
            value,
            description: None,
            native_id: None,
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
    let value = redact_secure_value(attrs.role.as_deref(), attrs.value.clone());
    let is_promoted_item = promoted_label.is_some();
    let available_actions = if is_promoted_item {
        vec![capability::CLICK.into(), capability::RIGHT_CLICK.into()]
    } else {
        platform_available_actions(el, &role, attrs.has_scrollbars)
    };

    let name = promoted_label.or_else(|| accessible_name(el, &attrs));
    let description = if attrs.title.is_some() {
        attrs.description.clone()
    } else {
        None
    };

    let state_ctx = super::state_reader::StateReaderContext {
        focused: context.focused.as_ref(),
        window_bounds: context.window_bounds,
        is_secure_text,
    };
    let states = super::state_reader::states_from_element(el, &attrs, &role, &state_ctx);

    let bounds = context.bounds_for(attrs.bounds);

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
        ancestors.remove(&ptr_key);
        return Some(AccessibilityNode {
            ref_id: None,
            role,
            name,
            value,
            description,
            native_id: attrs.native_id.clone(),
            hint: None,
            states,
            available_actions,
            bounds,
            children_count,
            children: vec![],
        });
    }

    let children_raw = copy_children(el, attrs.role.as_deref()).unwrap_or_default();

    let child_window_bounds = if attrs.role.as_deref() == Some("AXWindow") {
        attrs.bounds.or(context.window_bounds)
    } else {
        context.window_bounds
    };
    let child_context = context.child_context(child_window_bounds);

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
                    &child_context,
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
        native_id: attrs.native_id.clone(),
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

/// One-owner accessible-name reduction shared by the snapshot builder (which
/// stores a ref's name) and `element::resolve_element_name` (which recomputes
/// it during strict re-resolution), so a stored name always equals what the
/// resolver recomputes. Precedence: the element's own title, then description,
/// then a static-text value promoted to a name, then a label aggregated from
/// descendant text — each trimmed and treated as absent when blank.
#[cfg(target_os = "macos")]
pub(crate) fn accessible_name(
    el: &AXElement,
    attrs: &super::node_attrs::NodeAttrs,
) -> Option<String> {
    let ax_role = attrs.role.as_deref();
    let static_value = if ax_role == Some("AXStaticText") {
        attrs.value.as_deref()
    } else {
        None
    };
    reduce_text_name(
        attrs.title.as_deref(),
        attrs.description.as_deref(),
        static_value,
    )
    .or_else(|| label_from_children(&copy_children(el, ax_role).unwrap_or_default()))
}

/// The own-text portion of [`accessible_name`] (title -> description ->
/// static-text value), factored out pure and platform-agnostic so the
/// precedence and blank/whitespace handling are unit-testable without a live
/// AX element.
pub(crate) fn reduce_text_name(
    title: Option<&str>,
    description: Option<&str>,
    static_value: Option<&str>,
) -> Option<String> {
    non_empty(title)
        .or_else(|| non_empty(description))
        .or_else(|| non_empty(static_value))
}

fn non_empty(text: Option<&str>) -> Option<String> {
    text.filter(|value| !value.trim().is_empty())
        .map(str::to_string)
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
