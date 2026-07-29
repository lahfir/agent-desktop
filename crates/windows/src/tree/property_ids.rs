/// The properties sub-phase 2.2 reads, named independently of the UI
/// Automation numbering.
///
/// The numbering is deliberately absent here. A2-5 measured that UIA property
/// ids are build-specific - `IsAnnotationPatternAvailable` is 30118 on build
/// 17763 while 30113 is a different property - and named 2.2 as the place a
/// hand-written table would fail silently. Every id comes from the crate's
/// generated constants through the mapping below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeProperty {
    Name,
    AutomationId,
    ClassName,
    HelpText,
    Value,
    LegacyValue,
    BoundingRectangle,
    IsPassword,
    IsOffscreen,
    IsEnabled,
    ProviderDescription,
    ControlType,
    RuntimeId,
}

impl TreeProperty {
    /// Every property whose content originates in the target and must
    /// therefore be withheld from a control reporting `IsPassword`.
    pub const VALUE_BEARING: [TreeProperty; 4] = [
        TreeProperty::Name,
        TreeProperty::HelpText,
        TreeProperty::Value,
        TreeProperty::LegacyValue,
    ];

    /// The set a subtree walk reads, and therefore the set a cache request
    /// carries. Nothing else is requested, so nothing unread is paid for.
    pub const WALK_SET: [TreeProperty; 10] = [
        TreeProperty::Name,
        TreeProperty::AutomationId,
        TreeProperty::ClassName,
        TreeProperty::HelpText,
        TreeProperty::Value,
        TreeProperty::LegacyValue,
        TreeProperty::BoundingRectangle,
        TreeProperty::IsPassword,
        TreeProperty::IsOffscreen,
        TreeProperty::IsEnabled,
    ];

    pub fn is_value_bearing(self) -> bool {
        Self::VALUE_BEARING.contains(&self)
    }

    /// Names the property for a structured error without exposing its value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::AutomationId => "AutomationId",
            Self::ClassName => "ClassName",
            Self::HelpText => "HelpText",
            Self::Value => "Value",
            Self::LegacyValue => "LegacyIAccessibleValue",
            Self::BoundingRectangle => "BoundingRectangle",
            Self::IsPassword => "IsPassword",
            Self::IsOffscreen => "IsOffscreen",
            Self::IsEnabled => "IsEnabled",
            Self::ProviderDescription => "ProviderDescription",
            Self::ControlType => "ControlType",
            Self::RuntimeId => "RuntimeId",
        }
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::TreeProperty;
    use uiautomation::types::UIProperty;

    /// Resolves an internal property to the crate's generated constant.
    ///
    /// An exhaustive `match` with no catch-all arm, so adding a variant is a
    /// compile error rather than a silent fallback to a wrong id.
    pub fn uia_property(property: TreeProperty) -> UIProperty {
        match property {
            TreeProperty::Name => UIProperty::Name,
            TreeProperty::AutomationId => UIProperty::AutomationId,
            TreeProperty::ClassName => UIProperty::ClassName,
            TreeProperty::HelpText => UIProperty::HelpText,
            TreeProperty::Value => UIProperty::ValueValue,
            TreeProperty::LegacyValue => UIProperty::LegacyIAccessibleValue,
            TreeProperty::BoundingRectangle => UIProperty::BoundingRectangle,
            TreeProperty::IsPassword => UIProperty::IsPassword,
            TreeProperty::IsOffscreen => UIProperty::IsOffscreen,
            TreeProperty::IsEnabled => UIProperty::IsEnabled,
            TreeProperty::ProviderDescription => UIProperty::ProviderDescription,
            TreeProperty::ControlType => UIProperty::ControlType,
            TreeProperty::RuntimeId => UIProperty::RuntimeId,
        }
    }
}

#[cfg(target_os = "windows")]
pub use imp::uia_property;

#[cfg(test)]
#[path = "property_ids_tests.rs"]
mod tests;
