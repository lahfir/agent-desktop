use super::AXElement;
use super::capabilities::{copy_action_names, is_attr_settable};
use agent_desktop_core::capability;

#[cfg(target_os = "macos")]
use accessibility_sys::{kAXFocusedAttribute, kAXValueAttribute};

#[cfg(target_os = "macos")]
pub(crate) fn platform_available_actions(
    el: &AXElement,
    role: &str,
    has_scrollbars: bool,
) -> Vec<String> {
    let ax_actions = copy_action_names(el);
    let has = |name: &str| ax_actions.iter().any(|a| a == name);
    let mut actions = Vec::new();

    if has("AXPress") {
        push_unique(&mut actions, capability::CLICK);
        if crate::tree::roles::is_toggleable_role(role) {
            push_unique(&mut actions, capability::TOGGLE);
        }
        if matches!(role, "combobox" | "menuitem" | "tab") {
            push_unique(&mut actions, capability::SELECT);
        }
    }
    if has("AXShowMenu") && role_allows_context_menu_action(role) {
        push_unique(&mut actions, capability::RIGHT_CLICK);
    }
    if has("AXScrollToVisible") {
        push_unique(&mut actions, capability::SCROLL);
        push_unique(&mut actions, capability::SCROLL_TO);
    }
    if has_scroll_mechanism(role, &has, has_scrollbars) {
        push_unique(&mut actions, capability::SCROLL);
    }
    if has("AXIncrement")
        || has("AXDecrement")
        || (role_may_bear_value(role) && is_attr_settable(el, kAXValueAttribute))
    {
        push_unique(&mut actions, capability::SET_VALUE);
    }
    if role_may_accept_focus(role) && is_attr_settable(el, kAXFocusedAttribute) {
        push_unique(&mut actions, capability::SET_FOCUS);
    }
    if (role_may_expand(role) && is_attr_settable(el, "AXExpanded"))
        || (has("AXPress") && agent_desktop_core::roles::is_expandable_role(role))
    {
        push_unique(&mut actions, capability::EXPAND);
        push_unique(&mut actions, capability::COLLAPSE);
    }

    actions
}

/// Whether a role could carry a settable `AXValue`, so the `is_settable` probe
/// is worth an IPC. Click/navigation-only roles never do; `unknown` always
/// probes so an unmapped role never loses a capability.
fn role_may_bear_value(role: &str) -> bool {
    matches!(
        role,
        "textfield"
            | "combobox"
            | "slider"
            | "incrementor"
            | "stepper"
            | "spinbutton"
            | "checkbox"
            | "radiobutton"
            | "switch"
            | "colorwell"
            | "scrollbar"
            | "valueindicator"
            | "unknown"
    )
}

/// Whether a role could carry a settable `AXFocused`, so the `is_settable`
/// probe is worth an IPC. Interactive controls and focus-holding containers
/// (tables, outlines, web areas) can; static/decorative roles never do.
/// `unknown` always probes so an unmapped role never loses a capability.
fn role_may_accept_focus(role: &str) -> bool {
    agent_desktop_core::roles::is_interactive_role(role)
        || matches!(
            role,
            "table"
                | "outline"
                | "list"
                | "browser"
                | "webarea"
                | "scrollarea"
                | "group"
                | "row"
                | "unknown"
        )
}

/// Whether a role could expose a settable `AXExpanded`. Leaf/interactive roles
/// never expand; `unknown` always probes.
fn role_may_expand(role: &str) -> bool {
    agent_desktop_core::roles::is_expandable_role(role)
        || matches!(
            role,
            "group" | "outline" | "row" | "browser" | "table" | "list" | "cell" | "unknown"
        )
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn platform_available_actions(
    _el: &AXElement,
    _role: &str,
    _has_scrollbars: bool,
) -> Vec<String> {
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

fn has_scroll_mechanism(role: &str, has: &impl Fn(&str) -> bool, has_scrollbars: bool) -> bool {
    role_supports_scroll(role)
        || has("AXScrollDownByPage")
        || has("AXScrollUpByPage")
        || has("AXScrollLeftByPage")
        || has("AXScrollRightByPage")
        || (role_may_own_scrollbars(role) && has_scrollbars)
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
    use super::{
        role_allows_context_menu_action, role_may_accept_focus, role_may_own_scrollbars,
        role_supports_scroll,
    };

    #[test]
    fn focus_probe_is_limited_to_focus_bearing_roles() {
        assert!(role_may_accept_focus("textfield"));
        assert!(role_may_accept_focus("webarea"));
        assert!(role_may_accept_focus("unknown"));
        assert!(!role_may_accept_focus("statictext"));
        assert!(!role_may_accept_focus("image"));
    }

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
