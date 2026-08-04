use super::*;
use agent_desktop_core::{ElementIdentifier, LocatorEvidence, RefEntry, WindowInfo};

fn first_identifier(evidence: &LocatorEvidence) -> Option<ElementIdentifier> {
    evidence
        .identifiers
        .identifiers()
        .iter()
        .find(|identifier| {
            matches!(
                identifier.kind,
                agent_desktop_core::IdentifierKind::AutomationId
            )
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
    let mut ignored = false;
    let children =
        crate::tree::resolve_search::enumerate_children(source, element, budget, &mut ignored)
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
            crate::system::process_identity::token_for_pid(agent_desktop_core::ProcessId::from(
                fixture.process_id(),
            ))
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

    let mut prefix = Vec::new();
    let found = find_password(&source, &prepared, 0, &budget, &mut prefix)
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
    let mut ignored = false;
    let children =
        crate::tree::resolve_search::enumerate_children(source, element, budget, &mut ignored)?;
    for (index, child) in children.iter().enumerate() {
        prefix.push(index);
        if let Some(found) = find_password(source, child, depth + 1, budget, prefix)? {
            return Ok(Some(found));
        }
        prefix.pop();
    }
    Ok(None)
}

/// The cross-process takeover shape, end to end. The stored evidence here
/// genuinely describes a live element - captured from that very tree moments
/// earlier, and proven resolvable by the test above - while the stored
/// process is a *different* process that is alive and of exactly the stored
/// generation. Only the handle's live owner can tell the two apart, so every
/// resolver reaching a window root through `resolve_window_root` must settle
/// `STALE_REF` rather than find the element in an application the ref never
/// named.
#[test]
fn a_ref_whose_window_belongs_to_another_process_never_resolves_into_it() {
    crate::tree::fixture::ensure_test_apartment();
    let fixture = crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");
    let deadline = crate::tree::walker_fake::deadline();
    let root = crate::tree::automation::root_from_hwnd(fixture.handle(), deadline)
        .expect("the fixture window resolves");
    let source = UiaTreeSource::for_root(&root).expect("a tree source");
    let prepared = source.prepare_root(&root).expect("a prepared root");
    let budget = WalkBudget::new(10, deadline);

    let mut prefix = Vec::new();
    let (path, _, evidence, _) = find_password(&source, &prepared, 0, &budget, &mut prefix)
        .expect("the fixture exposes a password edit")
        .expect("a password element");
    let rect = evidence.ref_evidence.bounds.known().expect("a bounds");
    let hash = rect.bounds_hash().expect("a positive-area hash");

    let pid = agent_desktop_core::ProcessId::from(std::process::id());
    let token = crate::system::process_identity::token_for_pid(pid)
        .expect("the token read answers")
        .expect("a live process has a token");
    assert_ne!(
        pid,
        agent_desktop_core::ProcessId::from(fixture.process_id()),
        "the window under test belongs to a different process than the stored ref"
    );
    assert!(
        crate::system::process_identity::matches_instance(pid, &token)
            .expect("the generation check answers"),
        "the stored process is alive and of the stored generation, so only ownership refutes"
    );

    let entry = RefEntry {
        process: agent_desktop_core::RefProcess {
            pid,
            process_instance: Some(token),
        },
        identity: agent_desktop_core::RefEntryIdentity {
            role: evidence.role.known().cloned().unwrap_or_default(),
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
            source_app: None,
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

    let verdicts = [
        (
            "the window gate itself",
            resolve_window_root(&entry, deadline).err().map(code_of),
        ),
        (
            "the strict resolver",
            resolve_element_strict(&entry, deadline).err().map(code_of),
        ),
        (
            "the locator anchor",
            crate::tree::resolve_anchor::resolve_locator_anchor(&entry, deadline)
                .err()
                .map(code_of),
        ),
    ];
    assert_eq!(
        verdicts.to_vec(),
        vec![
            ("the window gate itself", Some(ErrorCode::StaleRef)),
            ("the strict resolver", Some(ErrorCode::StaleRef)),
            ("the locator anchor", Some(ErrorCode::StaleRef)),
        ],
        "no resolver may reach into a window another process owns"
    );
}

fn code_of(error: AdapterError) -> ErrorCode {
    error.code
}

#[path = "resolve_retry_tests.rs"]
mod retry;
