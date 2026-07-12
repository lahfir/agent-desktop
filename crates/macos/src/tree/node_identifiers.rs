use agent_desktop_core::LocatorField;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NodeIdentifiers {
    pub(crate) ax_identifier: LocatorField<String>,
    pub(crate) ax_dom_identifier: LocatorField<String>,
}

impl NodeIdentifiers {
    #[cfg(test)]
    pub(crate) fn from_options(
        ax_identifier: Option<String>,
        ax_dom_identifier: Option<String>,
    ) -> Self {
        Self {
            ax_identifier: normalize_ax(option_field(ax_identifier)),
            ax_dom_identifier: normalize_dom(option_field(ax_dom_identifier)),
        }
    }

    pub(crate) fn from_fields(
        ax_identifier: LocatorField<String>,
        ax_dom_identifier: LocatorField<String>,
    ) -> Self {
        Self {
            ax_identifier: normalize_ax(ax_identifier),
            ax_dom_identifier: normalize_dom(ax_dom_identifier),
        }
    }
}

impl Default for NodeIdentifiers {
    fn default() -> Self {
        Self {
            ax_identifier: LocatorField::Absent,
            ax_dom_identifier: LocatorField::Absent,
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
fn failed_field(error: i32) -> (LocatorField<String>, bool, bool, bool) {
    use accessibility_sys::{
        kAXErrorAPIDisabled, kAXErrorAttributeUnsupported, kAXErrorCannotComplete,
        kAXErrorInvalidUIElement, kAXErrorNoValue,
    };

    if error == kAXErrorAttributeUnsupported || error == kAXErrorNoValue {
        (LocatorField::Absent, false, false, false)
    } else {
        (
            LocatorField::Unknown,
            error == kAXErrorCannotComplete,
            error == kAXErrorInvalidUIElement,
            error == kAXErrorAPIDisabled,
        )
    }
}

#[cfg(test)]
fn option_field(value: Option<String>) -> LocatorField<String> {
    value
        .map(LocatorField::Known)
        .unwrap_or(LocatorField::Absent)
}

fn normalize_ax(field: LocatorField<String>) -> LocatorField<String> {
    normalize(field, crate::tree::native_id::meaningful_native_id)
}

fn normalize_dom(field: LocatorField<String>) -> LocatorField<String> {
    normalize(field, crate::tree::native_id::meaningful_dom_id)
}

fn normalize(
    field: LocatorField<String>,
    normalize: impl FnOnce(Option<String>) -> Option<String>,
) -> LocatorField<String> {
    match field {
        LocatorField::Known(value) => normalize(Some(value))
            .map(LocatorField::Known)
            .unwrap_or(LocatorField::Absent),
        LocatorField::Absent => LocatorField::Absent,
        LocatorField::Unknown => LocatorField::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    use accessibility_sys::{
        kAXErrorAPIDisabled, kAXErrorAttributeUnsupported, kAXErrorCannotComplete,
        kAXErrorInvalidUIElement,
    };

    #[test]
    fn dual_identifiers_remain_distinct() {
        let identifiers =
            NodeIdentifiers::from_options(Some("native-save".into()), Some("dom-save".into()));

        assert_eq!(
            identifiers.ax_identifier,
            LocatorField::Known("native-save".into())
        );
        assert_eq!(
            identifiers.ax_dom_identifier,
            LocatorField::Known("dom-save".into())
        );
    }

    #[test]
    fn native_and_dom_identifiers_are_both_preserved() {
        let identifiers =
            NodeIdentifiers::from_options(Some("_NS:42".into()), Some("compose".into()));

        assert_eq!(
            identifiers.ax_identifier,
            LocatorField::Known("_NS:42".into())
        );
        assert_eq!(
            identifiers.ax_dom_identifier,
            LocatorField::Known("compose".into())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn identifier_read_failures_preserve_absent_unknown_and_invalid_states() {
        assert_eq!(
            failed_field(kAXErrorAttributeUnsupported),
            (LocatorField::Absent, false, false, false)
        );
        assert_eq!(
            failed_field(kAXErrorCannotComplete),
            (LocatorField::Unknown, true, false, false)
        );
        assert_eq!(
            failed_field(kAXErrorInvalidUIElement),
            (LocatorField::Unknown, false, true, false)
        );
        assert_eq!(
            failed_field(kAXErrorAPIDisabled),
            (LocatorField::Unknown, false, false, true)
        );
    }
}
