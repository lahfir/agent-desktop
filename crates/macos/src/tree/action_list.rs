use super::AXElement;
use super::{
    capabilities::{copy_action_names, is_attr_settable},
    copy_element_attr,
};

#[cfg(target_os = "macos")]
use accessibility_sys::{kAXFocusedAttribute, kAXValueAttribute};

#[cfg(target_os = "macos")]
pub(crate) fn platform_available_actions(el: &AXElement, role: &str) -> Vec<String> {
    let ax_actions = copy_action_names(el);
    let has = |name: &str| ax_actions.iter().any(|a| a == name);
    let mut actions = Vec::new();

    if has("AXPress") {
        push_unique(&mut actions, "Click");
        if crate::tree::roles::is_toggleable_role(role) {
            push_unique(&mut actions, "Toggle");
        }
        if matches!(role, "combobox" | "menuitem" | "tab") {
            push_unique(&mut actions, "Select");
        }
    }
    if has("AXShowMenu") && role_allows_context_menu_action(role) {
        push_unique(&mut actions, "RightClick");
    }
    if has("AXScrollToVisible") {
        push_unique(&mut actions, "Scroll");
        push_unique(&mut actions, "ScrollTo");
    }
    if has_scroll_mechanism(el, role, &has) {
        push_unique(&mut actions, "Scroll");
    }
    if has("AXIncrement") || has("AXDecrement") || is_attr_settable(el, kAXValueAttribute) {
        push_unique(&mut actions, "SetValue");
    }
    if is_attr_settable(el, kAXFocusedAttribute) {
        push_unique(&mut actions, "SetFocus");
    }
    if is_attr_settable(el, "AXExpanded") {
        push_unique(&mut actions, "Expand");
        push_unique(&mut actions, "Collapse");
    }

    actions
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn platform_available_actions(_el: &AXElement, _role: &str) -> Vec<String> {
    Vec::new()
}

fn push_unique(actions: &mut Vec<String>, action: &str) {
    if !actions.iter().any(|a| a == action) {
        actions.push(action.to_string());
    }
}

fn role_allows_context_menu_action(role: &str) -> bool {
    !matches!(role, "combobox" | "menubutton")
}

fn has_scroll_mechanism(el: &AXElement, role: &str, has: &impl Fn(&str) -> bool) -> bool {
    role_supports_scroll(role)
        || has("AXScrollDownByPage")
        || has("AXScrollUpByPage")
        || has("AXScrollLeftByPage")
        || has("AXScrollRightByPage")
        || (role_may_own_scrollbars(role)
            && (copy_element_attr(el, "AXVerticalScrollBar").is_some()
                || copy_element_attr(el, "AXHorizontalScrollBar").is_some()))
}

fn role_supports_scroll(role: &str) -> bool {
    matches!(
        role,
        "scrollarea" | "browser" | "table" | "outline" | "list"
    )
}

fn role_may_own_scrollbars(role: &str) -> bool {
    matches!(
        role,
        "application"
            | "window"
            | "sheet"
            | "dialog"
            | "group"
            | "splitter"
            | "webarea"
            | "grid"
            | "unknown"
    )
}

#[cfg(test)]
mod tests {
    use super::{role_allows_context_menu_action, role_may_own_scrollbars, role_supports_scroll};

    #[test]
    fn menu_opening_controls_do_not_advertise_right_click() {
        assert!(!role_allows_context_menu_action("combobox"));
        assert!(!role_allows_context_menu_action("menubutton"));
        assert!(role_allows_context_menu_action("textfield"));
        assert!(role_allows_context_menu_action("button"));
    }

    #[test]
    fn scroll_container_roles_advertise_scroll_without_scroll_to() {
        assert!(role_supports_scroll("scrollarea"));
        assert!(role_supports_scroll("browser"));
        assert!(!role_supports_scroll("button"));
    }

    #[test]
    fn scrollbar_probe_is_limited_to_container_like_roles() {
        assert!(role_may_own_scrollbars("group"));
        assert!(role_may_own_scrollbars("webarea"));
        assert!(!role_may_own_scrollbars("button"));
        assert!(!role_may_own_scrollbars("cell"));
    }
}
