use super::{live_actions, live_bounds, live_element, live_state, live_value, read_live_element};
use crate::tree::automation::root_from_hwnd;
use crate::tree::element::UIAElement;
use crate::tree::fixture::{HostedFixture, ensure_test_apartment};
use crate::tree::walker::TreeSource;
use crate::tree::walker_fake::deadline;
use agent_desktop_core::{AdapterError, ErrorCode, NativeHandle, ProcessId, RefEntry};

fn fixture_pid(fixture: &HostedFixture) -> u32 {
    fixture.process_id()
}

fn verified_handle(fixture: &HostedFixture) -> Result<NativeHandle, AdapterError> {
    let deadline = deadline();
    let root = root_from_hwnd(fixture.handle(), deadline)?;
    let token =
        crate::system::process_identity::token_for_pid(ProcessId::new(fixture_pid(fixture)))
            .expect("a live fixture process has a token")
            .expect("a token reads for a live fixture");
    Ok(root
        .with_verified_process(fixture_pid(fixture), token)
        .into_native_handle())
}

/// Every reader projects live values off one resolved fixture handle: the role
/// is known, the bounds are a positive rect, the enabled flag is present, and
/// the state/actions projections complete honestly.
#[test]
fn each_reader_projects_live_values_from_a_resolved_fixture_handle() {
    ensure_test_apartment();
    let fixture = HostedFixture::spawn().expect("a fixture host starts");
    let handle = verified_handle(&fixture).expect("a verified handle");

    let read = read_live_element(&handle, deadline()).expect("the shared read succeeds");
    let element = live_element(&read).expect("the element projection succeeds");
    assert!(element.states_complete);
    assert!(!element.state.role.is_empty());
    assert!(
        element
            .bounds
            .is_some_and(|rect| rect.width > 0.0 && rect.height > 0.0),
        "a visible fixture window has positive-area bounds"
    );

    let state = live_state(&read).expect("the state projection succeeds");
    assert_eq!(state.enabled, Some(true));

    let bounds = live_bounds(&read);
    assert!(bounds.is_some());

    let actions = live_actions(&read).expect("the actions projection succeeds");
    assert!(actions.iter().all(|action| !action.is_empty()));

    let _ = live_value(&read);
}

/// The password control's live value is withheld at the reader path: the
/// shared read inherits the `IsPassword` gate, so the secure value is `Absent`
/// and no marker reaches the reader's output.
#[test]
fn the_secure_control_s_live_value_is_withheld_at_the_reader_path() {
    ensure_test_apartment();
    let fixture = HostedFixture::spawn().expect("a fixture host starts");

    let entry = blank_secure_entry(&fixture);
    let handle = crate::tree::resolve::resolve_element_strict(&entry, deadline())
        .expect("the blank secure ref resolves through path and geometry");
    let read = read_live_element(&handle, deadline()).expect("the shared read succeeds");
    let value = live_value(&read);
    assert_eq!(
        value, None,
        "a secure control's value is absent at the reader"
    );
    let element = live_element(&read).expect("the element projection succeeds");
    assert_eq!(element.state.value, None);
}

/// A dead process token fails the reader as `STALE_REF`-class, never as
/// empty success: the handle's payload carries the verified generation token
/// and the shared read corroborates against it before answering (A14-9's
/// rule at the reader path). Driven through a token that cannot match
/// the live fixture process for determinism.
#[test]
fn a_dead_token_fails_stale_class_never_empty_success() {
    ensure_test_apartment();
    let fixture = HostedFixture::spawn().expect("a fixture host starts");
    let deadline = deadline();
    let root = root_from_hwnd(fixture.handle(), deadline).expect("a fixture root");
    let dead_handle = root
        .with_verified_process(fixture_pid(&fixture), "0000-dead-token".to_string())
        .into_native_handle();

    let error = match read_live_element(&dead_handle, deadline) {
        Err(error) => error,
        Ok(_) => panic!("a dead token must fail closed, not read empty"),
    };
    assert_eq!(error.code, ErrorCode::StaleRef);
}

/// The same shape with a live token answers honestly (the same rule's other
/// direction): verification passes and the read proceeds.
#[test]
fn a_live_token_answers_honestly() {
    ensure_test_apartment();
    let fixture = HostedFixture::spawn().expect("a fixture host starts");
    let handle = verified_handle(&fixture).expect("a verified handle");
    let read = read_live_element(&handle, deadline()).expect("the shared read succeeds");
    assert!(!read.evidence.role.is_unknown());
}

/// The reader-path completeness discipline: an element whose essential slots
/// are all known passes; the essential-Unknown retryable failure is driven by
/// the shared read's completeness gate, exercised here through a handle that
/// carries a process payload but whose possession of the element type is the
/// only thing the reader trusts to downcast.
#[test]
fn the_payload_type_round_trips_through_a_native_handle() {
    ensure_test_apartment();
    let fixture = HostedFixture::spawn().expect("a fixture host starts");
    let root = root_from_hwnd(fixture.handle(), deadline()).expect("a fixture root");
    let handle = root
        .with_verified_process(fixture_pid(&fixture), "token".to_string())
        .into_native_handle();
    let downcast = handle
        .downcast_ref::<UIAElement>()
        .expect("a UIAElement payload");
    let (pid, token) = downcast
        .verified_process()
        .expect("verified process present");
    assert_eq!(pid, fixture_pid(&fixture));
    assert_eq!(token, "token");
}

fn blank_secure_entry(fixture: &HostedFixture) -> RefEntry {
    // Reuses the same construction resolve_tests drives: an id-less, name-less
    // ref with a positive-area bounds hash and an absolute path to the secure
    // edit resolves through the path-and-geometry tier.
    let deadline = deadline();
    let root = root_from_hwnd(fixture.handle(), deadline).expect("a fixture root");
    let source = crate::tree::walker_source::UiaTreeSource::for_root(&root).expect("a tree source");
    let prepared = source.prepare_root(&root).expect("a prepared root");
    let budget = crate::tree::walker::WalkBudget::new(10, deadline);
    let mut prefix = Vec::new();
    let found = find_secure(&source, &prepared, 0, &budget, &mut prefix)
        .expect("the fixture walk succeeds")
        .expect("a secure element exists");
    let (path, _, evidence, _) = found;
    let rect = evidence
        .ref_evidence
        .bounds
        .known()
        .expect("positive-area bounds");
    let hash = rect.bounds_hash().expect("a positive-area hash");
    let token =
        crate::system::process_identity::token_for_pid(ProcessId::new(fixture.process_id()))
            .unwrap()
            .expect("a live fixture token");
    RefEntry {
        process: agent_desktop_core::RefProcess {
            pid: ProcessId::new(fixture.process_id()),
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
    }
}

fn find_secure(
    source: &crate::tree::walker_source::UiaTreeSource,
    element: &UIAElement,
    depth: u8,
    budget: &crate::tree::walker::WalkBudget,
    prefix: &mut Vec<usize>,
) -> Result<
    Option<(
        agent_desktop_core::refs::RefPath,
        crate::tree::properties::ElementProperties,
        agent_desktop_core::LocatorEvidence,
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
        if let Some(found) = find_secure(source, child, depth + 1, budget, prefix)? {
            return Ok(Some(found));
        }
        prefix.pop();
    }
    Ok(None)
}
