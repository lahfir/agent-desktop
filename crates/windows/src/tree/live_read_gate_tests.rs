//! The essential-completeness gate is a pure predicate over the evidence, so
//! it is pinned here without a UI Automation client and on every target,
//! rather than beside the fixture-driven tests: any essential slot reading
//! `Unknown` - the completeness rule - rejects the bundle, and the rejection
//! error is the retryable `AppUnresponsive` the loop retries instead of a
//! partial answer claiming completeness.
//!
//! What these tests deliberately cannot see is whether the shared read still
//! consults the predicate at all. That wiring is pinned against a real
//! element in `live_read_seam_tests.rs`, because it is only observable
//! through a read that reaches the gate.

use super::{essential_live_evidence_complete, incomplete_live_evidence};
use agent_desktop_core::{
    ElementIdentifier, IdentifierEvidence, IdentifierKind, LocatorEvidence, LocatorField,
    LocatorRefEvidence, NodeDescriptor, Rect,
};

fn complete_evidence() -> LocatorEvidence {
    LocatorEvidence {
        role: LocatorField::Known("button".to_string()),
        name: LocatorField::Known("name".to_string()),
        value: LocatorField::Known("value".to_string()),
        description: LocatorField::Absent,
        identifiers: IdentifierEvidence::typed(
            [ElementIdentifier {
                kind: IdentifierKind::AutomationId,
                value: "id-1".to_string(),
            }],
            None,
            true,
        ),
        states: LocatorField::Known(vec!["enabled".to_string()]),
        ref_evidence: LocatorRefEvidence {
            bounds: LocatorField::Known(Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            }),
            available_actions: LocatorField::Known(vec!["click".to_string()]),
            descriptors: NodeDescriptor::default(),
        },
    }
}

#[test]
fn a_complete_bundle_passes_the_gate() {
    assert!(essential_live_evidence_complete(&complete_evidence()));
}

#[test]
fn an_unknown_role_fails_the_gate_retryable() {
    let evidence = LocatorEvidence {
        role: LocatorField::Unknown,
        ..complete_evidence()
    };
    assert!(!essential_live_evidence_complete(&evidence));
    let error = incomplete_live_evidence();
    assert!(error.is_explicitly_retryable());
    assert_eq!(error.code, agent_desktop_core::ErrorCode::AppUnresponsive);
}

#[test]
fn an_unknown_value_states_bounds_or_actions_fail_the_gate() {
    for field in [
        LocatorEvidence {
            value: LocatorField::Unknown,
            ..complete_evidence()
        },
        LocatorEvidence {
            states: LocatorField::Unknown,
            ..complete_evidence()
        },
        LocatorEvidence {
            ref_evidence: LocatorRefEvidence {
                bounds: LocatorField::Unknown,
                ..complete_evidence().ref_evidence
            },
            ..complete_evidence()
        },
        LocatorEvidence {
            ref_evidence: LocatorRefEvidence {
                available_actions: LocatorField::Unknown,
                ..complete_evidence().ref_evidence
            },
            ..complete_evidence()
        },
    ] {
        assert!(!essential_live_evidence_complete(&field));
    }
}
