use super::*;

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
    let properties = reads(&[
        (
            TreeProperty::IsPassword,
            PropertyOutcome::Known(PropertyValue::Flag(true)),
        ),
        (TreeProperty::Name, text("secret-name")),
        (TreeProperty::Value, text("secret-value")),
        (TreeProperty::HelpText, text("secret-help")),
        (TreeProperty::LegacyValue, text("secret-legacy")),
        (TreeProperty::AutomationId, text("field-1")),
        (TreeProperty::ClassName, text("Edit")),
    ]);

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

/// An unreadable `IsPassword` must not open the gate by accident, so the flag
/// defaults closed only in the sense that a non-`Known(true)` outcome leaves
/// the element non-secure - the branch that matters is asserted explicitly
/// rather than left to a default.
#[test]
fn an_unreadable_is_password_leaves_the_element_unmarked() {
    let properties = reads(&[
        (TreeProperty::IsPassword, PropertyOutcome::Unknown),
        (TreeProperty::Value, text("visible-value")),
    ]);

    assert!(!properties.is_secure());
}

#[test]
fn an_absent_automation_id_is_complete_evidence_and_a_failed_read_is_not() {
    let absent = reads(&[(TreeProperty::AutomationId, PropertyOutcome::Absent)])
        .into_locator_evidence(LocatorField::Unknown, LocatorField::Unknown);
    let failed = reads(&[(TreeProperty::AutomationId, PropertyOutcome::Unknown)])
        .into_locator_evidence(LocatorField::Unknown, LocatorField::Unknown);

    assert!(absent.identifiers.is_complete());
    assert!(!failed.identifiers.is_complete());
}

/// `IdentifierEvidence::new` stamps `IdentifierKind::Unknown`, which
/// `refs_validate.rs` treats as no identifier at all; the reader must use
/// `typed`.
#[test]
fn an_automation_id_is_carried_as_a_typed_identifier() {
    let evidence = reads(&[(TreeProperty::AutomationId, text("save-button"))])
        .into_locator_evidence(LocatorField::Unknown, LocatorField::Unknown);

    let identifier = evidence
        .identifiers
        .preferred_identifier()
        .expect("a known automation id is the preferred identifier");
    assert_eq!(identifier.kind, IdentifierKind::AutomationId);
    assert_eq!(identifier.value, "save-button");
}

#[test]
fn a_whitespace_only_automation_id_is_not_promoted_to_an_identifier() {
    let evidence = reads(&[(TreeProperty::AutomationId, text("   "))])
        .into_locator_evidence(LocatorField::Unknown, LocatorField::Unknown);

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
    );

    assert_eq!(evidence.name, LocatorField::Known("Save".into()));
    assert_eq!(evidence.value, LocatorField::Known("draft".into()));
    assert_eq!(evidence.description, LocatorField::Absent);
    assert_eq!(evidence.role, LocatorField::Known("button".into()));
    assert!(!evidence.ref_evidence.bounds.is_unknown());
    assert!(evidence.states.is_unknown());
}

/// KTD14: a failed read must name the property and never carry its content.
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
mod windows_only {
    use super::*;
    use crate::tree::fixture::{CONTENT_MARKER, HostedFixture, SECURE_MARKER};
    use crate::tree::properties::read_cached;
    use uiautomation::UIElement;

    fn walk_children(handle: isize) -> Vec<crate::tree::element::UIAElement> {
        let root = crate::tree::automation::root_from_hwnd(
            handle,
            agent_desktop_core::Deadline::standard().expect("a standard deadline"),
        )
        .expect("the fixture window resolves");
        let client = crate::tree::automation::automation_client().expect("a UIA client");
        let walker = client
            .get_raw_view_walker()
            .expect("the raw view walker is available");
        let mut children: Vec<UIElement> = Vec::new();
        if let Ok(mut current) = walker.get_first_child(&root.0) {
            loop {
                let next = walker.get_next_sibling(&current);
                children.push(current);
                match next {
                    Ok(sibling) => current = sibling,
                    Err(_) => break,
                }
            }
        }
        children
            .into_iter()
            .map(crate::tree::element::UIAElement::from)
            .collect()
    }

    fn bootstrap() {
        crate::tree::fixture::ensure_test_apartment();
    }

    /// The gate asserted against a real out-of-process provider, not only
    /// against the projection: text typed into the fixture's `ES_PASSWORD`
    /// control must appear in no read outcome for `Value`, `Name` or
    /// `HelpText`.
    #[test]
    fn secure_content_never_reaches_a_read_outcome_from_a_live_provider() {
        bootstrap();
        let fixture = HostedFixture::spawn().expect("the fixture host starts");
        let children = walk_children(fixture.handle());

        let secure = children
            .iter()
            .map(|child| read_live(child).0)
            .find(ElementProperties::is_secure)
            .expect("the fixture exposes a control reporting IsPassword");

        for property in TreeProperty::VALUE_BEARING {
            let rendered = format!("{:?}", secure.get(property));
            assert!(
                !rendered.contains(SECURE_MARKER),
                "{} leaked secure content",
                property.as_str()
            );
            assert_eq!(secure.get(property), PropertyOutcome::Absent);
        }
    }

    #[test]
    fn a_live_read_distinguishes_present_content_from_an_unimplemented_property() {
        bootstrap();
        let fixture = HostedFixture::spawn().expect("the fixture host starts");
        let children = walk_children(fixture.handle());
        let all: Vec<ElementProperties> = children.iter().map(|child| read_live(child).0).collect();

        assert!(
            all.iter().any(|properties| matches!(
                properties.get(TreeProperty::Value),
                PropertyOutcome::Known(PropertyValue::Text(ref value)) if value.contains(CONTENT_MARKER)
            )),
            "a plain control's content must read back as Known"
        );
        assert!(
            all.iter().any(|properties| matches!(
                properties.get(TreeProperty::AutomationId),
                PropertyOutcome::Known(_) | PropertyOutcome::Absent
            )),
            "an unimplemented property must classify Absent, never Unknown"
        );
        assert!(
            all.iter().all(|properties| !matches!(
                properties.get(TreeProperty::ClassName),
                PropertyOutcome::Unknown
            )),
            "a readable property must never classify Unknown on a healthy provider"
        );
    }

    /// A14-9: once the host process exits, the client-side HWND proxy answers
    /// property reads locally with an empty string rather than failing, so
    /// process death is no more detectable through a property read than
    /// through the sibling terminator (A14-4).
    ///
    /// The guarantee 2.2 does make is asserted instead, and it is the one
    /// that matters downstream: a provider that went away is never reported
    /// as a provider that does not implement the property. `Absent` feeds
    /// completeness gating as a legitimate answer; fabricating it here would
    /// let a dead target satisfy `EvidenceRequirements` it never answered.
    #[test]
    fn a_read_after_the_provider_exits_never_fabricates_absent() {
        bootstrap();
        let mut fixture = HostedFixture::spawn().expect("the fixture host starts");
        let children = walk_children(fixture.handle());
        assert!(!children.is_empty(), "the fixture exposes child controls");

        fixture.terminate();
        std::thread::sleep(std::time::Duration::from_millis(750));

        let (properties, _) = read_live(&children[0]);

        for property in [
            TreeProperty::ClassName,
            TreeProperty::Name,
            TreeProperty::Value,
        ] {
            assert_ne!(
                properties.get(property),
                PropertyOutcome::Absent,
                "a provider that went away must not be reported as not implementing {}",
                property.as_str()
            );
        }
    }

    /// The failure branch, forced deterministically rather than by killing a
    /// process: a property the cache request never carried cannot be answered,
    /// and must classify `Unknown` with a structured error.
    #[test]
    fn a_read_that_genuinely_fails_classifies_unknown_and_never_absent() {
        bootstrap();
        let fixture = HostedFixture::spawn().expect("the fixture host starts");
        let root = crate::tree::automation::root_from_hwnd(
            fixture.handle(),
            agent_desktop_core::Deadline::standard().expect("a standard deadline"),
        )
        .expect("the fixture window resolves");
        let client = crate::tree::automation::automation_client().expect("a UIA client");
        let request = client
            .create_cache_request()
            .expect("an empty cache request builds");
        request
            .set_element_mode(uiautomation::types::ElementMode::Full)
            .expect("the element mode is settable");
        let sparse = root
            .0
            .build_updated_cache(&request)
            .map(crate::tree::element::UIAElement::from)
            .expect("an element with an empty cache");

        let (properties, errors) = read_cached(&sparse);

        assert!(
            !errors.is_empty(),
            "a read that cannot be answered must surface a structured error"
        );
        assert_eq!(
            properties.get(TreeProperty::ClassName),
            PropertyOutcome::Unknown
        );
        assert_ne!(
            properties.get(TreeProperty::ClassName),
            PropertyOutcome::Absent
        );
    }
}
