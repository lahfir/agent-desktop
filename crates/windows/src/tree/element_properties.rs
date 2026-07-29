use agent_desktop_core::{
    ElementIdentifier, IdentifierEvidence, IdentifierKind, LocatorEvidence, LocatorField,
    LocatorRefEvidence,
};

use super::property_ids::TreeProperty;
use super::property_outcome::{PropertyOutcome, PropertyValue};

/// Withholds a value-bearing property from a secure element.
///
/// Only a value that was actually read is replaced. A read that failed carries
/// no content, so there is nothing to withhold — and rewriting it to `Absent`
/// would claim the provider does not implement the property, which is false
/// and is exactly the fabrication A14-9 forbids: `Absent` is a legitimate
/// answer that satisfies `EvidenceRequirements`, and a target that never
/// answered must not be able to satisfy them.
fn withheld(outcome: PropertyOutcome) -> PropertyOutcome {
    match outcome {
        PropertyOutcome::Known(_) => PropertyOutcome::Absent,
        PropertyOutcome::Absent => PropertyOutcome::Absent,
        PropertyOutcome::Unknown => PropertyOutcome::Unknown,
    }
}

fn withholds_content(outcome: &PropertyOutcome) -> bool {
    match outcome {
        PropertyOutcome::Known(PropertyValue::Flag(secure)) => *secure,
        PropertyOutcome::Known(PropertyValue::Number(secure)) => *secure != 0,
        PropertyOutcome::Known(_) => true,
        PropertyOutcome::Unknown => true,
        PropertyOutcome::Absent => false,
    }
}

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
            .is_some_and(|(_, outcome)| withholds_content(outcome));
        let entries = reads
            .into_iter()
            .map(|(property, outcome)| {
                if secure && property.is_value_bearing() {
                    (property, withheld(outcome))
                } else {
                    (property, outcome)
                }
            })
            .collect();
        Self { entries, secure }
    }

    /// Decides the secure-field gate from one `IsPassword` outcome.
    ///
    /// Fails closed, because the cost is asymmetric: withholding a name that
    /// was not secret loses a little evidence, while publishing one that was
    /// puts a password into a snapshot, a session JSONL segment and a trace
    /// HTML export.
    ///
    /// - `Known(true)` is the ordinary answer, and a non-zero integer is the
    ///   same answer from a provider that returns `VT_I4` where UIA documents
    ///   `VT_BOOL`. Reading only `VT_BOOL` would let such a provider open the
    ///   gate.
    /// - `Unknown` means the read failed, which is not evidence the element is
    ///   safe, so it withholds too.
    /// - `Absent` means the provider answered and does not implement the
    ///   property, which is a real answer: not a password field.
    ///
    /// An element whose read set never requested `IsPassword` is not gated at
    /// all - there was nothing to gate, and the caller chose the property set.
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
