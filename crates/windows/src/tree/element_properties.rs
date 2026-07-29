use agent_desktop_core::{
    ElementIdentifier, IdentifierEvidence, IdentifierKind, LocatorEvidence, LocatorField,
    LocatorRefEvidence,
};

use super::property_ids::TreeProperty;
use super::property_outcome::{PropertyOutcome, PropertyValue};

/// Every property read for one element, already gated on `IsPassword`.
#[derive(Debug, Clone, Default)]
pub struct ElementProperties {
    entries: Vec<(TreeProperty, PropertyOutcome)>,
    secure: bool,
}

impl ElementProperties {
    pub fn from_reads(reads: Vec<(TreeProperty, PropertyOutcome)>) -> Self {
        let secure = reads
            .iter()
            .find(|(property, _)| *property == TreeProperty::IsPassword)
            .and_then(|(_, outcome)| outcome.flag())
            .unwrap_or(false);
        let entries = reads
            .into_iter()
            .map(|(property, outcome)| {
                if secure && property.is_value_bearing() {
                    (property, PropertyOutcome::Absent)
                } else {
                    (property, outcome)
                }
            })
            .collect();
        Self { entries, secure }
    }

    pub fn is_secure(&self) -> bool {
        self.secure
    }

    pub fn get(&self, property: TreeProperty) -> PropertyOutcome {
        self.entries
            .iter()
            .find(|(candidate, _)| *candidate == property)
            .map(|(_, outcome)| outcome.clone())
            .unwrap_or(PropertyOutcome::Unknown)
    }

    /// Projects the read set onto the evidence slot shape core consumes, so
    /// 2.4 needs no translation layer.
    ///
    /// `role` and `available_actions` come from the 2.3 seams and are
    /// deliberately `Unknown` until 2.3 fills them; `states` likewise.
    /// `identifiers` uses `IdentifierEvidence::typed`, because
    /// `IdentifierEvidence::new` stamps every value `Unknown` and would void
    /// the ref downstream in `refs_validate.rs`.
    pub fn into_locator_evidence(
        self,
        role: LocatorField<String>,
        available_actions: LocatorField<Vec<String>>,
    ) -> LocatorEvidence {
        let name = self.get(TreeProperty::Name).text();
        let value = self.get(TreeProperty::Value).text();
        let description = self.get(TreeProperty::HelpText).text();
        let bounds = self.get(TreeProperty::BoundingRectangle).bounds();
        LocatorEvidence {
            role,
            name,
            description,
            value,
            identifiers: self.identifier_evidence(),
            states: LocatorField::Unknown,
            ref_evidence: LocatorRefEvidence {
                bounds,
                available_actions,
            },
        }
    }

    fn identifier_evidence(&self) -> IdentifierEvidence {
        let automation_id = self.get(TreeProperty::AutomationId);
        match automation_id {
            PropertyOutcome::Known(PropertyValue::Text(value)) if !value.trim().is_empty() => {
                IdentifierEvidence::typed(
                    [ElementIdentifier {
                        kind: IdentifierKind::AutomationId,
                        value,
                    }],
                    Some(0),
                    true,
                )
            }
            PropertyOutcome::Known(_) | PropertyOutcome::Absent => IdentifierEvidence::absent(),
            PropertyOutcome::Unknown => IdentifierEvidence::unknown(),
        }
    }
}
