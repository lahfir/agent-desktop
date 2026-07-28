use agent_desktop_core::{
    AdapterError, ElementIdentifier, IdentifierEvidence, IdentifierKind, LocatorEvidence,
    LocatorField, LocatorRefEvidence, Rect,
};

use super::property_ids::TreeProperty;

/// Longest string this sub-phase will carry into evidence.
///
/// A value past the bound is `Unknown` rather than a truncated `Known`: a
/// prefix that is presented as exact identity evidence would make 2.5's
/// re-identification match the wrong element.
pub const MAX_EVIDENCE_CHARS: usize = 2_048;

/// One property read, in the three states core's `LocatorField` distinguishes.
///
/// UI Automation has no per-property error channel. macOS gets a parallel
/// array where an absent slot is `kCFNull` and a failed slot carries its own
/// error; UIA has neither, so this type is built by hand from the
/// not-supported sentinel, the variant tag, and the call's own result.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyOutcome {
    /// The provider answered with a value.
    Known(PropertyValue),
    /// The provider answered, and does not implement this property.
    Absent,
    /// The read failed, or its answer cannot be trusted as identity evidence.
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    Text(String),
    Flag(bool),
    Number(i32),
    Bounds(Rect),
}

impl PropertyOutcome {
    pub fn text(&self) -> LocatorField<String> {
        match self {
            Self::Known(PropertyValue::Text(value)) => LocatorField::Known(value.clone()),
            Self::Known(_) => LocatorField::Unknown,
            Self::Absent => LocatorField::Absent,
            Self::Unknown => LocatorField::Unknown,
        }
    }

    pub fn flag(&self) -> Option<bool> {
        match self {
            Self::Known(PropertyValue::Flag(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn number(&self) -> Option<i32> {
        match self {
            Self::Known(PropertyValue::Number(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn bounds(&self) -> LocatorField<Rect> {
        match self {
            Self::Known(PropertyValue::Bounds(value)) => LocatorField::Known(*value),
            Self::Known(_) => LocatorField::Unknown,
            Self::Absent => LocatorField::Absent,
            Self::Unknown => LocatorField::Unknown,
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

/// Bounds a string read and reports whether it survived intact.
///
/// A value longer than the bound is not truncated into evidence; the caller
/// marks it `Unknown`.
pub fn bounded_text(value: String) -> PropertyOutcome {
    if value.chars().count() > MAX_EVIDENCE_CHARS {
        PropertyOutcome::Unknown
    } else {
        PropertyOutcome::Known(PropertyValue::Text(value))
    }
}

/// Builds the structured error for a property read that failed, carrying the
/// property's name and never its value (KTD14).
pub fn property_read_error(base: AdapterError, property: TreeProperty) -> AdapterError {
    base.with_details(serde_json::json!({
        "kind": "property_read_failed",
        "property": property.as_str(),
    }))
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        ElementProperties, PropertyOutcome, PropertyValue, TreeProperty, bounded_text,
        property_read_error,
    };
    use crate::tree::automation::{failure_of, uia_failure_error};
    use crate::tree::element::UIAElement;
    use crate::tree::property_ids::uia_property;
    use agent_desktop_core::{AdapterError, Rect};
    use uiautomation::Error as UiaError;
    use uiautomation::variants::{Value, Variant};
    use windows::Win32::UI::Accessibility::UiaGetReservedNotSupportedValue;
    use windows::core::Interface;

    /// Reads the walk property set from one element, live.
    ///
    /// Errors are collected rather than propagated: a single failed property
    /// makes that slot `Unknown`, it does not abandon the element. The
    /// returned errors carry shape only and feed the walk's incompleteness
    /// accounting.
    pub fn read_live(element: &UIAElement) -> (ElementProperties, Vec<AdapterError>) {
        read_with(element, live_read)
    }

    /// Reads the walk property set from an element's cache.
    ///
    /// A property absent from the cache request is a programming error, not a
    /// live fetch: it classifies `Unknown` with a structured error, never
    /// `Absent`, so a missing request entry cannot masquerade as a provider
    /// that does not implement the property.
    pub fn read_cached(element: &UIAElement) -> (ElementProperties, Vec<AdapterError>) {
        read_with(element, cached_read)
    }

    /// Reads one property live, for the cache policy's provider-class probe
    /// and for tests.
    pub fn read_one(element: &UIAElement, property: TreeProperty) -> PropertyOutcome {
        live_read(element, property).unwrap_or(PropertyOutcome::Unknown)
    }

    fn live_read(
        element: &UIAElement,
        property: TreeProperty,
    ) -> Result<PropertyOutcome, AdapterError> {
        if property == TreeProperty::BoundingRectangle {
            return element
                .0
                .get_bounding_rectangle()
                .map(rect_outcome)
                .map_err(|error| failed(&error, property, "read an element property"));
        }
        element
            .0
            .get_property_value(uia_property(property))
            .map(|variant| classify(&variant))
            .map_err(|error| failed(&error, property, "read an element property"))
    }

    fn cached_read(
        element: &UIAElement,
        property: TreeProperty,
    ) -> Result<PropertyOutcome, AdapterError> {
        if property == TreeProperty::BoundingRectangle {
            return element
                .0
                .get_cached_bounding_rectangle()
                .map(rect_outcome)
                .map_err(|error| failed(&error, property, "read a cached element property"));
        }
        element
            .0
            .get_cached_property_value(uia_property(property))
            .map(|variant| classify(&variant))
            .map_err(|error| failed(&error, property, "read a cached element property"))
    }

    fn failed(error: &UiaError, property: TreeProperty, context: &str) -> AdapterError {
        property_read_error(uia_failure_error(failure_of(error), context), property)
    }

    fn read_with(
        element: &UIAElement,
        read: impl Fn(&UIAElement, TreeProperty) -> Result<PropertyOutcome, AdapterError>,
    ) -> (ElementProperties, Vec<AdapterError>) {
        let mut reads = Vec::with_capacity(TreeProperty::WALK_SET.len());
        let mut errors = Vec::new();
        for property in TreeProperty::WALK_SET {
            match read(element, property) {
                Ok(outcome) => reads.push((property, outcome)),
                Err(error) => {
                    errors.push(error);
                    reads.push((property, PropertyOutcome::Unknown));
                }
            }
        }
        (ElementProperties::from_reads(reads), errors)
    }

    fn rect_outcome(rectangle: uiautomation::types::Rect) -> PropertyOutcome {
        PropertyOutcome::Known(PropertyValue::Bounds(Rect {
            x: f64::from(rectangle.get_left()),
            y: f64::from(rectangle.get_top()),
            width: f64::from(rectangle.get_right() - rectangle.get_left()),
            height: f64::from(rectangle.get_bottom() - rectangle.get_top()),
        }))
    }

    /// Turns one variant into the tri-state.
    ///
    /// The not-supported sentinel is compared by **pointer identity**, which
    /// is the only reliable test: it arrives as `VT_UNKNOWN` carrying the
    /// singleton `UiaGetReservedNotSupportedValue` returns, and no field of
    /// the variant distinguishes it from any other interface pointer.
    /// `VT_EMPTY`, `VT_NULL` and `VT_VOID` mean the provider answered with
    /// nothing, which is `Absent`; a variant this sub-phase cannot decode is
    /// `Unknown`, because presenting an undecoded value as identity evidence
    /// is what silently breaks re-identification downstream.
    fn classify(variant: &Variant) -> PropertyOutcome {
        if is_not_supported(variant) {
            return PropertyOutcome::Absent;
        }
        match variant.get_value() {
            Err(_) => PropertyOutcome::Unknown,
            Ok(Value::EMPTY | Value::NULL | Value::VOID) => PropertyOutcome::Absent,
            Ok(Value::STRING(text)) => bounded_text(text),
            Ok(Value::BOOL(flag)) => PropertyOutcome::Known(PropertyValue::Flag(flag)),
            Ok(Value::I4(number) | Value::INT(number)) => {
                PropertyOutcome::Known(PropertyValue::Number(number))
            }
            Ok(_) => PropertyOutcome::Unknown,
        }
    }

    fn is_not_supported(variant: &Variant) -> bool {
        let Ok(Value::UNKNOWN(candidate)) = variant.get_value() else {
            return false;
        };
        let Ok(sentinel) = (unsafe { UiaGetReservedNotSupportedValue() }) else {
            return false;
        };
        candidate.as_raw() == sentinel.as_raw()
    }
}

#[cfg(target_os = "windows")]
pub use imp::{read_cached, read_live, read_one};

#[cfg(test)]
#[path = "properties_tests.rs"]
mod tests;
