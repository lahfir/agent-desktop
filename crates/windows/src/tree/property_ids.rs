/// The properties the walk reads, named independently of the UI Automation
/// numbering.
///
/// The numbering is deliberately absent here. A2-5 measured that UIA property
/// ids are build-specific - `IsAnnotationPatternAvailable` is 30118 on build
/// 17763 while 30113 is a different property - and named 2.2 as the place a
/// hand-written table would fail silently. Every id comes from the crate's
/// generated constants through the mapping below.
///
/// Pattern-derived state is read here as ordinary properties. UIA exposes
/// `ToggleState`, `ExpandCollapseState`, `SelectionItem.IsSelected`,
/// `Value.IsReadOnly` and `Selection.CanSelectMultiple` as plain automation
/// properties, so they arrive in the same batch as everything else and cost no
/// `QueryInterface` per node per pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeProperty {
    Name,
    AutomationId,
    ClassName,
    HelpText,
    FullDescription,
    LabeledBy,
    Value,
    LegacyValue,
    BoundingRectangle,
    IsPassword,
    IsOffscreen,
    IsEnabled,
    IsControlElement,
    IsContentElement,
    IsKeyboardFocusable,
    HasKeyboardFocus,
    IsRequiredForForm,
    IsDataValidForForm,
    IsDialog,
    ToggleState,
    ExpandCollapseState,
    SelectionItemIsSelected,
    ValueIsReadOnly,
    SelectionCanSelectMultiple,
    WindowIsModal,
    LegacyState,
    LegacyDefaultAction,
    InvokeAvailable,
    ToggleAvailable,
    ExpandCollapseAvailable,
    SelectionItemAvailable,
    SelectionAvailable,
    ValueAvailable,
    RangeValueAvailable,
    ScrollAvailable,
    ScrollItemAvailable,
    WindowAvailable,
    GridItemAvailable,
    TableItemAvailable,
    LegacyAvailable,
    ProviderDescription,
    ControlType,
    RuntimeId,
}

impl TreeProperty {
    /// Every property whose content originates in the target and must
    /// therefore be withheld from a control reporting `IsPassword`.
    ///
    /// `LabeledBy` is not here and is not an omission: it yields an *element*,
    /// not text, so there is no content on this element to withhold. The text
    /// derived from it is guarded at both ends in `name_evidence.rs` - the
    /// referring element's own `IsPassword`, and the target's, which A15-2
    /// measured as a reachable case rather than a hypothetical one.
    pub const VALUE_BEARING: [TreeProperty; 6] = [
        TreeProperty::Name,
        TreeProperty::HelpText,
        TreeProperty::FullDescription,
        TreeProperty::Value,
        TreeProperty::LegacyValue,
        TreeProperty::LegacyDefaultAction,
    ];

    /// The set a subtree walk reads, and therefore the set a cache request
    /// carries. Nothing else is requested, so nothing unread is paid for.
    ///
    /// **The set is flat, and that is a measured decision rather than a
    /// default.** A15-11 measured the expanded set against 2.2's ten
    /// properties on the same walk: 1.16x on the in-process Win32 proxy at 20
    /// nodes, 1.89x on a real out-of-process WPF provider at 46 nodes, min of
    /// seven repeats after a discarded warm-up. That is materially slower, so
    /// A15-12 built the conditional split the plan pre-committed to - core
    /// plus availability prefetched, pattern state fetched with one
    /// `BuildUpdatedCache` round trip per node that advertises a pattern - and
    /// timed it head to head. It costs 0.95x to 1.05x the flat set: no
    /// recovery in either direction, because the expense sits in the identity
    /// and naming properties **every** node needs and no gate can exclude.
    ///
    /// The split would also trade a bounded per-node prefetch for a round trip
    /// per pattern-bearing node, which is unbounded on a selection-dense tree.
    /// What was available instead was trimming: `ItemStatus`, `Orientation`
    /// and `RangeValueIsReadOnly` are read by no vocabulary here and are not
    /// requested.
    pub const WALK_SET: [TreeProperty; 41] = [
        TreeProperty::Name,
        TreeProperty::AutomationId,
        TreeProperty::ClassName,
        TreeProperty::HelpText,
        TreeProperty::FullDescription,
        TreeProperty::LabeledBy,
        TreeProperty::Value,
        TreeProperty::LegacyValue,
        TreeProperty::BoundingRectangle,
        TreeProperty::IsPassword,
        TreeProperty::IsOffscreen,
        TreeProperty::IsEnabled,
        TreeProperty::IsControlElement,
        TreeProperty::IsContentElement,
        TreeProperty::IsKeyboardFocusable,
        TreeProperty::HasKeyboardFocus,
        TreeProperty::IsRequiredForForm,
        TreeProperty::IsDataValidForForm,
        TreeProperty::IsDialog,
        TreeProperty::ToggleState,
        TreeProperty::ExpandCollapseState,
        TreeProperty::SelectionItemIsSelected,
        TreeProperty::ValueIsReadOnly,
        TreeProperty::SelectionCanSelectMultiple,
        TreeProperty::WindowIsModal,
        TreeProperty::LegacyState,
        TreeProperty::LegacyDefaultAction,
        TreeProperty::ControlType,
        TreeProperty::InvokeAvailable,
        TreeProperty::ToggleAvailable,
        TreeProperty::ExpandCollapseAvailable,
        TreeProperty::SelectionItemAvailable,
        TreeProperty::SelectionAvailable,
        TreeProperty::ValueAvailable,
        TreeProperty::RangeValueAvailable,
        TreeProperty::ScrollAvailable,
        TreeProperty::ScrollItemAvailable,
        TreeProperty::WindowAvailable,
        TreeProperty::GridItemAvailable,
        TreeProperty::TableItemAvailable,
        TreeProperty::LegacyAvailable,
    ];

    pub fn is_value_bearing(self) -> bool {
        Self::VALUE_BEARING.contains(&self)
    }

    /// Whether the property's value is an element rather than a scalar.
    ///
    /// The tri-state classifier decodes strings, flags, numbers and bounds; an
    /// element arrives as `VT_UNKNOWN` and would classify `Unknown`, which
    /// would report a successful read as a failed one. An element-valued
    /// property therefore rides the cache request - so the read is cheap - but
    /// is resolved by the consumer that knows what to do with an element,
    /// never through `ElementProperties`.
    pub fn is_element_valued(self) -> bool {
        matches!(self, TreeProperty::LabeledBy)
    }

    /// The availability property that must report `true` before this
    /// property's value means anything.
    ///
    /// A15-7 measured that a pattern-state property returns a **default-looking
    /// value** and never the not-supported sentinel on an element whose
    /// provider does not implement the pattern: a static text reports
    /// `ToggleState` = `Indeterminate` and `ValueIsReadOnly` = `true`. Read
    /// ungated, every inert node in every tree would carry two states it does
    /// not have. The gate lives here, on the property, so a consumer cannot
    /// forget it.
    pub fn gate(self) -> Option<TreeProperty> {
        match self {
            TreeProperty::ToggleState => Some(TreeProperty::ToggleAvailable),
            TreeProperty::ExpandCollapseState => Some(TreeProperty::ExpandCollapseAvailable),
            TreeProperty::SelectionItemIsSelected => Some(TreeProperty::SelectionItemAvailable),
            TreeProperty::ValueIsReadOnly => Some(TreeProperty::ValueAvailable),
            TreeProperty::SelectionCanSelectMultiple => Some(TreeProperty::SelectionAvailable),
            TreeProperty::WindowIsModal => Some(TreeProperty::WindowAvailable),
            _ => None,
        }
    }

    /// Names the property for a structured error without exposing its value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::AutomationId => "AutomationId",
            Self::ClassName => "ClassName",
            Self::HelpText => "HelpText",
            Self::FullDescription => "FullDescription",
            Self::LabeledBy => "LabeledBy",
            Self::Value => "Value",
            Self::LegacyValue => "LegacyIAccessibleValue",
            Self::BoundingRectangle => "BoundingRectangle",
            Self::IsPassword => "IsPassword",
            Self::IsOffscreen => "IsOffscreen",
            Self::IsEnabled => "IsEnabled",
            Self::IsControlElement => "IsControlElement",
            Self::IsContentElement => "IsContentElement",
            Self::IsKeyboardFocusable => "IsKeyboardFocusable",
            Self::HasKeyboardFocus => "HasKeyboardFocus",
            Self::IsRequiredForForm => "IsRequiredForForm",
            Self::IsDataValidForForm => "IsDataValidForForm",
            Self::IsDialog => "IsDialog",
            Self::ToggleState => "ToggleToggleState",
            Self::ExpandCollapseState => "ExpandCollapseExpandCollapseState",
            Self::SelectionItemIsSelected => "SelectionItemIsSelected",
            Self::ValueIsReadOnly => "ValueIsReadOnly",
            Self::SelectionCanSelectMultiple => "SelectionCanSelectMultiple",
            Self::WindowIsModal => "WindowIsModal",
            Self::LegacyState => "LegacyIAccessibleState",
            Self::LegacyDefaultAction => "LegacyIAccessibleDefaultAction",
            Self::InvokeAvailable => "IsInvokePatternAvailable",
            Self::ToggleAvailable => "IsTogglePatternAvailable",
            Self::ExpandCollapseAvailable => "IsExpandCollapsePatternAvailable",
            Self::SelectionItemAvailable => "IsSelectionItemPatternAvailable",
            Self::SelectionAvailable => "IsSelectionPatternAvailable",
            Self::ValueAvailable => "IsValuePatternAvailable",
            Self::RangeValueAvailable => "IsRangeValuePatternAvailable",
            Self::ScrollAvailable => "IsScrollPatternAvailable",
            Self::ScrollItemAvailable => "IsScrollItemPatternAvailable",
            Self::WindowAvailable => "IsWindowPatternAvailable",
            Self::GridItemAvailable => "IsGridItemPatternAvailable",
            Self::TableItemAvailable => "IsTableItemPatternAvailable",
            Self::LegacyAvailable => "IsLegacyIAccessiblePatternAvailable",
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
            TreeProperty::FullDescription => UIProperty::FullDescription,
            TreeProperty::LabeledBy => UIProperty::LabeledBy,
            TreeProperty::Value => UIProperty::ValueValue,
            TreeProperty::LegacyValue => UIProperty::LegacyIAccessibleValue,
            TreeProperty::BoundingRectangle => UIProperty::BoundingRectangle,
            TreeProperty::IsPassword => UIProperty::IsPassword,
            TreeProperty::IsOffscreen => UIProperty::IsOffscreen,
            TreeProperty::IsEnabled => UIProperty::IsEnabled,
            TreeProperty::IsControlElement => UIProperty::IsControlElement,
            TreeProperty::IsContentElement => UIProperty::IsContentElement,
            TreeProperty::IsKeyboardFocusable => UIProperty::IsKeyboardFocusable,
            TreeProperty::HasKeyboardFocus => UIProperty::HasKeyboardFocus,
            TreeProperty::IsRequiredForForm => UIProperty::IsRequiredForForm,
            TreeProperty::IsDataValidForForm => UIProperty::IsDataValidForForm,
            TreeProperty::IsDialog => UIProperty::IsDialog,
            TreeProperty::ToggleState => UIProperty::ToggleToggleState,
            TreeProperty::ExpandCollapseState => UIProperty::ExpandCollapseExpandCollapseState,
            TreeProperty::SelectionItemIsSelected => UIProperty::SelectionItemIsSelected,
            TreeProperty::ValueIsReadOnly => UIProperty::ValueIsReadOnly,
            TreeProperty::SelectionCanSelectMultiple => UIProperty::SelectionCanSelectMultiple,
            TreeProperty::WindowIsModal => UIProperty::WindowIsModal,
            TreeProperty::LegacyState => UIProperty::LegacyIAccessibleState,
            TreeProperty::LegacyDefaultAction => UIProperty::LegacyIAccessibleDefaultAction,
            TreeProperty::InvokeAvailable => UIProperty::IsInvokePatternAvailable,
            TreeProperty::ToggleAvailable => UIProperty::IsTogglePatternAvailable,
            TreeProperty::ExpandCollapseAvailable => UIProperty::IsExpandCollapsePatternAvailable,
            TreeProperty::SelectionItemAvailable => UIProperty::IsSelectionItemPatternAvailable,
            TreeProperty::SelectionAvailable => UIProperty::IsSelectionPatternAvailable,
            TreeProperty::ValueAvailable => UIProperty::IsValuePatternAvailable,
            TreeProperty::RangeValueAvailable => UIProperty::IsRangeValuePatternAvailable,
            TreeProperty::ScrollAvailable => UIProperty::IsScrollPatternAvailable,
            TreeProperty::ScrollItemAvailable => UIProperty::IsScrollItemPatternAvailable,
            TreeProperty::WindowAvailable => UIProperty::IsWindowPatternAvailable,
            TreeProperty::GridItemAvailable => UIProperty::IsGridItemPatternAvailable,
            TreeProperty::TableItemAvailable => UIProperty::IsTableItemPatternAvailable,
            TreeProperty::LegacyAvailable => UIProperty::IsLegacyIAccessiblePatternAvailable,
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
