#[cfg(target_os = "macos")]
mod imp {
    use crate::actions::ax_helpers;
    use crate::tree::AXElement;
    use agent_desktop_core::error::AdapterError;

    pub(crate) fn show_menu(el: &AXElement) -> Result<bool, AdapterError> {
        show_menu_on_element(el)
    }

    pub(crate) fn show_menu_on_ancestors(el: &AXElement) -> Result<bool, AdapterError> {
        let mut current = crate::tree::copy_element_attr(el, "AXParent");
        for _ in 0..3 {
            let Some(parent) = current else {
                return Ok(false);
            };
            if show_menu_on_element(&parent)? {
                return Ok(true);
            }
            current = crate::tree::copy_element_attr(&parent, "AXParent");
        }
        Ok(false)
    }

    pub(crate) fn show_menu_on_children(el: &AXElement) -> Result<bool, AdapterError> {
        for child in crate::tree::copy_ax_array(el, "AXChildren")
            .unwrap_or_default()
            .iter()
            .take(5)
        {
            if show_menu_on_element(child)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) fn select_then_show_menu(el: &AXElement) -> Result<bool, AdapterError> {
        if !select_containing_item(el)? {
            return Ok(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        show_menu_on_element(el)
    }

    pub(crate) fn select_then_selected_items_menu(el: &AXElement) -> Result<bool, AdapterError> {
        if !select_containing_item(el)? {
            tracing::debug!("selected-items menu: could not select containing item");
            return Ok(false);
        }
        let selected_name = crate::tree::resolve_element_name(el);
        let Some(window) = window_ancestor(el) else {
            tracing::debug!("selected-items menu: no window ancestor");
            return Ok(false);
        };
        let Some(menu_button) = selected_items_menu_button(&window) else {
            tracing::debug!("selected-items menu: no selected-items menu button");
            return Ok(false);
        };
        show_menu_or_press_selected(&menu_button, selected_name.as_deref())
    }

    fn show_menu_on_element(el: &AXElement) -> Result<bool, AdapterError> {
        let Some(pid) = crate::system::app_ops::pid_from_element(el) else {
            return Ok(false);
        };
        let was_open = is_menu_open(pid);
        if !ax_helpers::try_ax_action_retried_or_err(el, "AXShowMenu")? {
            return Ok(false);
        }
        Ok(wait_for_new_menu(pid, was_open))
    }

    fn show_menu_or_press_selected(
        el: &AXElement,
        selected_name: Option<&str>,
    ) -> Result<bool, AdapterError> {
        let Some(pid) = crate::system::app_ops::pid_from_element(el) else {
            return Ok(false);
        };
        if ax_helpers::try_ax_action_retried_or_err(el, "AXShowMenu")?
            && wait_for_selected_menu(pid, selected_name)
        {
            return Ok(true);
        }
        Ok(ax_helpers::try_ax_action_retried_or_err(el, "AXPress")?
            && wait_for_selected_menu(pid, selected_name))
    }

    fn wait_for_new_menu(pid: i32, was_open: bool) -> bool {
        if was_open {
            return false;
        }
        crate::system::wait::wait_for_menu(pid, true, crate::system::wait::menu_timeout_ms())
            .is_ok()
    }

    fn is_menu_open(pid: i32) -> bool {
        crate::system::wait::wait_for_menu(pid, true, 0).is_ok()
    }

    fn wait_for_selected_menu(pid: i32, selected_name: Option<&str>) -> bool {
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_millis(crate::system::wait::menu_timeout_ms());
        loop {
            if menu_matches_selection(pid, selected_name) {
                return true;
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    fn menu_matches_selection(pid: i32, selected_name: Option<&str>) -> bool {
        let Some(menu) = crate::tree::surfaces::menu_element_for_pid(pid) else {
            return false;
        };
        selected_name
            .filter(|name| !name.is_empty())
            .is_none_or(|name| element_text_contains(&menu, name, 0))
    }

    fn element_text_contains(el: &AXElement, needle: &str, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        ["AXTitle", "AXDescription", "AXValue", "AXHelp"]
            .into_iter()
            .filter_map(|attr| crate::tree::copy_string_attr(el, attr))
            .any(|value| value.contains(needle))
            || crate::tree::copy_ax_array(el, "AXChildren")
                .unwrap_or_default()
                .iter()
                .any(|child| element_text_contains(child, needle, depth + 1))
    }

    fn select_containing_item(el: &AXElement) -> Result<bool, AdapterError> {
        Ok(ax_helpers::set_ax_bool_or_err(el, "AXSelected", true)?
            || crate::actions::chain_steps::try_select_containing_item(el)?)
    }

    fn window_ancestor(el: &AXElement) -> Option<AXElement> {
        let mut current = crate::tree::copy_element_attr(el, "AXParent");
        for _ in 0..20 {
            let ancestor = current?;
            if crate::tree::copy_string_attr(&ancestor, "AXRole").as_deref() == Some("AXWindow") {
                return Some(ancestor);
            }
            current = crate::tree::copy_element_attr(&ancestor, "AXParent");
        }
        None
    }

    fn selected_items_menu_button(root: &AXElement) -> Option<AXElement> {
        find_descendant(root, 0, &|el| {
            crate::tree::copy_string_attr(el, "AXRole").as_deref() == Some("AXMenuButton")
                && is_selected_items_control(el)
        })
    }

    fn is_selected_items_control(el: &AXElement) -> bool {
        ["AXHelp", "AXDescription", "AXTitle"]
            .into_iter()
            .filter_map(|attr| crate::tree::copy_string_attr(el, attr))
            .any(|value| {
                let matches = selected_items_text(&value);
                if matches {
                    tracing::debug!(
                        text_chars = value.chars().count(),
                        "selected-items menu: matched control text"
                    );
                }
                matches
            })
    }

    fn selected_items_text(value: &str) -> bool {
        let value = value.to_ascii_lowercase();
        value.contains("selected item")
    }

    fn find_descendant(
        el: &AXElement,
        depth: usize,
        predicate: &impl Fn(&AXElement) -> bool,
    ) -> Option<AXElement> {
        if depth > 8 {
            return None;
        }
        if predicate(el) {
            return Some(el.clone());
        }
        for child in crate::tree::copy_ax_array(el, "AXChildren").unwrap_or_default() {
            if let Some(found) = find_descendant(&child, depth + 1, predicate) {
                return Some(found);
            }
        }
        None
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::AXElement;

    pub fn show_menu(_el: &AXElement) -> bool {
        false
    }

    pub fn show_menu_on_ancestors(_el: &AXElement) -> bool {
        false
    }

    pub fn show_menu_on_children(_el: &AXElement) -> bool {
        false
    }

    pub fn select_then_show_menu(_el: &AXElement) -> bool {
        false
    }

    pub fn select_then_selected_items_menu(_el: &AXElement) -> bool {
        false
    }
}

pub(crate) use imp::{
    select_then_selected_items_menu, select_then_show_menu, show_menu, show_menu_on_ancestors,
    show_menu_on_children,
};
