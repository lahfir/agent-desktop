//! Property sets the 2.4 observation probe compares for marginal cost.
//!
//! The probe pre-commits to the same comparison A15-11 used: the walk as 2.3
//! ships it (the 40-property flat set that sub-phase 2.4 inherits), against the
//! same set plus the three new properties 2.4 adds to the walk
//! (`LocalizedControlType`, `AriaRole`, `AriaProperties`). The comparison is
//! the evidence U1's cost row rests on: if adding three properties is inside
//! the per-property envelope A15-11 established, the flat set survives U1's
//! branch (U3 keeps WALK_SET flat); if it materially re-opens the split, U1
//! records that instead.
//!
//! Nothing here is product code.

use uiautomation::types::UIProperty;

pub const BASE_SET: [UIProperty; 8] = [
    UIProperty::Name,
    UIProperty::AutomationId,
    UIProperty::ClassName,
    UIProperty::HelpText,
    UIProperty::FullDescription,
    UIProperty::ValueValue,
    UIProperty::BoundingRectangle,
    UIProperty::ControlType,
];

/// The three properties 2.4 adds to the walk, each first-observed here.
pub const EXTRA_SET: [UIProperty; 3] = [
    UIProperty::LocalizedControlType,
    UIProperty::AriaRole,
    UIProperty::AriaProperties,
];
