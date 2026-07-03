#[cfg(target_os = "macos")]
mod imp {
    use crate::tree::{AXElement, copy_string_attr, resolve_element_name};
    use accessibility_sys::kAXRoleAttribute;
    use agent_desktop_core::accname::NameEvidence;

    pub fn name_evidence_impl(el: &AXElement) -> NameEvidence {
        let ax_role = copy_string_attr(el, kAXRoleAttribute);
        let title = copy_string_attr(el, "AXTitle");
        let description = copy_string_attr(el, "AXDescription");
        let value = copy_string_attr(el, "AXValue");
        let value_promoted =
            title.is_none() && description.is_none() && ax_role.as_deref() == Some("AXStaticText");
        let resolved = resolve_element_name(el);
        let child_label = resolved.is_some() && title.is_none() && description.is_none();
        NameEvidence {
            title,
            description,
            value_promoted: value_promoted && value.is_some(),
            child_label,
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::AXElement;
    use agent_desktop_core::accname::NameEvidence;

    pub fn name_evidence_impl(_el: &AXElement) -> NameEvidence {
        NameEvidence {
            title: None,
            description: None,
            value_promoted: false,
            child_label: false,
        }
    }
}

pub use imp::name_evidence_impl;
