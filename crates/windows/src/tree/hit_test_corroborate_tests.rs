use super::{OCCLUDER_EVIDENCE_PROPERTIES, interception_agreed, occluder_from_properties};
use crate::tree::element_properties::ElementProperties;
use crate::tree::name_evidence::LabelOutcome;
use crate::tree::property_ids::TreeProperty;
use crate::tree::property_outcome::{PropertyOutcome, PropertyValue};
use agent_desktop_core::{LocatorField, Rect, hit_test::HitTestResult};

#[test]
fn arm1_same_root_agrees() {
    assert!(interception_agreed(
        Some(10),
        Some(10),
        Some(10),
        Some(1),
        Some(1),
        Some(1)
    ));
}

#[test]
fn arm2_cross_window_roots_agree() {
    assert!(interception_agreed(
        Some(10),
        Some(20),
        Some(20),
        Some(1),
        Some(2),
        Some(2)
    ));
}

#[test]
fn arm3_pid_widening_when_hit_root_unobtainable() {
    assert!(interception_agreed(
        Some(10),
        None,
        Some(20),
        Some(1),
        Some(2),
        Some(2)
    ));
}

#[test]
fn arm3_pid_equal_never_widens() {
    assert!(!interception_agreed(
        Some(10),
        None,
        Some(20),
        Some(1),
        Some(1),
        Some(1)
    ));
}

#[test]
fn win32_skip_cell_is_unknown() {
    assert!(!interception_agreed(
        Some(10),
        Some(20),
        Some(10),
        Some(1),
        Some(2),
        Some(1)
    ));
}

#[test]
fn three_distinct_roots_is_unknown() {
    assert!(!interception_agreed(
        Some(10),
        Some(20),
        Some(30),
        Some(1),
        Some(2),
        Some(3)
    ));
}

#[test]
fn target_root_failure_is_unknown() {
    assert!(!interception_agreed(
        None,
        Some(20),
        Some(20),
        Some(1),
        Some(2),
        Some(2)
    ));
}

#[test]
fn win32_root_failure_is_unknown() {
    assert!(!interception_agreed(
        Some(10),
        Some(20),
        None,
        Some(1),
        Some(2),
        None
    ));
}

#[test]
fn unreadable_pid_blocks_widening() {
    assert!(!interception_agreed(
        Some(10),
        None,
        Some(20),
        Some(1),
        None,
        Some(2)
    ));
    assert!(!interception_agreed(
        Some(10),
        None,
        Some(20),
        None,
        Some(2),
        Some(2)
    ));
    assert!(!interception_agreed(
        Some(10),
        None,
        Some(20),
        Some(1),
        Some(2),
        None
    ));
}

#[test]
fn zero_handles_are_treated_as_unobtainable() {
    assert!(!interception_agreed(
        Some(0),
        Some(20),
        Some(20),
        Some(1),
        Some(2),
        Some(2)
    ));
    assert!(interception_agreed(
        Some(10),
        Some(0),
        Some(20),
        Some(1),
        Some(2),
        Some(2)
    ));
}

#[test]
fn occluder_evidence_batch_includes_is_password() {
    assert!(
        OCCLUDER_EVIDENCE_PROPERTIES.contains(&TreeProperty::IsPassword),
        "withholding is batch-conditional on IsPassword being in the read set"
    );
}

#[test]
fn password_occluder_name_is_withheld() {
    let properties = ElementProperties::from_reads(vec![
        (
            TreeProperty::IsPassword,
            PropertyOutcome::Known(PropertyValue::Flag(true)),
        ),
        (
            TreeProperty::Name,
            PropertyOutcome::Known(PropertyValue::Text("zzfixturesecretzz".into())),
        ),
        (
            TreeProperty::ControlType,
            PropertyOutcome::Known(PropertyValue::Number(50004)),
        ),
        (
            TreeProperty::BoundingRectangle,
            PropertyOutcome::Known(PropertyValue::Bounds(Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            })),
        ),
    ]);
    let result = occluder_from_properties(&properties, LabelOutcome::Unlabelled)
        .expect("evidence assembles");
    match result {
        HitTestResult::InterceptedBy { name, role, .. } => {
            assert_eq!(name, None);
            assert_eq!(role, Some("textfield".into()));
        }
        other => panic!("expected InterceptedBy, got {other:?}"),
    }
}

#[test]
fn control_type_failure_still_reports_unknown_role() {
    let properties = ElementProperties::from_reads(vec![
        (TreeProperty::IsPassword, PropertyOutcome::Absent),
        (TreeProperty::ControlType, PropertyOutcome::Unknown),
        (
            TreeProperty::BoundingRectangle,
            PropertyOutcome::Known(PropertyValue::Bounds(Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            })),
        ),
    ]);
    let result = occluder_from_properties(&properties, LabelOutcome::Unlabelled)
        .expect("evidence assembles");
    match result {
        HitTestResult::InterceptedBy { role, .. } => {
            assert_eq!(role, Some("unknown".into()));
        }
        other => panic!("expected InterceptedBy, got {other:?}"),
    }
}

#[test]
fn bounds_read_failure_demotes_evidence() {
    let properties = ElementProperties::from_reads(vec![
        (TreeProperty::IsPassword, PropertyOutcome::Absent),
        (
            TreeProperty::ControlType,
            PropertyOutcome::Known(PropertyValue::Number(50000)),
        ),
        (TreeProperty::BoundingRectangle, PropertyOutcome::Unknown),
    ]);
    assert!(occluder_from_properties(&properties, LabelOutcome::Unlabelled).is_none());
}

#[test]
fn resolve_role_unknown_maps_to_some_unknown_string() {
    assert_eq!(
        crate::tree::roles::resolve_role(&ElementProperties::from_reads(vec![(
            TreeProperty::ControlType,
            PropertyOutcome::Unknown
        )])),
        LocatorField::Unknown
    );
}

#[test]
fn flipped_arm1_must_not_agree_when_win32_differs() {
    assert!(!interception_agreed(
        Some(10),
        Some(10),
        Some(20),
        Some(1),
        Some(1),
        Some(2)
    ));
}

#[test]
fn flipped_arm2_must_not_agree_when_win32_matches_target() {
    assert!(!interception_agreed(
        Some(10),
        Some(20),
        Some(10),
        Some(1),
        Some(2),
        Some(1)
    ));
}
