use agent_desktop_core::{LocatorField, NameEvidence};

use crate::tree::node_attribute_status::{
    DESCRIPTION, LABEL, NodeAttributeStatus, PLACEHOLDER, TITLE, TITLE_ELEMENT, VALUE,
};

pub(crate) fn name_field(
    evidence: &NameEvidence,
    status: &NodeAttributeStatus,
    role: Option<&str>,
    children_complete: bool,
) -> LocatorField<String> {
    let mut uncertain = false;
    for (candidate, unknown) in name_sources(evidence, status, role, children_complete) {
        uncertain |= unknown;
        if let Some(value) = meaningful(candidate) {
            return if uncertain {
                LocatorField::Unknown
            } else {
                LocatorField::Known(value.to_string())
            };
        }
    }
    if uncertain {
        LocatorField::Unknown
    } else {
        LocatorField::Absent
    }
}

pub(crate) fn description_field(
    evidence: &NameEvidence,
    status: &NodeAttributeStatus,
    role: Option<&str>,
    _children_complete: bool,
) -> LocatorField<String> {
    let sources = stronger_name_sources(evidence, status, role);
    let pre_name_known = sources
        .iter()
        .any(|(candidate, _)| meaningful(*candidate).is_some());
    let pre_name_unknown = sources.iter().any(|(_, unknown)| *unknown);
    let description = meaningful(evidence.description.as_deref());
    let description_unknown = status.field_unknown(DESCRIPTION);
    match (description, pre_name_known, pre_name_unknown) {
        (Some(_), true, _) if description_unknown => LocatorField::Unknown,
        (Some(value), true, _) => LocatorField::Known(value.to_string()),
        (Some(_), false, true) => LocatorField::Unknown,
        (None, true, _) if description_unknown => LocatorField::Unknown,
        (None, false, true) if description_unknown => LocatorField::Unknown,
        _ => LocatorField::Absent,
    }
}

fn name_sources<'a>(
    evidence: &'a NameEvidence,
    status: &NodeAttributeStatus,
    role: Option<&str>,
    children_complete: bool,
) -> [(Option<&'a str>, bool); 7] {
    let stronger = stronger_name_sources(evidence, status, role);
    [
        stronger[0],
        stronger[1],
        stronger[2],
        stronger[3],
        (
            evidence.description.as_deref(),
            status.field_unknown(DESCRIPTION),
        ),
        (evidence.child_label.as_deref(), !children_complete),
        (
            evidence.placeholder.as_deref(),
            status.field_unknown(PLACEHOLDER),
        ),
    ]
}

fn stronger_name_sources<'a>(
    evidence: &'a NameEvidence,
    status: &NodeAttributeStatus,
    role: Option<&str>,
) -> [(Option<&'a str>, bool); 4] {
    [
        (
            evidence.explicit_label.as_deref(),
            status.field_unknown(LABEL),
        ),
        (
            evidence.labelled_by_text.as_deref(),
            status.field_unknown(TITLE_ELEMENT),
        ),
        (
            evidence.native_title.as_deref(),
            status.field_unknown(TITLE),
        ),
        (
            (role == Some("AXStaticText"))
                .then_some(evidence.static_value.as_deref())
                .flatten(),
            role == Some("AXStaticText") && status.field_unknown(VALUE),
        ),
    ]
}

fn meaningful(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_higher_priority_name_source_keeps_fallback_unknown() {
        let evidence = NameEvidence {
            native_title: Some("Save".into()),
            ..NameEvidence::default()
        };
        let mut status = NodeAttributeStatus::default();
        status.record_slot_error(LABEL, accessibility_sys::kAXErrorCannotComplete);

        assert_eq!(
            name_field(&evidence, &status, Some("AXButton"), true),
            LocatorField::Unknown
        );
    }

    #[test]
    fn known_higher_priority_name_ignores_lower_priority_unknowns() {
        let evidence = NameEvidence {
            explicit_label: Some("Save".into()),
            ..NameEvidence::default()
        };
        let mut status = NodeAttributeStatus::default();
        status.record_slot_error(TITLE, accessibility_sys::kAXErrorCannotComplete);

        assert_eq!(
            name_field(&evidence, &status, Some("AXButton"), true),
            LocatorField::Known("Save".into())
        );
    }

    #[test]
    fn description_remains_unknown_when_a_name_source_is_transient() {
        let evidence = NameEvidence {
            description: Some("Saves the draft".into()),
            ..NameEvidence::default()
        };
        let mut status = NodeAttributeStatus::default();
        status.record_slot_error(TITLE, accessibility_sys::kAXErrorCannotComplete);

        assert_eq!(
            name_field(&evidence, &status, Some("AXButton"), true),
            LocatorField::Unknown
        );
        assert_eq!(
            description_field(&evidence, &status, Some("AXButton"), true),
            LocatorField::Unknown
        );
    }

    #[test]
    fn description_only_evidence_is_the_name_not_a_duplicate_description() {
        let evidence = NameEvidence {
            description: Some("scroll-area".into()),
            ..NameEvidence::default()
        };
        let status = NodeAttributeStatus::default();

        assert_eq!(
            name_field(&evidence, &status, Some("AXScrollArea"), true),
            LocatorField::Known("scroll-area".into())
        );
        assert_eq!(
            description_field(&evidence, &status, Some("AXScrollArea"), true),
            LocatorField::Absent
        );
    }

    #[test]
    fn description_stays_separate_when_a_stronger_name_exists() {
        let evidence = NameEvidence {
            native_title: Some("Save".into()),
            description: Some("Saves the draft".into()),
            ..NameEvidence::default()
        };
        let status = NodeAttributeStatus::default();

        assert_eq!(
            name_field(&evidence, &status, Some("AXButton"), true),
            LocatorField::Known("Save".into())
        );
        assert_eq!(
            description_field(&evidence, &status, Some("AXButton"), true),
            LocatorField::Known("Saves the draft".into())
        );
    }

    #[test]
    fn explicit_label_also_preserves_a_separate_description() {
        let evidence = NameEvidence {
            explicit_label: Some("Save".into()),
            description: Some("Saves the draft".into()),
            ..NameEvidence::default()
        };
        let status = NodeAttributeStatus::default();

        assert_eq!(
            name_field(&evidence, &status, Some("AXButton"), true),
            LocatorField::Known("Save".into())
        );
        assert_eq!(
            description_field(&evidence, &status, Some("AXButton"), true),
            LocatorField::Known("Saves the draft".into())
        );
    }

    #[test]
    fn unknown_description_cannot_be_bypassed_by_child_content() {
        let evidence = NameEvidence {
            child_label: Some("fallback child".into()),
            ..NameEvidence::default()
        };
        let mut status = NodeAttributeStatus::default();
        status.record_slot_error(DESCRIPTION, accessibility_sys::kAXErrorCannotComplete);

        assert_eq!(
            name_field(&evidence, &status, Some("AXGroup"), true),
            LocatorField::Unknown
        );
    }
}
