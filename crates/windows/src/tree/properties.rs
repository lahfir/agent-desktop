use agent_desktop_core::AdapterError;

use super::property_ids::TreeProperty;

pub use super::element_properties::ElementProperties;
pub use super::property_outcome::{MAX_EVIDENCE_CHARS, PropertyOutcome, PropertyValue};

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

/// Measures one side of a rectangle.
///
/// Saturating, because a provider is free to hand back nonsense and this crate
/// denies panicking paths: `right - left` on extreme values overflows `i32` and
/// would abort a debug build. An inverted side is degenerate, not
/// negative-sized, so it collapses to zero - the same shape A14-8 measured on a
/// minimized top-level window - rather than travelling downstream as a negative
/// width no consumer expects.
pub fn extent(near: i32, far: i32) -> f64 {
    f64::from(far.saturating_sub(near).max(0))
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
    use windows::core::{IUnknown, Interface};

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

    /// Converts a UI Automation rectangle into core's.
    ///
    /// An inverted rectangle - `right` left of `left` - is degenerate, not
    /// negative-sized. It collapses to zero extent, which is the same shape a
    /// minimized top-level window reports (A14-8), rather than travelling
    /// downstream as a negative width that no consumer expects.
    fn rect_outcome(rectangle: uiautomation::types::Rect) -> PropertyOutcome {
        PropertyOutcome::Known(PropertyValue::Bounds(Rect {
            x: f64::from(rectangle.get_left()),
            y: f64::from(rectangle.get_top()),
            width: super::extent(rectangle.get_left(), rectangle.get_right()),
            height: super::extent(rectangle.get_top(), rectangle.get_bottom()),
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

    thread_local! {
        /// The not-supported sentinel, fetched once per thread.
        ///
        /// UI Automation documents it as a process singleton, so its address
        /// is stable and the comparison below is a pointer test. Fetching it
        /// per property per element turned a constant into an FFI call on the
        /// hottest path in the walk.
        static NOT_SUPPORTED: Option<IUnknown> =
            unsafe { UiaGetReservedNotSupportedValue() }.ok();
    }

    fn is_not_supported(variant: &Variant) -> bool {
        let Ok(Value::UNKNOWN(candidate)) = variant.get_value() else {
            return false;
        };
        NOT_SUPPORTED.with(|sentinel| {
            sentinel
                .as_ref()
                .is_some_and(|sentinel| candidate.as_raw() == sentinel.as_raw())
        })
    }
}

#[cfg(target_os = "windows")]
pub use imp::{read_cached, read_live, read_one};

#[cfg(test)]
#[path = "properties_tests.rs"]
mod tests;
