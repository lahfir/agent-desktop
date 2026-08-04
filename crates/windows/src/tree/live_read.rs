//! The shared single-element live read and its five projections.
//!
//! Mirrors macOS `post_state.rs`'s single shared read: one function takes a
//! resolved `NativeHandle`, corroborates the verified process identity the
//! resolver stamped into the handle payload - a dead provider never
//! satisfies completeness, because a corpse's reads can succeed empty on some
//! builds (A14-9), reads the full walk property set live through
//! `properties::read_live`, and projects it through the walk's own vocabulary
//! composition (`read_label` + `walk_vocabulary` + `into_locator_evidence`).
//! The five readers are projections over that one read: value, state, actions,
//! element, bounds.

use agent_desktop_core::{
    AdapterError, ElementState, ErrorCode, LiveElement, LiveIdentity, LocatorEvidence,
    LocatorField, Rect,
};

use super::element_properties::ElementProperties;

/// One element's live read: the properties set kept alongside the projected
/// evidence, because `ElementState.enabled`/`offscreen` read `IsEnabled`/
/// `IsOffscreen` from the property set and `into_locator_evidence` does not
/// carry them.
pub(crate) struct LiveRead {
    pub(crate) properties: ElementProperties,
    pub(crate) evidence: LocatorEvidence,
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        LiveRead, essential_live_evidence_complete, incomplete_live_evidence, stale_reader_error,
    };
    use crate::tree::element::uia_element;
    use crate::tree::name_evidence::read_label;
    use crate::tree::properties::read_live;
    use crate::tree::walker::walk_vocabulary;
    use agent_desktop_core::{AdapterError, Deadline, NativeHandle, ProcessId};

    /// The shared single-element live read.
    ///
    /// Fails `STALE_REF`-class when the verified process token has moved on
    /// (the dead-provider shape, driven through a dead token), and fails
    /// retryable `AppUnresponsive` when an essential slot reads `Unknown` - it
    /// never answers with a partial bundle claiming completeness.
    pub fn read_live_element(
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<LiveRead, AdapterError> {
        let element = uia_element(handle)?;
        crate::system::permissions::ensure_budget(deadline)?;
        if let Some((pid, token)) = element.verified_process() {
            if !crate::system::process_identity::matches_instance(ProcessId::new(pid), token)? {
                return Err(stale_reader_error());
            }
        }
        if deadline.is_expired() {
            return Err(deadline.timeout_error());
        }
        let (properties, _errors) = read_live(element);
        let label = read_label(element, false);
        let vocabulary = walk_vocabulary(&properties, &label);
        let evidence = properties.clone().into_locator_evidence(vocabulary);
        if !essential_live_evidence_complete(&evidence) {
            return Err(incomplete_live_evidence());
        }
        Ok(LiveRead {
            properties,
            evidence,
        })
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::LiveRead;
    use agent_desktop_core::{AdapterError, Deadline, NativeHandle};

    /// Canned twin so the adapter compiles on a non-Windows lane; there are no
    /// live elements there, so every live read is refused rather than faked.
    pub fn read_live_element(
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<LiveRead, AdapterError> {
        Err(AdapterError::not_supported("get_live_*"))
    }
}

pub(crate) use imp::read_live_element;

/// The value projection: a secure field's stored value is `Absent` here by
/// the shared read's own withholding, so nothing secure escapes.
pub(crate) fn live_value(read: &LiveRead) -> Option<String> {
    read.evidence.value.known().cloned()
}

/// The state projection.
///
/// `enabled` and `offscreen` read the provider's own flags from the property
/// set - UIA exposes `IsOffscreen` directly, the deliberate divergence from
/// macOS's window-bounds arithmetic. `hidden` has no Windows producer (UIA's
/// offscreen signal is the closest, and reading it twice would double-count), so it
/// stays unset.
pub(crate) fn live_state(read: &LiveRead) -> Result<ElementState, AdapterError> {
    let role = read
        .evidence
        .role
        .known()
        .cloned()
        .ok_or_else(incomplete_live_evidence)?;
    let states = read.evidence.states.known().cloned().unwrap_or_default();
    let value = live_value(read);
    let enabled = read
        .properties
        .get(super::property_ids::TreeProperty::IsEnabled)
        .flag();
    let offscreen = read
        .properties
        .get(super::property_ids::TreeProperty::IsOffscreen)
        .flag();
    Ok(ElementState {
        role,
        states,
        value,
        enabled,
        hidden: None,
        offscreen,
    })
}

/// The actions projection (a free projection, no pattern invocation).
pub(crate) fn live_actions(read: &LiveRead) -> Result<Vec<String>, AdapterError> {
    known_actions(read.evidence.ref_evidence.available_actions.clone())
}

/// The element projection: identity, state, bounds and actions all off the
/// one shared read.
pub(crate) fn live_element(read: &LiveRead) -> Result<LiveElement, AdapterError> {
    let state = live_state(read)?;
    let identity = LiveIdentity {
        name: read.evidence.name.clone(),
        description: read.evidence.description.clone(),
        identifiers: read.evidence.identifiers.clone(),
    };
    let available_actions = known_actions(read.evidence.ref_evidence.available_actions.clone())?;
    let bounds = read.evidence.ref_evidence.bounds.known().copied();
    Ok(LiveElement {
        identity,
        state,
        states_complete: true,
        bounds,
        available_actions,
    })
}

/// The bounds projection, with the same essential-completeness discipline as
/// the other readers: an unknown bounds read fails the shared read first.
pub(crate) fn live_bounds(read: &LiveRead) -> Option<Rect> {
    read.evidence.ref_evidence.bounds.known().copied()
}

fn known_actions(actions: LocatorField<Vec<String>>) -> Result<Vec<String>, AdapterError> {
    match actions {
        LocatorField::Known(actions) => Ok(actions),
        LocatorField::Absent => Ok(Vec::new()),
        LocatorField::Unknown => Err(incomplete_live_evidence()),
    }
}

fn essential_live_evidence_complete(evidence: &LocatorEvidence) -> bool {
    !evidence.role.is_unknown()
        && !evidence.value.is_unknown()
        && !evidence.states.is_unknown()
        && !evidence.ref_evidence.bounds.is_unknown()
        && !evidence.ref_evidence.available_actions.is_unknown()
}

fn incomplete_live_evidence() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Live element evidence was incomplete",
    )
    .with_details(serde_json::json!({
        "kind": "live_element_evidence",
        "complete": false,
        "retryable": true,
    }))
}

fn stale_reader_error() -> AdapterError {
    AdapterError::stale_ref("Element became invalid while reading live state").with_details(
        serde_json::json!({
            "kind": "live_element_invalid",
            "complete": true,
        }),
    )
}

#[cfg(all(test, target_os = "windows"))]
#[path = "live_read_tests.rs"]
mod tests;
/// The essential-completeness gate is a pure predicate over the evidence, so
/// it is pinned without a UI Automation client: any essential slot reading
/// `Unknown` - the completeness rule - rejects the bundle, and the
/// rejection error is the retryable `AppUnresponsive` the loop retries
/// instead of a partial answer claiming completeness.
#[cfg(test)]
mod completeness_tests {
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
}