#[cfg(target_os = "macos")]
mod imp {
    use crate::tree::AXElement;
    use crate::tree::attributes::copy_string_attr;
    use accessibility_sys::{
        kAXDescriptionAttribute, kAXRoleAttribute, kAXTitleAttribute, kAXValueAttribute,
    };
    use agent_desktop_core::accname::NameEvidence;

    const STATIC_TEXT_ROLE: &str = "AXStaticText";

    /// Gathers raw `NameEvidence` from an element's own AX attributes: its
    /// title, its description, and (for static text only) its value. Precedence
    /// between them is `resolve_element_name`'s job, never decided here.
    pub fn name_evidence_impl(el: &AXElement) -> NameEvidence {
        let ax_role = copy_string_attr(el, kAXRoleAttribute);
        NameEvidence {
            native_title: copy_string_attr(el, kAXTitleAttribute),
            static_role_value: static_role_value(el, ax_role.as_deref()),
            description: copy_string_attr(el, kAXDescriptionAttribute),
        }
    }

    fn static_role_value(el: &AXElement, ax_role: Option<&str>) -> Option<String> {
        if ax_role != Some(STATIC_TEXT_ROLE) {
            return None;
        }
        copy_string_attr(el, kAXValueAttribute)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::AXElement;
    use agent_desktop_core::accname::NameEvidence;

    pub fn name_evidence_impl(_el: &AXElement) -> NameEvidence {
        NameEvidence::default()
    }
}

pub use imp::name_evidence_impl;
