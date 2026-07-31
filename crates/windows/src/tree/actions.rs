use agent_desktop_core::LocatorField;

use super::properties::ElementProperties;

/// Resolves the available-action list from the read set.
pub fn resolve_actions(_properties: &ElementProperties) -> LocatorField<Vec<String>> {
    LocatorField::Unknown
}
