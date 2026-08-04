use agent_desktop_core::{AdapterError, Deadline, NativeHandle, RefEntry};

#[cfg(target_os = "windows")]
use super::element::UIAElement;
#[cfg(target_os = "windows")]
use super::resolve_match::{CandidateOutcome, ambiguous_target_error};
use super::resolve_match::stale_ref_error;
#[cfg(target_os = "windows")]
use super::resolve_search::{
    MAX_RESOLVE_DEPTH, can_use_path_fast_path, element_at_path, geometry_matches,
    identity_unknown_error, search_under,
};
#[cfg(target_os = "windows")]
use super::walker::{DEFAULT_MAX_SIBLINGS, TreeSource, WalkBudget};
#[cfg(target_os = "windows")]
use super::walker_source::UiaTreeSource;

/// Resolves a stored ref to its live element, fail-closed and three-state.
///
/// The search descends from the stored window's root to a resolve-scoped
/// depth, reading each candidate with the same composition the walk uses
/// (`UiaTreeSource::evidence`), gates on role, and runs core's composed
/// identity rule (`resolve_match::candidate_outcome`). Then:
///
/// - zero candidates and every decision was readable -> `STALE_REF`, settled
/// - zero candidates but one was **unreadable** -> incomplete-and-retryable
///   (`AppUnresponsive`, the three-state discipline: an `Unknown` verdict is
///   never a `NoMatch`)
/// - two or more candidates that all match -> `AMBIGUOUS_TARGET`
/// - exactly one -> a `NativeHandle` wrapping the live element
///
/// Anything short of an exact match fails closed rather than guessing,
/// because A7-3 measured Explorer re-resolving 29 of 29 `AutomationId` keys
/// with 5 landing on a different element - the silent-wrong-target shape
/// strictness exists to prevent.
#[cfg(target_os = "windows")]
pub(crate) fn resolve_element_strict(
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    let root = resolve_window_root(entry, deadline)?;
    let source = UiaTreeSource::for_root(&root)?;
    let prepared = source.prepare_root(&root)?;

    let budget = WalkBudget::new(MAX_RESOLVE_DEPTH, deadline)
        .with_max_raw_depth(MAX_RESOLVE_DEPTH)
        .with_max_siblings(DEFAULT_MAX_SIBLINGS);

    // The path fast-path (see `resolve_search`): a locator, never identity.
    if can_use_path_fast_path(entry) {
        if let Some(candidate) = element_at_path(&source, &prepared, &entry.scope.path, &budget)?
        {
            let (_, evidence, _) = source.evidence(&candidate);
            let role_matches = evidence
                .role
                .known()
                .is_some_and(|role| role == &entry.identity.role);
            if role_matches {
                match super::resolve_match::candidate_outcome(entry, &evidence) {
                    CandidateOutcome::Matched => return Ok(candidate.into_native_handle()),
                    CandidateOutcome::Incomplete if geometry_matches(entry, &evidence) => {
                        return Ok(candidate.into_native_handle());
                    }
                    _ => {}
                }
            }
        }
    }

    let mut searched = Vec::new();
    let mut incomplete = false;
    search_under(
        &source,
        &prepared,
        0,
        &budget,
        entry,
        &mut searched,
        &mut incomplete,
    )?;

    match searched.len() {
        0 if incomplete => Err(identity_unknown_error(entry)),
        0 => Err(stale_ref_error(entry)),
        1 => {
            let Some(candidate) = searched.into_iter().next() else {
                return Err(stale_ref_error(entry));
            };
            Ok(candidate.element.into_native_handle())
        }
        _ => {
            let candidate_hashes: Vec<Option<u64>> = searched
                .iter()
                .map(|candidate| candidate.bounds_hash)
                .collect();
            match super::resolve_match::select_by_bounds_hash(
                &candidate_hashes,
                entry.geometry.bounds_hash,
            ) {
                super::resolve_match::Selection::Resolved(index) => {
                    Ok(searched[index].element.clone().into_native_handle())
                }
                super::resolve_match::Selection::Ambiguous => {
                    Err(ambiguous_target_error(entry, searched.len()))
                }
            }
        }
    }
}

/// The non-Windows twin. The crate cross-compiles to the Linux lane with the
/// resolver reachable, but there are no UI Automation elements there, so every
/// stored ref fails closed as stale rather than attempting a search that
/// cannot find anything.
#[cfg(not(target_os = "windows"))]
pub(crate) fn resolve_element_strict(
    entry: &RefEntry,
    _deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    Err(stale_ref_error(entry))
}

/// Reaches the stored window's root element from the ref's source window id.
///
/// The fail-closed process gate (the A7-3 wrong-target shape exists to
/// prevent): a stored ref must not search the tree of a different process that
/// has since recycled the HWND. The macOS resolver verifies process instance
/// before searching either. A token-less ref (elevated process whose token
/// could not be read) fails closed here rather than searching an unverified
/// window.
#[cfg(target_os = "windows")]
fn resolve_window_root(entry: &RefEntry, deadline: Deadline) -> Result<UIAElement, AdapterError> {
    let window_id = entry
        .source
        .source_window_id
        .as_deref()
        .ok_or_else(|| stale_ref_error(entry))?;
    if let Some(instance) = entry.process.process_instance.as_deref() {
        if !crate::system::process_identity::matches_instance(entry.process.pid, instance)? {
            return Err(stale_ref_error(entry));
        }
    } else {
        return Err(stale_ref_error(entry));
    }
    crate::tree::surfaces::surface_root(
        agent_desktop_core::ObservationRoot::Window(&agent_desktop_core::WindowInfo {
            id: window_id.to_string(),
            title: entry.source.source_window_title.clone().unwrap_or_default(),
            app: entry.source.source_app.clone().unwrap_or_default(),
            pid: entry.process.pid,
            process_instance: entry.process.process_instance.clone(),
            bounds: None,
            state: Default::default(),
        }),
        entry.source.source_surface,
        deadline,
    )
}

#[cfg(all(test, target_os = "windows"))]
mod windows_only {
    use super::*;
    use agent_desktop_core::{ElementIdentifier, LocatorEvidence, RefEntry, WindowInfo};

    fn first_identifier(evidence: &LocatorEvidence) -> Option<ElementIdentifier> {
        evidence
            .identifiers
            .identifiers()
            .iter()
            .find(|identifier| {
                matches!(identifier.kind, agent_desktop_core::IdentifierKind::AutomationId)
            })
            .cloned()
    }

    fn capture_identified(
        root: &UIAElement,
        deadline: agent_desktop_core::Deadline,
    ) -> Option<(Option<ElementIdentifier>, Option<String>, Option<String>)> {
        let source = UiaTreeSource::for_root(root).ok()?;
        let prepared = source.prepare_root(root).ok()?;
        let budget = WalkBudget::new(10, deadline);
        walk_for_identity(&source, &prepared, 0, &budget)
    }

    fn walk_for_identity(
        source: &UiaTreeSource,
        element: &UIAElement,
        depth: u8,
        budget: &WalkBudget,
    ) -> Option<(Option<ElementIdentifier>, Option<String>, Option<String>)> {
        if depth >= 10 {
            return None;
        }
        let (_, evidence, _) = source.evidence(element);
        let native_id = first_identifier(&evidence);
        if native_id.is_some() {
            return Some((
                native_id,
                evidence.role.known().cloned(),
                evidence.name.known().cloned(),
            ));
        }
        let children = crate::tree::resolve_search::enumerate_children(source, element, budget)
            .ok()?;
        for child in children {
            if let Some(found) = walk_for_identity(source, &child, depth + 1, budget) {
                return Some(found);
            }
        }
        None
    }

    #[test]
    fn a_fixture_ref_resolves_to_the_same_element_end_to_end() {
        crate::tree::fixture::ensure_test_apartment();
        let fixture = crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");
        let window = WindowInfo {
            id: format!("w-{}", fixture.handle()),
            title: "agent-desktop fixture".into(),
            app: "fixture.exe".into(),
            pid: agent_desktop_core::ProcessId::from(fixture.process_id()),
            process_instance: Some(
                crate::system::process_identity::token_for_pid(
                    agent_desktop_core::ProcessId::from(fixture.process_id()),
                )
                .unwrap()
                .expect("a live fixture process has a token"),
            ),
            bounds: None,
            state: Default::default(),
        };
        let deadline = crate::tree::walker_fake::deadline();
        let root = crate::tree::automation::root_from_hwnd(fixture.handle(), deadline)
            .expect("the fixture window resolves");
        let token = window.process_instance.clone().unwrap();

        let captured = capture_identified(&root, deadline).expect("a fixture element has an id");

        let entry = RefEntry {
            process: agent_desktop_core::RefProcess {
                pid: window.pid,
                process_instance: Some(token),
            },
            identity: agent_desktop_core::RefEntryIdentity {
                role: captured.1.clone().unwrap_or_default(),
                name: captured.2.clone(),
                value: None,
                description: None,
                native_id: captured.0.clone(),
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
                source_app: Some("fixture.exe".into()),
                source_window_id: Some(window.id.clone()),
                source_window_title: None,
                source_window_bounds_hash: None,
                source_surface: agent_desktop_core::SnapshotSurface::Window,
            },
            scope: agent_desktop_core::RefScope {
                root_ref: None,
                path_is_absolute: false,
                path: agent_desktop_core::refs::RefPath::default(),
            },
        };

        let handle = resolve_element_strict(&entry, deadline)
            .expect("the stored identity re-resolves to a live element");

        assert!(
            handle.downcast_ref::<UIAElement>().is_some(),
            "the resolved handle carries a UI Automation element"
        );
    }

    /// A ref taken from the fixture's password control - no text identity,
    /// positive-area bounds, secure content withheld - resolves through the
    /// path fast-path and the geometry tier on an unchanged tree, and the
    /// secure value reaches no error or detail.
    #[test]
    fn a_blank_secure_ref_resolves_through_the_path_and_geometry_tier() {
        crate::tree::fixture::ensure_test_apartment();
        let fixture = crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");
        let deadline = crate::tree::walker_fake::deadline();
        let root = crate::tree::automation::root_from_hwnd(fixture.handle(), deadline)
            .expect("the fixture window resolves");
        let source = UiaTreeSource::for_root(&root).expect("a tree source");
        let prepared = source.prepare_root(&root).expect("a prepared root");
        let budget = WalkBudget::new(10, deadline);

        // Locate the password edit: an unlabelled, id-less EDIT whose value is
        // withheld by the secure gate, and record its child-index path.
        let mut prefix = Vec::new();
        let found = find_password(
            &source,
            &prepared,
            0,
            &budget,
            &mut prefix,
        )
        .expect("the fixture exposes a password edit")
        .expect("a password element");
        let (path, _, evidence, _) = found;
        let role = evidence.role.known().cloned();
        let rect = evidence.ref_evidence.bounds.known().expect("a bounds");
        let hash = rect.bounds_hash().expect("a positive-area hash");

        let entry = RefEntry {
            process: agent_desktop_core::RefProcess {
                pid: agent_desktop_core::ProcessId::from(fixture.process_id()),
                process_instance: Some(
                    crate::system::process_identity::token_for_pid(
                        agent_desktop_core::ProcessId::from(fixture.process_id()),
                    )
                    .unwrap()
                    .expect("a live fixture process has a token"),
                ),
            },
            identity: agent_desktop_core::RefEntryIdentity {
                role: role.clone().unwrap_or_default(),
                name: None,
                value: None,
                description: None,
                native_id: None,
            },
            geometry: agent_desktop_core::RefGeometry {
                bounds: Some(*rect),
                bounds_hash: Some(hash),
            },
            capabilities: agent_desktop_core::RefCapabilities {
                states: Vec::new(),
                available_actions: Vec::new(),
            },
            source: agent_desktop_core::RefSource {
                source_app: Some("fixture.exe".into()),
                source_window_id: Some(format!("w-{}", fixture.handle())),
                source_window_title: None,
                source_window_bounds_hash: None,
                source_surface: agent_desktop_core::SnapshotSurface::Window,
            },
            scope: agent_desktop_core::RefScope {
                root_ref: None,
                path_is_absolute: true,
                path,
            },
        };

        assert!(
            crate::tree::resolve_search::can_use_path_fast_path(&entry),
            "a window-rooted path with a positive-area hash qualifies"
        );
        assert!(
            crate::tree::resolve_search::provisional_geometry_candidate(&entry),
            "no meaningful identity plus a positive-area hash is promotion-eligible"
        );

        let handle = resolve_element_strict(&entry, deadline)
            .expect("the blank secure ref resolves through path and geometry");

        assert!(
            handle.downcast_ref::<UIAElement>().is_some(),
            "the resolved handle carries a UI Automation element"
        );
    }

    fn find_password(
        source: &UiaTreeSource,
        element: &UIAElement,
        depth: u8,
        budget: &WalkBudget,
        prefix: &mut Vec<usize>,
    ) -> Result<
        Option<(
            agent_desktop_core::refs::RefPath,
            crate::tree::properties::ElementProperties,
            LocatorEvidence,
            u64,
        )>,
        AdapterError,
    > {
        if depth >= 10 {
            return Ok(None);
        }
        let (properties, node_evidence, failed) = source.evidence(element);
        if properties.is_secure() {
            let mut path = agent_desktop_core::refs::RefPath::default();
            path.extend_from_slice(prefix);
            return Ok(Some((path, properties, node_evidence, failed)));
        }
        let children = crate::tree::resolve_search::enumerate_children(source, element, budget)?;
        for (index, child) in children.iter().enumerate() {
            prefix.push(index);
            if let Some(found) = find_password(source, child, depth + 1, budget, prefix)? {
                return Ok(Some(found));
            }
            prefix.pop();
        }
        Ok(None)
    }
}