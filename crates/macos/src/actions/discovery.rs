#[cfg(target_os = "macos")]
#[allow(dead_code)]
mod imp {
    use crate::actions::ax_helpers;
    use crate::tree::AXElement;

    pub struct ElementCaps {
        pub actions: Vec<String>,
        pub settable_value: bool,
        pub settable_focus: bool,
        pub settable_selected: bool,
        pub settable_disclosing: bool,
        pub role: Option<String>,
        pub has_children: bool,
        pub pid: Option<i32>,
    }

    pub fn discover(el: &AXElement) -> ElementCaps {
        let actions = ax_helpers::list_ax_actions(el);
        let settable_value = ax_helpers::is_attr_settable(el, "AXValue");
        let settable_focus = ax_helpers::is_attr_settable(el, "AXFocused");
        let settable_selected = ax_helpers::is_attr_settable(el, "AXSelected");
        let settable_disclosing = ax_helpers::is_attr_settable(el, "AXDisclosing");
        let role = ax_helpers::element_role(el);
        let has_children = crate::tree::copy_ax_array(el, "AXChildren")
            .map(|c| !c.is_empty())
            .unwrap_or(false);
        let pid = crate::system::app_ops::pid_from_element(el);

        ElementCaps {
            actions,
            settable_value,
            settable_focus,
            settable_selected,
            settable_disclosing,
            role,
            has_children,
            pid,
        }
    }

    impl ElementCaps {
        pub fn has_action(&self, name: &str) -> bool {
            self.actions.iter().any(|a| a == name)
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
mod imp {
    use crate::tree::AXElement;

    pub struct ElementCaps {
        pub actions: Vec<String>,
        pub settable_value: bool,
        pub settable_focus: bool,
        pub settable_selected: bool,
        pub settable_disclosing: bool,
        pub role: Option<String>,
        pub has_children: bool,
        pub pid: Option<i32>,
    }

    pub fn discover(_el: &AXElement) -> ElementCaps {
        ElementCaps {
            actions: Vec::new(),
            settable_value: false,
            settable_focus: false,
            settable_selected: false,
            settable_disclosing: false,
            role: None,
            has_children: false,
            pid: None,
        }
    }

    impl ElementCaps {
        pub fn has_action(&self, _name: &str) -> bool {
            false
        }
    }
}

#[allow(unused_imports)]
pub(crate) use imp::{discover, ElementCaps};
