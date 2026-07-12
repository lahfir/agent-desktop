#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentifierEvidence {
    identifiers: Vec<crate::ElementIdentifier>,
    preferred: Option<usize>,
    complete: bool,
}

impl IdentifierEvidence {
    pub fn new(
        values: impl IntoIterator<Item = String>,
        preferred: Option<usize>,
        complete: bool,
    ) -> Self {
        Self::typed(
            values.into_iter().map(|value| crate::ElementIdentifier {
                kind: crate::IdentifierKind::Unknown,
                value,
            }),
            preferred,
            complete,
        )
    }

    pub fn typed(
        identifiers: impl IntoIterator<Item = crate::ElementIdentifier>,
        preferred: Option<usize>,
        complete: bool,
    ) -> Self {
        let original = identifiers.into_iter().collect::<Vec<_>>();
        let preferred_value = preferred
            .and_then(|index| original.get(index))
            .filter(|identifier| !identifier.value.trim().is_empty())
            .cloned();
        let mut normalized = Vec::new();
        for identifier in original {
            if !identifier.value.trim().is_empty() && !normalized.contains(&identifier) {
                normalized.push(identifier);
            }
        }
        let preferred = preferred_value
            .as_ref()
            .and_then(|value| normalized.iter().position(|candidate| candidate == value));
        Self {
            identifiers: normalized,
            preferred,
            complete,
        }
    }

    pub fn absent() -> Self {
        Self::new([], None, true)
    }

    pub fn unknown() -> Self {
        Self::new([], None, false)
    }

    pub fn identifiers(&self) -> &[crate::ElementIdentifier] {
        &self.identifiers
    }

    #[cfg(test)]
    pub fn values(&self) -> Vec<&str> {
        self.identifiers
            .iter()
            .map(|identifier| identifier.value.as_str())
            .collect()
    }

    pub fn preferred_value(&self) -> Option<&str> {
        self.preferred
            .and_then(|index| self.identifiers.get(index))
            .map(|identifier| identifier.value.as_str())
    }

    pub fn preferred_identifier(&self) -> Option<&crate::ElementIdentifier> {
        self.preferred.and_then(|index| self.identifiers.get(index))
    }

    pub fn preferred_index(&self) -> Option<usize> {
        self.preferred
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_index_is_resolved_before_empty_values_are_removed() {
        let evidence = IdentifierEvidence::new([String::new(), "dom-id".into()], Some(1), true);

        assert_eq!(evidence.values(), ["dom-id"]);
        assert_eq!(evidence.preferred_index(), Some(0));
        assert_eq!(evidence.preferred_value(), Some("dom-id"));
    }

    #[test]
    fn preferred_duplicate_maps_to_the_deduplicated_value() {
        let evidence = IdentifierEvidence::new(
            ["shared".into(), "other".into(), "shared".into()],
            Some(2),
            true,
        );

        assert_eq!(evidence.values(), ["shared", "other"]);
        assert_eq!(evidence.preferred_index(), Some(0));
    }
}
