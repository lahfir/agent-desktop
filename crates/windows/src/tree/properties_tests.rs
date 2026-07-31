use super::*;
use agent_desktop_core::{IdentifierKind, LocatorField, Rect};

fn text(value: &str) -> PropertyOutcome {
    PropertyOutcome::Known(PropertyValue::Text(value.into()))
}

fn reads(entries: &[(TreeProperty, PropertyOutcome)]) -> ElementProperties {
    ElementProperties::from_reads(entries.to_vec())
}

#[test]
fn a_property_the_provider_does_not_implement_is_absent_not_unknown() {
    let properties = reads(&[(TreeProperty::Name, PropertyOutcome::Absent)]);

    assert_eq!(
        properties.get(TreeProperty::Name).text(),
        LocatorField::Absent
    );
    assert!(!properties.get(TreeProperty::Name).text().is_unknown());
}

#[test]
fn a_property_whose_read_failed_is_unknown_not_absent() {
    let properties = reads(&[(TreeProperty::Name, PropertyOutcome::Unknown)]);

    assert_eq!(
        properties.get(TreeProperty::Name).text(),
        LocatorField::Unknown
    );
}

#[test]
fn an_empty_but_present_string_stays_known() {
    let properties = reads(&[(TreeProperty::Name, text(""))]);

    assert_eq!(
        properties.get(TreeProperty::Name).text(),
        LocatorField::Known(String::new())
    );
}

#[test]
fn a_string_past_the_bound_is_unknown_and_is_never_truncated_into_evidence() {
    let oversized = "x".repeat(MAX_EVIDENCE_CHARS + 1);

    let outcome = bounded_text(oversized.clone());

    assert_eq!(outcome, PropertyOutcome::Unknown);
    assert_eq!(outcome.text(), LocatorField::Unknown);
    assert_eq!(
        bounded_text("x".repeat(MAX_EVIDENCE_CHARS)).text(),
        LocatorField::Known("x".repeat(MAX_EVIDENCE_CHARS))
    );
}

/// KTD13's gate, asserted on the projection rather than on a provider: every
/// value-bearing property is withheld when `IsPassword` is true, and nothing
/// else is.
#[test]
fn a_secure_element_withholds_every_value_bearing_property() {
    let mut entries = vec![
        (
            TreeProperty::IsPassword,
            PropertyOutcome::Known(PropertyValue::Flag(true)),
        ),
        (TreeProperty::AutomationId, text("field-1")),
        (TreeProperty::ClassName, text("Edit")),
    ];
    for property in TreeProperty::VALUE_BEARING {
        entries.push((property, text("zzsecretzz")));
    }
    let properties = reads(&entries);

    assert!(properties.is_secure());
    for property in TreeProperty::VALUE_BEARING {
        assert_eq!(
            properties.get(property),
            PropertyOutcome::Absent,
            "{} leaked through a secure element",
            property.as_str()
        );
    }
    assert_eq!(properties.get(TreeProperty::AutomationId), text("field-1"));
    assert_eq!(properties.get(TreeProperty::ClassName), text("Edit"));
}

#[test]
fn a_non_secure_element_keeps_every_value_bearing_property() {
    let properties = reads(&[
        (
            TreeProperty::IsPassword,
            PropertyOutcome::Known(PropertyValue::Flag(false)),
        ),
        (TreeProperty::Name, text("visible-name")),
        (TreeProperty::Value, text("visible-value")),
    ]);

    assert!(!properties.is_secure());
    assert_eq!(properties.get(TreeProperty::Name), text("visible-name"));
    assert_eq!(properties.get(TreeProperty::Value), text("visible-value"));
}

/// The gate fails closed. A failed `IsPassword` read is not evidence the
/// element is safe to publish, and the costs are asymmetric: withholding a
/// name that was not secret loses a little evidence, publishing one that was
/// puts a password into a snapshot, a session JSONL segment and a trace HTML
/// export.
#[test]
fn an_unreadable_is_password_withholds_rather_than_publishing() {
    let properties = reads(&[
        (TreeProperty::IsPassword, PropertyOutcome::Unknown),
        (TreeProperty::Value, text("visible-value")),
    ]);

    assert!(properties.is_secure());
    assert_eq!(properties.get(TreeProperty::Value), PropertyOutcome::Absent);
}

/// Withholding replaces content, it does not invent an answer. When the whole
/// read failed there is nothing to withhold, and rewriting `Unknown` to
/// `Absent` would claim the provider does not implement the property — the
/// fabrication A14-9 forbids, because `Absent` satisfies
/// `EvidenceRequirements` and a target that never answered must not.
#[test]
fn withholding_never_turns_a_failed_read_into_a_real_answer() {
    let properties = reads(&[
        (TreeProperty::IsPassword, PropertyOutcome::Unknown),
        (TreeProperty::Value, PropertyOutcome::Unknown),
        (TreeProperty::Name, text("readable-name")),
    ]);

    assert!(properties.is_secure());
    assert_eq!(
        properties.get(TreeProperty::Value),
        PropertyOutcome::Unknown,
        "a failed read has no content to withhold"
    );
    assert_eq!(
        properties.get(TreeProperty::Name),
        PropertyOutcome::Absent,
        "content that was actually read is withheld"
    );
}

/// A provider that answers `IsPassword` as an integer where UIA documents a
/// boolean must not slip past the gate. Reading only `VT_BOOL` would let it.
#[test]
fn an_integer_is_password_still_closes_the_gate() {
    let properties = reads(&[
        (
            TreeProperty::IsPassword,
            PropertyOutcome::Known(PropertyValue::Number(1)),
        ),
        (TreeProperty::Value, text("secret-value")),
    ]);

    assert!(properties.is_secure());
    assert_eq!(properties.get(TreeProperty::Value), PropertyOutcome::Absent);
}

/// A provider that answers `IsPassword` and does not implement it has given a
/// real answer, so the gate stays open - otherwise every element on a provider
/// without the property would lose its name and value.
#[test]
fn an_absent_is_password_is_a_real_answer_and_leaves_content_readable() {
    let properties = reads(&[
        (TreeProperty::IsPassword, PropertyOutcome::Absent),
        (TreeProperty::Value, text("visible-value")),
    ]);

    assert!(!properties.is_secure());
    assert_eq!(properties.get(TreeProperty::Value), text("visible-value"));
}

/// An element whose read set never asked for `IsPassword` is not gated: there
/// was nothing to gate, and the walk chooses the property set.
#[test]
fn a_read_set_without_the_flag_is_not_gated() {
    let properties = reads(&[(TreeProperty::Value, text("visible-value"))]);

    assert!(!properties.is_secure());
    assert_eq!(properties.get(TreeProperty::Value), text("visible-value"));
}

#[test]
fn an_absent_automation_id_is_complete_evidence_and_a_failed_read_is_not() {
    let absent = reads(&[(TreeProperty::AutomationId, PropertyOutcome::Absent)])
        .into_locator_evidence(
            LocatorField::Unknown,
            LocatorField::Unknown,
            LocatorField::Unknown,
        );
    let failed = reads(&[(TreeProperty::AutomationId, PropertyOutcome::Unknown)])
        .into_locator_evidence(
            LocatorField::Unknown,
            LocatorField::Unknown,
            LocatorField::Unknown,
        );

    assert!(absent.identifiers.is_complete());
    assert!(!failed.identifiers.is_complete());
}

/// `IdentifierEvidence::new` stamps `IdentifierKind::Unknown`, which
/// `refs_validate.rs` treats as no identifier at all; the reader must use
/// `typed`.
#[test]
fn an_automation_id_is_carried_as_a_typed_identifier() {
    let evidence = reads(&[(TreeProperty::AutomationId, text("save-button"))])
        .into_locator_evidence(
            LocatorField::Unknown,
            LocatorField::Unknown,
            LocatorField::Unknown,
        );

    let identifier = evidence
        .identifiers
        .preferred_identifier()
        .expect("a known automation id is the preferred identifier");
    assert_eq!(identifier.kind, IdentifierKind::AutomationId);
    assert_eq!(identifier.value, "save-button");
}

#[test]
fn a_whitespace_only_automation_id_is_not_promoted_to_an_identifier() {
    let evidence = reads(&[(TreeProperty::AutomationId, text("   "))]).into_locator_evidence(
        LocatorField::Unknown,
        LocatorField::Unknown,
        LocatorField::Unknown,
    );

    assert!(evidence.identifiers.preferred_identifier().is_none());
    assert!(evidence.identifiers.is_complete());
}

#[test]
fn the_evidence_projection_fills_every_slot_the_walk_owns() {
    let evidence = reads(&[
        (TreeProperty::Name, text("Save")),
        (TreeProperty::Value, text("draft")),
        (TreeProperty::HelpText, PropertyOutcome::Absent),
        (
            TreeProperty::BoundingRectangle,
            PropertyOutcome::Known(PropertyValue::Bounds(Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            })),
        ),
    ])
    .into_locator_evidence(
        LocatorField::Known("button".into()),
        LocatorField::Known(Vec::new()),
        LocatorField::Known(vec!["focused".into()]),
    );

    assert_eq!(evidence.name, LocatorField::Known("Save".into()));
    assert_eq!(evidence.value, LocatorField::Known("draft".into()));
    assert_eq!(evidence.description, LocatorField::Absent);
    assert_eq!(evidence.role, LocatorField::Known("button".into()));
    assert!(!evidence.ref_evidence.bounds.is_unknown());
    assert_eq!(
        evidence.states,
        LocatorField::Known(vec!["focused".into()]),
        "the states slot carries what the caller resolved, rather than a literal the projection invented"
    );
}

/// KTD14: a failed read must name the property and never carry its content.
/// A provider is free to hand back nonsense. An inverted side is degenerate,
/// not negative-sized, and an extreme one must not overflow a debug build.
#[test]
fn a_rectangle_side_is_never_negative_and_never_overflows() {
    assert_eq!(extent(10, 40), 30.0);
    assert_eq!(extent(40, 10), 0.0, "an inverted side collapses to zero");
    assert_eq!(extent(7, 7), 0.0);
    assert_eq!(
        extent(i32::MIN, i32::MAX),
        f64::from(i32::MAX),
        "an extreme span saturates rather than overflowing"
    );
    assert_eq!(extent(i32::MAX, i32::MIN), 0.0);
}

#[test]
fn a_property_read_error_names_the_property_and_carries_no_content() {
    const MARKER: &str = "zzmarkerzz-account-number";
    let base = crate::tree::automation::uia_failure_error(
        crate::tree::automation::UiaFailure::Sentinel(crate::tree::automation::ERR_TIMEOUT),
        "read an element property",
    );

    let error = property_read_error(base, TreeProperty::Value);

    let rendered = format!(
        "{}|{}|{}",
        error.message,
        error.platform_detail.clone().unwrap_or_default(),
        serde_json::to_string(&error.details).unwrap_or_default()
    );
    assert!(
        rendered.contains("Value"),
        "the error must name the property"
    );
    assert!(!rendered.contains(MARKER), "leaked: {rendered}");
}

#[cfg(target_os = "windows")]
#[path = "properties_live_tests.rs"]
mod windows_only;
