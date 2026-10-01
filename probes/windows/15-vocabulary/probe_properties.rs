//! The property sets sub-phase 2.3 measures the cost of, named once so the
//! cost comparison and the measurements read the same lists.

use uiautomation::types::UIProperty;

pub const BASE_SET: [UIProperty; 10] = [
    UIProperty::Name,
    UIProperty::AutomationId,
    UIProperty::ClassName,
    UIProperty::HelpText,
    UIProperty::ValueValue,
    UIProperty::LegacyIAccessibleValue,
    UIProperty::BoundingRectangle,
    UIProperty::IsPassword,
    UIProperty::IsOffscreen,
    UIProperty::IsEnabled,
];

/// Every property sub-phase 2.3 is considering reading, so the cost question is
/// measured against the widest candidate set rather than an optimistic subset.
pub const FULL_SET: [UIProperty; 43] = [
    UIProperty::Name,
    UIProperty::AutomationId,
    UIProperty::ClassName,
    UIProperty::HelpText,
    UIProperty::ValueValue,
    UIProperty::LegacyIAccessibleValue,
    UIProperty::BoundingRectangle,
    UIProperty::IsPassword,
    UIProperty::IsOffscreen,
    UIProperty::IsEnabled,
    UIProperty::ControlType,
    UIProperty::FullDescription,
    UIProperty::LabeledBy,
    UIProperty::IsControlElement,
    UIProperty::IsContentElement,
    UIProperty::IsKeyboardFocusable,
    UIProperty::HasKeyboardFocus,
    UIProperty::IsRequiredForForm,
    UIProperty::IsDataValidForForm,
    UIProperty::ItemStatus,
    UIProperty::Orientation,
    UIProperty::IsDialog,
    UIProperty::ToggleToggleState,
    UIProperty::ExpandCollapseExpandCollapseState,
    UIProperty::SelectionItemIsSelected,
    UIProperty::ValueIsReadOnly,
    UIProperty::RangeValueIsReadOnly,
    UIProperty::SelectionCanSelectMultiple,
    UIProperty::WindowIsModal,
    UIProperty::LegacyIAccessibleState,
    UIProperty::LegacyIAccessibleDefaultAction,
    UIProperty::IsInvokePatternAvailable,
    UIProperty::IsTogglePatternAvailable,
    UIProperty::IsExpandCollapsePatternAvailable,
    UIProperty::IsSelectionItemPatternAvailable,
    UIProperty::IsSelectionPatternAvailable,
    UIProperty::IsValuePatternAvailable,
    UIProperty::IsRangeValuePatternAvailable,
    UIProperty::IsScrollPatternAvailable,
    UIProperty::IsScrollItemPatternAvailable,
    UIProperty::IsWindowPatternAvailable,
    UIProperty::IsGridItemPatternAvailable,
    UIProperty::IsLegacyIAccessiblePatternAvailable,
];

/// The baseline plus everything the vocabulary needs that is *not* a pattern
/// question: identity, naming, the view-membership flags and the form flags.
/// Every node needs all of these, so this group cannot be gated on anything.
pub const CORE_SET: [UIProperty; 24] = [
    UIProperty::Name,
    UIProperty::AutomationId,
    UIProperty::ClassName,
    UIProperty::HelpText,
    UIProperty::ValueValue,
    UIProperty::LegacyIAccessibleValue,
    UIProperty::BoundingRectangle,
    UIProperty::IsPassword,
    UIProperty::IsOffscreen,
    UIProperty::IsEnabled,
    UIProperty::ControlType,
    UIProperty::FullDescription,
    UIProperty::LabeledBy,
    UIProperty::IsControlElement,
    UIProperty::IsContentElement,
    UIProperty::IsKeyboardFocusable,
    UIProperty::HasKeyboardFocus,
    UIProperty::IsRequiredForForm,
    UIProperty::IsDataValidForForm,
    UIProperty::ItemStatus,
    UIProperty::Orientation,
    UIProperty::IsDialog,
    UIProperty::LegacyIAccessibleState,
    UIProperty::LegacyIAccessibleDefaultAction,
];

/// The core set plus the availability flags role refinement and the action
/// list read. If a conditional split were taken, this is what phase one would
/// have to fetch, because the flags are what the gate reads.
pub const CORE_PLUS_AVAILABILITY: [UIProperty; 36] = [
    UIProperty::Name,
    UIProperty::AutomationId,
    UIProperty::ClassName,
    UIProperty::HelpText,
    UIProperty::ValueValue,
    UIProperty::LegacyIAccessibleValue,
    UIProperty::BoundingRectangle,
    UIProperty::IsPassword,
    UIProperty::IsOffscreen,
    UIProperty::IsEnabled,
    UIProperty::ControlType,
    UIProperty::FullDescription,
    UIProperty::LabeledBy,
    UIProperty::IsControlElement,
    UIProperty::IsContentElement,
    UIProperty::IsKeyboardFocusable,
    UIProperty::HasKeyboardFocus,
    UIProperty::IsRequiredForForm,
    UIProperty::IsDataValidForForm,
    UIProperty::ItemStatus,
    UIProperty::Orientation,
    UIProperty::IsDialog,
    UIProperty::LegacyIAccessibleState,
    UIProperty::LegacyIAccessibleDefaultAction,
    UIProperty::IsInvokePatternAvailable,
    UIProperty::IsTogglePatternAvailable,
    UIProperty::IsExpandCollapsePatternAvailable,
    UIProperty::IsSelectionItemPatternAvailable,
    UIProperty::IsSelectionPatternAvailable,
    UIProperty::IsValuePatternAvailable,
    UIProperty::IsRangeValuePatternAvailable,
    UIProperty::IsScrollPatternAvailable,
    UIProperty::IsScrollItemPatternAvailable,
    UIProperty::IsWindowPatternAvailable,
    UIProperty::IsGridItemPatternAvailable,
    UIProperty::IsLegacyIAccessiblePatternAvailable,
];

/// The pattern-state properties question 6 reads on an element implementing
/// none of them.
pub const PATTERN_STATE: [(&str, UIProperty); 5] = [
    ("ToggleToggleState", UIProperty::ToggleToggleState),
    (
        "ExpandCollapseExpandCollapseState",
        UIProperty::ExpandCollapseExpandCollapseState,
    ),
    ("SelectionItemIsSelected", UIProperty::SelectionItemIsSelected),
    ("ValueIsReadOnly", UIProperty::ValueIsReadOnly),
    (
        "SelectionCanSelectMultiple",
        UIProperty::SelectionCanSelectMultiple,
    ),
];

/// How many times each property set is walked before its cost is reported.
/// One walk each produced a non-monotonic ordering in which adding twelve
/// properties made the walk faster, so a single sample cannot support the
/// flat-versus-split decision it was taken for.
pub const BATCH_REPEATS: usize = 7;
