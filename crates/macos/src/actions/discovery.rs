use crate::tree::AXElement;

pub struct ElementCaps {
    pub settable_focus: bool,
    pub settable_selected: bool,
    pub settable_disclosing: bool,
}

#[cfg(target_os = "macos")]
pub fn discover(el: &AXElement) -> ElementCaps {
    use crate::actions::ax_helpers;
    ElementCaps {
        settable_focus: ax_helpers::is_attr_settable(el, "AXFocused"),
        settable_selected: ax_helpers::is_attr_settable(el, "AXSelected"),
        settable_disclosing: ax_helpers::is_attr_settable(el, "AXDisclosing"),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn discover(_el: &AXElement) -> ElementCaps {
    ElementCaps {
        settable_focus: false,
        settable_selected: false,
        settable_disclosing: false,
    }
}
