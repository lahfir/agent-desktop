//! How an attempt's two tiers compose into its verdict: either tier's unread
//! region alone withholds a negative one, and only a walk that read everything
//! may settle `STALE_REF`.

use super::*;

fn entry() -> RefEntry {
    RefEntry {
        process: agent_desktop_core::RefProcess {
            pid: agent_desktop_core::ProcessId::new(1),
            process_instance: None,
        },
        identity: agent_desktop_core::RefEntryIdentity {
            role: "button".to_string(),
            name: Some("Save".to_string()),
            value: None,
            description: None,
            native_id: None,
        },
        geometry: agent_desktop_core::RefGeometry {
            bounds: None,
            bounds_hash: None,
        },
        capabilities: agent_desktop_core::RefCapabilities {
            states: Vec::new(),
            available_actions: Vec::new(),
        },
        source: agent_desktop_core::RefSource {
            source_app: None,
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: agent_desktop_core::SnapshotSurface::Window,
        },
        scope: agent_desktop_core::RefScope {
            root_ref: None,
            path_is_absolute: true,
            path: agent_desktop_core::refs::RefPath::default(),
        },
    }
}

fn landing(unread_region: bool) -> PathLanding<crate::tree::element::UIAElement> {
    PathLanding {
        element: None,
        unread_region,
    }
}

/// The path tier is the only tier that descends past the broad search's depth
/// cap, so a gap it met can be the sole evidence that the tier able to reach
/// the stored element never got to look. An attempt that let that gap die with
/// the tier would answer `STALE_REF` - "the element is gone" - off a walk that
/// never finished.
#[test]
fn a_gap_on_the_stored_path_alone_withholds_the_negative_verdict() {
    assert!(
        matches!(
            attempt_verdict(&[], &landing(true), false, &entry()),
            SearchVerdict::Incomplete
        ),
        "a candidate-less search must not settle stale while the path walk left a region unread"
    );
}

/// The search's own gap withholds it by the same rule, so neither tier is
/// privileged and neither can be dropped in favour of the other.
#[test]
fn a_gap_in_the_broad_search_alone_withholds_the_negative_verdict() {
    assert!(matches!(
        attempt_verdict(&[], &landing(false), true, &entry()),
        SearchVerdict::Incomplete
    ));
}

/// The control both pins above depend on: with every region read and no
/// candidate found, the attempt does settle. Without this, withholding on an
/// unread region would be indistinguishable from never settling at all.
#[test]
fn a_walk_that_read_everything_and_found_nothing_settles_stale() {
    assert!(matches!(
        attempt_verdict(&[], &landing(false), false, &entry()),
        SearchVerdict::Stale
    ));
}

/// The positive verdict is withheld on the same evidence: a sole match found
/// while the path walk left a region unread could be the wrong one of two, and
/// the unread part is exactly where the second would be.
#[test]
fn a_sole_match_is_retried_while_the_path_walk_left_a_region_unread() {
    assert!(matches!(
        attempt_verdict(&[None], &landing(true), false, &entry()),
        SearchVerdict::Incomplete
    ));
    assert!(
        matches!(
            attempt_verdict(&[None], &landing(false), false, &entry()),
            SearchVerdict::Resolved(0)
        ),
        "the same sole match resolves once nothing is left unread"
    );
}
