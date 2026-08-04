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
//!
//! The process corroboration runs twice, not once: a first, cheap check
//! before the read rejects an already-dead handle without paying for a
//! property fetch, and a second, authoritative check after the read and its
//! completeness gate rejects a handle whose process died *during* the read -
//! the corpse case A14-9 measured, where the dead provider's reads still
//! return success. Only the second check's answer is trusted to gate the
//! `Ok`; the first is an optimization, not a substitute. Separately, the
//! read's own discarded errors are inspected for the vanished-element
//! disposition before the completeness gate ever sees them, so a resolved
//! target that reports `UIA_E_ELEMENTNOTAVAILABLE` settles as stale rather
//! than surfacing as a retryable, indefinitely-repeated `AppUnresponsive`.

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
        target_read_reports_vanished,
    };
    use crate::tree::element::{UIAElement, uia_element};
    use crate::tree::name_evidence::read_label;
    use crate::tree::properties::{ElementProperties, read_live_bounded};
    use crate::tree::walker::walk_vocabulary;
    use agent_desktop_core::{AdapterError, Deadline, NativeHandle, ProcessId};

    /// The shared single-element live read.
    ///
    /// Fails `STALE_REF`-class when the verified process token has moved on
    /// either before or after the read (the dead-provider shape, driven
    /// through a dead token), fails `STALE_REF`-class when the read itself
    /// reports the resolved target vanished, and fails retryable
    /// `AppUnresponsive` when an essential slot reads `Unknown` - it never
    /// answers with a partial bundle claiming completeness.
    ///
    /// Wires `properties::read_live_bounded` rather than the unbounded
    /// `read_live`: the property set is 42 properties wide, one cross-process
    /// call apiece, and nothing checked the operation deadline between them -
    /// a poll-style caller (`resolve.rs`'s retry loop, core's ref-action poll)
    /// could overshoot its own user-visible timeout on every single iteration.
    /// `read_live_bounded` consults the same deadline between property reads
    /// and truncates the set rather than running past it; a truncated read
    /// still reaches the completeness gate below, which is the established
    /// incomplete/retryable shape for "did not finish in time" - no new
    /// timing abstraction, no change to what a successful read returns.
    pub fn read_live_element(
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<LiveRead, AdapterError> {
        let element = uia_element(handle)?;
        read_live_element_core(element, deadline, corroborate_verified_process, |element| {
            read_live_bounded(element, deadline)
        })
    }

    /// The read's body, generic over how process-instance corroboration and
    /// the property read itself are performed.
    ///
    /// Production always passes the real corroborator and the real
    /// `properties::read_live`; a test substitutes one or the other to drive
    /// a specific failure deterministically - a fake corroborator that
    /// answers live once and dead the next call stands in for a process that
    /// dies between the pre-read and post-read checks (A14-9's corpse timing,
    /// which a real process kill cannot reproduce without a race), and a
    /// fake read stands in for a resolved target that has vanished.
    pub(crate) fn read_live_element_core(
        element: &UIAElement,
        deadline: Deadline,
        corroborate: impl Fn(&UIAElement) -> Result<(), AdapterError>,
        read: impl Fn(&UIAElement) -> (ElementProperties, Vec<AdapterError>),
    ) -> Result<LiveRead, AdapterError> {
        crate::system::permissions::ensure_budget(deadline)?;
        corroborate(element)?;
        if deadline.is_expired() {
            return Err(deadline.timeout_error());
        }
        let (properties, errors) = read(element);
        if target_read_reports_vanished(&errors) {
            return Err(stale_reader_error());
        }
        let label = read_label(element, false);
        let vocabulary = walk_vocabulary(&properties, &label);
        let evidence = properties.clone().into_locator_evidence(vocabulary);
        if !essential_live_evidence_complete(&evidence) {
            return Err(incomplete_live_evidence());
        }
        corroborate(element)?;
        Ok(LiveRead {
            properties,
            evidence,
        })
    }

    /// Corroborates the handle's verified process token against the
    /// process's live generation, right now.
    ///
    /// Not memoized: called again after the read, it genuinely re-queries the
    /// OS rather than trusting the answer the pre-read call already gave,
    /// which is what lets the post-read call catch a process that died
    /// during the read.
    pub(crate) fn corroborate_verified_process(element: &UIAElement) -> Result<(), AdapterError> {
        if let Some((pid, token)) = element.verified_process() {
            if !crate::system::process_identity::matches_instance(ProcessId::new(pid), token)? {
                return Err(stale_reader_error());
            }
        }
        Ok(())
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
    let available_actions = live_actions(read)?;
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

/// Reports whether the read's own discarded errors already settled the
/// resolved target as vanished.
///
/// Keyed on `error.code` alone, deliberately not on a code-plus-retryable-flag
/// coincidence. Every entry in the vector this function inspects comes from
/// `properties::read_live_bounded`, and that vector can only ever be built
/// two ways: a property-read failure, always classified through
/// `uia_failure_error` - which decides the code through
/// `hresult::hresult_record` (HRESULT branch) or `automation_classify.rs`'s
/// `sentinel_record` (sentinel branch), the only two places `StaleRef` can
/// enter this vector, each naming it for exactly one family
/// (`UIA_E_ELEMENTNOTAVAILABLE`, `ERR_INVALID_OBJECT`), both
/// `ReadDisposition::Unavailable` and exhaustively pinned by
/// `hresult_tests.rs` and `automation_classify.rs`'s own test module - or the
/// deadline's own `timeout_error()` push on truncation, which is always
/// `ErrorCode::Timeout` and never reaches `uia_failure_error` at all. Neither
/// path can produce a `StaleRef` that means anything else, so a `StaleRef`
/// code here is unambiguous. (The rest of this crate also constructs
/// `ErrorCode::StaleRef` directly - `resolve_match.rs`'s `stale_ref_error`,
/// this module's own `stale_reader_error`, `surfaces.rs` - but none of those
/// errors can reach this vector, since it only ever collects property-read
/// failures and the deadline's truncation stamp.) The check also does not
/// need to lean on whether a later `.with_details` call (as
/// `properties::property_read_error` makes) preserves a typed retryable
/// flag: `AdapterError::with_details` only touches `retryability` when the
/// new details carry their own `retryable` key, so a wrap that omits one -
/// `property_read_error`'s does - leaves whatever stamp was already there
/// unchanged, never loses it.
fn target_read_reports_vanished(errors: &[AdapterError]) -> bool {
    errors.iter().any(|error| error.code == ErrorCode::StaleRef)
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

/// Split from `live_read_tests.rs` to keep both files under the crate's
/// per-file line cap: this module owns the one test that mutates the
/// fixture's content control cross-process, plus the Win32 lookup helpers
/// that only that test needs.
#[cfg(all(test, target_os = "windows"))]
#[path = "live_read_edit_tests.rs"]
mod edit_tests;
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
