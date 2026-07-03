#[cfg(target_os = "macos")]
mod imp {
    use crate::tree::AXElement;
    use crate::tree::attributes::{
        copy_ax_array, copy_ax_array_prefix, copy_element_attr, copy_string_attr,
    };
    use crate::tree::element::child_attributes;
    use accessibility_sys::{
        kAXChildrenAttribute, kAXDescriptionAttribute, kAXPlaceholderValueAttribute,
        kAXRoleAttribute, kAXTitleAttribute, kAXTitleUIElementAttribute, kAXValueAttribute,
    };
    use agent_desktop_core::accname::{NameEvidence, join_child_labels};

    const STATIC_TEXT_ROLE: &str = "AXStaticText";

    /// Gathers raw `NameEvidence` from an element's own AX attributes.
    /// Every rung is a direct attribute read — precedence between rungs is
    /// core's job via `accname::compute_name`, never decided here.
    pub fn name_evidence_impl(el: &AXElement) -> NameEvidence {
        let ax_role = copy_string_attr(el, kAXRoleAttribute);
        NameEvidence {
            explicit_label: None,
            labelled_by_text: labelled_by_text(el),
            native_title: copy_string_attr(el, kAXTitleAttribute),
            static_role_value: static_role_value(el, ax_role.as_deref()),
            child_label: aggregated_child_label(el, ax_role.as_deref()),
            placeholder: copy_string_attr(el, kAXPlaceholderValueAttribute),
            description: copy_string_attr(el, kAXDescriptionAttribute),
        }
    }

    fn labelled_by_text(el: &AXElement) -> Option<String> {
        let label_el = copy_element_attr(el, kAXTitleUIElementAttribute)?;
        copy_string_attr(&label_el, kAXTitleAttribute)
            .or_else(|| copy_string_attr(&label_el, kAXValueAttribute))
    }

    fn static_role_value(el: &AXElement, ax_role: Option<&str>) -> Option<String> {
        if ax_role != Some(STATIC_TEXT_ROLE) {
            return None;
        }
        copy_string_attr(el, kAXValueAttribute)
    }

    fn aggregated_child_label(el: &AXElement, ax_role: Option<&str>) -> Option<String> {
        let children = child_attributes(ax_role)
            .iter()
            .find_map(|attr| copy_ax_array(el, attr).filter(|v| !v.is_empty()))
            .unwrap_or_default();
        let texts = collect_child_texts(&children);
        join_child_labels(texts.iter().map(String::as_str))
    }

    fn collect_child_texts(children: &[AXElement]) -> Vec<String> {
        fn text_of(el: &AXElement) -> Option<String> {
            copy_string_attr(el, kAXValueAttribute)
                .or_else(|| copy_string_attr(el, kAXTitleAttribute))
        }

        let mut texts = Vec::new();
        for child in children.iter().take(5) {
            match copy_string_attr(child, kAXRoleAttribute).as_deref() {
                Some("AXStaticText") => {
                    if let Some(text) = text_of(child) {
                        texts.push(text);
                    }
                }
                Some("AXCell") | Some("AXGroup") => {
                    for grandchild in
                        copy_ax_array_prefix(child, kAXChildrenAttribute, 5).unwrap_or_default()
                    {
                        if copy_string_attr(&grandchild, kAXRoleAttribute).as_deref()
                            == Some("AXStaticText")
                            && let Some(text) = text_of(&grandchild)
                        {
                            texts.push(text);
                        }
                    }
                }
                _ => {}
            }
        }
        texts
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
