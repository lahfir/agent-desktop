use agent_desktop_core::LocatorField;

use super::properties::ElementProperties;

/// Resolves a canonical role from the read set.
pub fn resolve_role(_properties: &ElementProperties) -> LocatorField<String> {
    LocatorField::Unknown
}
