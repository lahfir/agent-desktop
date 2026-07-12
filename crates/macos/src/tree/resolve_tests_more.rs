use super::tests::entry;
use super::*;
use crate::tree::AXElement;
use crate::tree::resolve_classify::classify_candidates;
use crate::tree::resolve_search::{match_native_or_text_identity, should_stop_collecting};

#[test]
fn identifier_kind_mismatch_is_not_an_exact_match() {
    use agent_desktop_core::{
        ElementIdentifier, IdentifierEvidence, IdentifierKind, LocatorEvidence, LocatorField,
        LocatorRefEvidence,
    };

    let mut stored = entry(Some(42), Some("w-42"), None, None);
    stored.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AxIdentifier,
        value: "stable-id".into(),
    });
    let live = LocatorEvidence {
        role: LocatorField::Known(stored.identity.role.clone()),
        name: LocatorField::Known("Investors".into()),
        description: LocatorField::Absent,
        value: LocatorField::Absent,
        identifiers: IdentifierEvidence::typed(
            [ElementIdentifier {
                kind: IdentifierKind::AxDomIdentifier,
                value: "stable-id".into(),
            }],
            Some(0),
            true,
        ),
        states: LocatorField::Absent,
        ref_evidence: LocatorRefEvidence {
            bounds: LocatorField::Absent,
            available_actions: LocatorField::Absent,
        },
    };

    assert_eq!(
        match_native_or_text_identity(&stored, &live),
        agent_desktop_core::IdentityMatch::NoMatch
    );
}

#[test]
fn duplicate_identity_candidates_remain_ambiguous_after_bounds_drift() {
    let error = classify_candidates(
        vec![
            AXElement(std::ptr::null_mut()),
            AXElement(std::ptr::null_mut()),
        ],
        &entry(Some(42), Some("w-42"), None, None),
        true,
        Instant::now() + Duration::from_secs(1),
    )
    .err()
    .expect("duplicate live identities must remain ambiguous");

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
}

#[test]
fn unscoped_candidate_requires_matching_bounds() {
    let error = classify_candidates(
        vec![AXElement(std::ptr::null_mut())],
        &entry(Some(42), Some("w-42"), None, None),
        false,
        Instant::now() + Duration::from_secs(1),
    )
    .err()
    .expect("unscoped candidate without matching bounds must fail");

    assert_eq!(error.code, ErrorCode::ElementNotFound);
}

#[test]
fn bounds_search_keeps_collecting_for_disambiguation() {
    assert!(!should_stop_collecting(
        2,
        &entry(Some(42), None, None, None)
    ));
    assert!(should_stop_collecting(2, &entry(None, None, None, None)));
}

#[test]
fn ax_read_errors_distinguish_absent_unknown_and_permission_denied() {
    use accessibility_sys::{
        kAXErrorAPIDisabled, kAXErrorAttributeUnsupported, kAXErrorCannotComplete,
        kAXErrorInvalidUIElement,
    };

    assert!(
        !crate::tree::resolve_ax_read::classify_ax_read(
            kAXErrorAttributeUnsupported,
            "AXIdentifier"
        )
        .unwrap()
    );
    for error in [kAXErrorCannotComplete, kAXErrorInvalidUIElement] {
        let classified =
            crate::tree::resolve_ax_read::classify_ax_read(error, "AXIdentifier").unwrap_err();
        assert_eq!(classified.code, ErrorCode::AppUnresponsive);
        assert_eq!(classified.details.unwrap()["complete"], false);
    }
    let denied =
        crate::tree::resolve_ax_read::classify_ax_read(kAXErrorAPIDisabled, "AXIdentifier")
            .unwrap_err();
    assert_eq!(denied.code, ErrorCode::PermDenied);
}
