use agent_desktop_core::{
    AdapterError, Deadline, ElementIdentifier, IdentifierKind, LocatorField, NativeHandle, Rect,
    RefEntry,
};
use serde_json::json;

use super::element::UIAElement;
#[cfg(target_os = "windows")]
use super::properties::read_live;
#[cfg(target_os = "windows")]
use super::property_ids::TreeProperty;
use super::walker::{DEFAULT_MAX_SIBLINGS, TreeSource, WalkBudget};
use super::walker_source::UiaTreeSource;

/// The resolve-scoped depth cap (KTD9).
///
/// Independently bounded from the walk's own ceiling, mirroring macOS's
/// `MAX_RESOLVE_DEPTH` (`crates/macos/src/tree/resolve.rs:15`, a distinct
/// constant).
const MAX_RESOLVE_DEPTH: u8 = 50;

/// The identity evidence one element carries, compared against the stored ref.
///
/// Split from [`Candidate`] so the matching decision is a pure function over
/// plain values, testable without a live COM element in scope.
#[derive(Clone)]
struct NodeIdentity {
    native_id: Option<ElementIdentifier>,
    role: Option<String>,
    name: Option<String>,
    bounds_hash: Option<u64>,
}

/// A candidate match: the live element plus the identity evidence read from it.
#[derive(Clone)]
struct Candidate {
    element: UIAElement,
    identity: NodeIdentity,
}

/// Resolves a stored ref to its live element, fail-closed (KTD9).
///
/// The search descends from the stored window's root (the source window the
/// ref was taken from) to a resolve-scoped depth, collecting every element
/// whose `native_id` matches the stored one by kind and value, corroborated by
/// role and the role-conditional stable text identity. Then:
///
/// - zero candidates → `STALE_REF`
/// - two or more candidates that all match → `AMBIGUOUS_TARGET`
/// - exactly one → a `NativeHandle` wrapping the live element
///
/// Anything short of an exact match fails closed rather than guessing, because
/// A7-3 measured Explorer re-resolving 29 of 29 `AutomationId` keys with 5
/// landing on a different element - the silent-wrong-target shape strictness
/// exists to prevent.
#[cfg(target_os = "windows")]
pub(crate) fn resolve_element_strict(
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    let root = resolve_window_root(entry, deadline)?;
    let source = UiaTreeSource::for_root(&root)?;
    let prepared = source.prepare_root(&root)?;

    let mut searched = Vec::new();
    let budget = WalkBudget::new(MAX_RESOLVE_DEPTH, deadline)
        .with_max_raw_depth(MAX_RESOLVE_DEPTH)
        .with_max_siblings(DEFAULT_MAX_SIBLINGS);
    search_under(&source, &prepared, 0, &budget, &mut searched)?;

    let expected_id = entry.identity.native_id.as_ref();
    let expected_role = entry.identity.role.as_str();
    let expected_name = entry.identity.name.as_deref();

    let matches: Vec<Candidate> = searched
        .into_iter()
        .filter(|candidate| {
            candidate_matches(
                &candidate.identity,
                expected_id,
                expected_role,
                expected_name,
            )
        })
        .collect();

    match matches.len() {
        0 => Err(stale_ref_error(entry)),
        1 => matches.into_iter().next().map_or_else(
            || Err(stale_ref_error(entry)),
            |Candidate { element, .. }| Ok(element.into_native_handle()),
        ),
        _ => {
            // The bounds hash is the soft signal (KTD9): it never refutes an
            // exact match, but when several elements match the immutable
            // identity and exactly one also occupied the stored bounds, that
            // one is the window the ref was taken from. Without that single
            // corroboration, fail closed as ambiguous rather than guess.
            let expected_hash = entry.geometry.bounds_hash;
            let hash_matches = matches
                .iter()
                .filter(|candidate| candidate.identity.bounds_hash == expected_hash)
                .count();
            if hash_matches == 1 {
                if let Some(sole) = matches
                    .iter()
                    .find(|candidate| candidate.identity.bounds_hash == expected_hash)
                {
                    return Ok(sole.element.clone().into_native_handle());
                }
            }
            Err(ambiguous_target_error(entry, matches.len()))
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

/// Whether one searched element matches the stored evidence exactly.
///
/// `native_id` must match by kind **and** value when the ref carries one; role
/// must match when the ref records a role other than `unknown`; name is
/// corroboration only when the ref records a name and the element reports one
/// (a blank live name cannot refute a ref that had no name). The bounds hash,
/// when both sides have one, is a soft signal - it never refutes, it only
/// corroborates, per the identity-against-OS-reorder practice.
fn candidate_matches(
    identity: &NodeIdentity,
    expected_id: Option<&ElementIdentifier>,
    expected_role: &str,
    expected_name: Option<&str>,
) -> bool {
    if let Some(expected) = expected_id {
        let id_matches = identity
            .native_id
            .as_ref()
            .is_some_and(|actual| actual.kind == expected.kind && actual.value == expected.value);
        if !id_matches {
            return false;
        }
    }
    if !expected_role.is_empty() && expected_role != "unknown" {
        if identity.role.as_deref() != Some(expected_role) {
            return false;
        }
    }
    if let Some(name) = expected_name {
        if !name.is_empty() && identity.name.as_deref() != Some(name) {
            return false;
        }
    }
    true
}

/// Reaches the stored window's root element from the ref's source window id.
#[cfg(target_os = "windows")]
fn resolve_window_root(entry: &RefEntry, deadline: Deadline) -> Result<UIAElement, AdapterError> {
    let window_id = entry
        .source
        .source_window_id
        .as_deref()
        .ok_or_else(|| stale_ref_error(entry))?;
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

/// Searches the subtree under `element` to the resolve depth, collecting every
/// node's identity evidence.
#[cfg(target_os = "windows")]
fn search_under(
    source: &UiaTreeSource,
    element: &UIAElement,
    depth: u8,
    budget: &WalkBudget,
    out: &mut Vec<Candidate>,
) -> Result<(), AdapterError> {
    if depth >= MAX_RESOLVE_DEPTH {
        return Ok(());
    }
    crate::system::permissions::ensure_budget(budget.deadline)?;

    out.push(read_candidate(element));

    let children = enumerate_children(source, element, budget)?;
    for child in children {
        search_under(source, &child, depth + 1, budget, out)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn enumerate_children(
    source: &UiaTreeSource,
    element: &UIAElement,
    budget: &WalkBudget,
) -> Result<Vec<UIAElement>, AdapterError> {
    let mut children = Vec::new();
    let mut current = match source.first_child(element) {
        Ok(first) => first,
        Err(failure) if failure.is_exhaustion() => return Ok(children),
        Err(failure) => {
            return Err(super::automation::uia_failure_error(
                failure,
                "descend to a stored ref",
            ));
        }
    };
    loop {
        if children.len() >= budget.max_siblings {
            break;
        }
        let next = source.next_sibling(&current);
        children.push(current);
        match next {
            Ok(sibling) => current = sibling,
            Err(failure) if failure.is_exhaustion() => break,
            Err(failure) => {
                return Err(super::automation::uia_failure_error(
                    failure,
                    "walk a stored ref's siblings",
                ));
            }
        }
    }
    Ok(children)
}

/// Reads the identity-bearing evidence off one element for comparison, from a
/// single batched read of the walk property set.
#[cfg(target_os = "windows")]
fn read_candidate(element: &UIAElement) -> Candidate {
    let (properties, _) = read_live(element);
    let role = crate::tree::roles::resolve_role(&properties)
        .known()
        .cloned();
    Candidate {
        element: element.clone(),
        identity: NodeIdentity {
            native_id: match properties.get(TreeProperty::AutomationId).text() {
                LocatorField::Known(value) if !value.trim().is_empty() => Some(ElementIdentifier {
                    kind: IdentifierKind::AutomationId,
                    value,
                }),
                _ => None,
            },
            role,
            name: match properties.get(TreeProperty::Name).text() {
                LocatorField::Known(value) if !value.trim().is_empty() => Some(value),
                _ => None,
            },
            bounds_hash: properties
                .get(TreeProperty::BoundingRectangle)
                .bounds()
                .known()
                .and_then(Rect::bounds_hash),
        },
    }
}

fn stale_ref_error(_entry: &RefEntry) -> AdapterError {
    AdapterError::stale_ref("Stored ref does not match any live element").with_details(json!({
        "kind": "resolve_no_candidate",
    }))
}

fn ambiguous_target_error(_entry: &RefEntry, count: usize) -> AdapterError {
    AdapterError::ambiguous_target("Multiple live elements match the stored identity")
        .with_suggestion("Take a fresh snapshot and use its new ref")
        .with_details(json!({
            "kind": "resolve_ambiguous",
            "candidate_count": count,
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::ErrorCode;

    #[test]
    fn a_zero_candidate_resolution_is_stale() {
        let error = stale_ref_error(&RefEntry {
            process: agent_desktop_core::RefProcess {
                pid: agent_desktop_core::ProcessId::new(1),
                process_instance: None,
            },
            identity: agent_desktop_core::RefEntryIdentity {
                role: "button".into(),
                name: None,
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
                source_window_id: Some("w-1".into()),
                source_window_title: None,
                source_window_bounds_hash: None,
                source_surface: agent_desktop_core::SnapshotSurface::Window,
            },
            scope: agent_desktop_core::RefScope {
                root_ref: None,
                path_is_absolute: false,
                path: agent_desktop_core::refs::RefPath::default(),
            },
        });
        assert_eq!(error.code, ErrorCode::StaleRef);
    }

    fn identity(native: Option<&str>, role: &str, name: Option<&str>) -> NodeIdentity {
        NodeIdentity {
            native_id: native.map(|value| ElementIdentifier {
                kind: IdentifierKind::AutomationId,
                value: value.to_string(),
            }),
            role: if role.is_empty() {
                None
            } else {
                Some(role.to_string())
            },
            name: name.map(str::to_string),
            bounds_hash: None,
        }
    }

    fn id(expected: &str) -> ElementIdentifier {
        ElementIdentifier {
            kind: IdentifierKind::AutomationId,
            value: expected.to_string(),
        }
    }

    /// The A7-3 silent-wrong-target pin (KTD9): two elements sharing a
    /// `native_id` but with different roles must not both match a ref that
    /// records the role. Weakening the evidence check to id-only would let a
    /// wrong element resolve - the exact shape A7-3 measured on Explorer.
    #[test]
    fn a_matching_native_id_with_a_mismatched_role_does_not_resolve() {
        let stored = identity(Some("row-77"), "row", Some("Alpha"));
        let other_row = identity(Some("row-77"), "option", Some("Beta"));

        assert!(!candidate_matches(
            &other_row,
            Some(&id("row-77")),
            "row",
            Some("Alpha")
        ));
        assert!(candidate_matches(
            &stored,
            Some(&id("row-77")),
            "row",
            Some("Alpha")
        ));
    }

    /// A ref that records no native_id (the Electron shape, A7-1) cannot be
    /// pinned by id; with no corroborating role/name either, it matches a
    /// blank candidate, and the caller resolves `AMBIGUOUS` over the search.
    #[test]
    fn a_native_id_less_ref_never_matches_by_id_alone() {
        let stored = identity(None, "", None);
        assert!(candidate_matches(&stored, None, "", None));
    }

    #[cfg(target_os = "windows")]
    mod windows_only {
        use super::*;
        use agent_desktop_core::{RefEntry, WindowInfo};

        /// The live half of the resolver: the fixture's tree is walked, one
        /// element carrying a non-empty `AutomationId` is captured, and a ref
        /// built from that exact evidence resolves back to a live element -
        /// the `snapshot` → pick ref → `--root` drill-down loop, across a real
        /// process boundary.
        #[test]
        fn a_fixture_ref_resolves_to_the_same_element_end_to_end() {
            crate::tree::fixture::ensure_test_apartment();
            let fixture =
                crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");
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

            let captured =
                capture_identified(&root, deadline).expect("a fixture element has an id");

            let entry = RefEntry {
                process: agent_desktop_core::RefProcess {
                    pid: window.pid,
                    process_instance: Some(token),
                },
                identity: agent_desktop_core::RefEntryIdentity {
                    role: captured.role.clone().unwrap_or_default(),
                    name: captured.name.clone(),
                    value: None,
                    description: None,
                    native_id: captured.native_id.clone(),
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

        /// Walks the fixture and returns the first element carrying a
        /// non-empty `AutomationId`, with the identity read off it.
        fn capture_identified(
            root: &UIAElement,
            deadline: agent_desktop_core::Deadline,
        ) -> Option<NodeIdentity> {
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
        ) -> Option<NodeIdentity> {
            if depth >= 10 {
                return None;
            }
            let candidate = read_candidate(element);
            if candidate.identity.native_id.is_some() {
                return Some(candidate.identity);
            }
            let children = enumerate_children(source, element, budget).ok()?;
            for child in children {
                if let Some(found) = walk_for_identity(source, &child, depth + 1, budget) {
                    return Some(found);
                }
            }
            None
        }
    }
}
