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

/// What the vocabulary modules made of one element's read set.
///
/// These travel together because they are resolved together, from the same
/// batch, by the same walk step. Passing them as five separate arguments to
/// the projection was the alternative, and it put the projection one slot away
/// from the parameter limit with every future evidence field making it worse.
#[derive(Debug, Clone)]
pub struct ResolvedVocabulary {
    pub role: LocatorField<String>,
    pub available_actions: LocatorField<Vec<String>>,
    pub states: LocatorField<Vec<String>>,
    pub name: LocatorField<String>,
    pub description: LocatorField<String>,
}

impl ResolvedVocabulary {
    /// The answer for an element nothing was read from: every slot unknown.
    /// Used by the non-Windows twin and by tests that assert on other slots.
    pub fn unknown() -> Self {
        Self {
            role: LocatorField::Unknown,
            available_actions: LocatorField::Unknown,
            states: LocatorField::Unknown,
            name: LocatorField::Unknown,
            description: LocatorField::Unknown,
        }
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

    /// Reads a boolean **through its gate**, which is the only safe way to
    /// read one.
    ///
    /// A15-7 measured that a pattern-state property returns a
    /// default-looking value - never the not-supported sentinel - on an
    /// element whose provider does not implement the pattern: a static text
    /// reports `ValueIsReadOnly` = `true` with no `Value` pattern at all.
    /// `TreeProperty::gate()` names the availability property that has to be
    /// `true` first, and this is the single accessor that consults it, so a
    /// caller cannot read a gated property without the gate. A property with
    /// no gate reads straight through.
    pub fn gated_flag(&self, property: TreeProperty) -> Option<bool> {
        if !self.gate_open(property) {
            return None;
        }
        self.get(property).flag()
    }

    pub fn gated_number(&self, property: TreeProperty) -> Option<i32> {
        if !self.gate_open(property) {
            return None;
        }
        self.get(property).number()
    }

    /// Whether a gated property's value means anything on this element.
    pub fn is_true(&self, property: TreeProperty) -> bool {
        self.gated_flag(property) == Some(true)
    }

    fn gate_open(&self, property: TreeProperty) -> bool {
        match property.gate() {
            Some(gate) => self.get(gate).flag() == Some(true),
            None => true,
        }
    }

    /// Projects the read set onto the evidence slot shape core consumes, so
    /// the consumer needs no translation layer.
    ///
    /// Every interpreted slot is resolved by a vocabulary module from this
    /// same read set and threaded in by the caller, so this stays a projection
    /// and takes no decision of its own - notably the name, which core
    /// computes and no adapter does. `identifiers` uses
    /// `IdentifierEvidence::typed`, because `IdentifierEvidence::new` stamps
    /// every value `Unknown` and would void the ref downstream in
    /// `refs_validate.rs`.
    pub fn into_locator_evidence(self, vocabulary: ResolvedVocabulary) -> LocatorEvidence {
        let value = self.get(TreeProperty::Value).text();
        let bounds = self.get(TreeProperty::BoundingRectangle).bounds();
        LocatorEvidence {
            role: vocabulary.role,
            name: vocabulary.name,
            description: vocabulary.description,
            value,
            identifiers: self.identifier_evidence(),
            states: vocabulary.states,
            ref_evidence: LocatorRefEvidence {
                bounds,
                available_actions: vocabulary.available_actions,
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
