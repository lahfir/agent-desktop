use agent_desktop_core::LocatorField;

use super::properties::ElementProperties;

/// Resolves the state vocabulary from the read set and the resolved role.
pub fn resolve_states(
    _properties: &ElementProperties,
    _role: &LocatorField<String>,
) -> LocatorField<Vec<String>> {
    LocatorField::Unknown
}
