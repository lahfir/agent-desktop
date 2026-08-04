use super::*;
use crate::tree::fixture::ensure_test_apartment;
use agent_desktop_core::{
    ElementIdentifier, IdentifierEvidence, IdentifierKind, LocatorField, NodeDescriptor,
};

fn entry(
    bounds: Option<agent_desktop_core::Rect>,
    hash: Option<u64>,
    name: Option<&str>,
    native: Option<&str>,
) -> RefEntry {
    RefEntry {
        process: agent_desktop_core::RefProcess {
            pid: agent_desktop_core::ProcessId::new(1),
            process_instance: None,
        },
        identity: agent_desktop_core::RefEntryIdentity {
            role: "button".to_string(),
            name: name.map(str::to_string),
            value: None,
            description: None,
            native_id: native.map(|value| ElementIdentifier {
                kind: IdentifierKind::AutomationId,
                value: value.to_string(),
            }),
        },
        geometry: agent_desktop_core::RefGeometry {
            bounds,
            bounds_hash: hash,
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

fn rect(width: f64, height: f64) -> agent_desktop_core::Rect {
    agent_desktop_core::Rect {
        x: 10.0,
        y: 10.0,
        width,
        height,
    }
}

fn evidence(bounds_hash: Option<u64>) -> LocatorEvidence {
    LocatorEvidence {
        role: LocatorField::Known("button".to_string()),
        name: LocatorField::Absent,
        value: LocatorField::Absent,
        description: LocatorField::Absent,
        identifiers: IdentifierEvidence::absent(),
        states: LocatorField::Absent,
        ref_evidence: agent_desktop_core::LocatorRefEvidence {
            bounds: match bounds_hash {
                Some(_) => LocatorField::Known(rect(40.0, 20.0)),
                None => LocatorField::Absent,
            },
            available_actions: LocatorField::Absent,
            descriptors: NodeDescriptor::default(),
        },
    }
}

#[test]
fn a_window_rooted_non_empty_path_with_identity_qualifies_for_the_fast_path() {
    let mut absolute = entry(None, Some(1), Some("name"), Some("id"));
    absolute.scope.path.push(2);
    assert!(can_use_path_fast_path(&absolute));
}

#[test]
fn a_relative_drill_down_path_skips_the_fast_path() {
    let mut relative = entry(None, Some(1), None, Some("id"));
    relative.scope.root_ref = Some("root".to_string());
    relative.scope.path_is_absolute = false;
    assert!(!can_use_path_fast_path(&relative));
}

#[test]
fn an_empty_path_skips_the_fast_path() {
    assert!(!can_use_path_fast_path(&entry(None, Some(1), None, None)));
}

#[test]
fn a_ref_with_no_id_text_or_hash_skips_the_fast_path() {
    assert!(!can_use_path_fast_path(&entry(None, None, None, None)));
}

#[test]
fn promotion_requires_a_positive_area_stored_bounds() {
    let positive = entry(Some(rect(40.0, 20.0)), Some(1), None, None);
    assert!(provisional_geometry_candidate(&positive));

    let zero_extent = entry(Some(rect(0.0, 0.0)), Some(1), None, None);
    assert!(!provisional_geometry_candidate(&zero_extent));

    let only_hash = entry(None, Some(1), None, None);
    assert!(!provisional_geometry_candidate(&only_hash));
}

#[test]
fn promotion_never_fires_when_the_entry_has_a_text_identity() {
    let named = entry(Some(rect(40.0, 20.0)), Some(1), Some("name"), None);
    assert!(!provisional_geometry_candidate(&named));

    let native = entry(Some(rect(40.0, 20.0)), Some(1), None, Some("id"));
    assert!(!provisional_geometry_candidate(&native));
}

#[test]
fn geometry_matches_only_on_the_live_bounds_hash() {
    let live_hash = rect(40.0, 20.0)
        .bounds_hash()
        .expect("a positive-area hash");
    let stored = entry(Some(rect(40.0, 20.0)), Some(live_hash), None, None);
    assert!(geometry_matches(&stored, &evidence(Some(live_hash))));
    assert!(!geometry_matches(&stored, &evidence(None)));
}

#[test]
fn geometry_matches_never_promotes_a_zero_extent_stored_hash() {
    let stored = entry(None, Some(0x1234), None, None);
    assert!(!geometry_matches(&stored, &evidence(Some(0x1234))));
}

#[test]
fn the_live_fixture_exposes_a_promotion_eligible_password_edit() {
    ensure_test_apartment();
    let fixture = crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");
    let source = UiaTreeSource::for_root(
        &crate::tree::automation::root_from_hwnd(
            fixture.handle(),
            crate::tree::walker_fake::deadline(),
        )
        .expect("the fixture resolves"),
    )
    .expect("a tree source");
    let prepared = source
        .prepare_root(
            &crate::tree::automation::root_from_hwnd(
                fixture.handle(),
                crate::tree::walker_fake::deadline(),
            )
            .expect("the fixture resolves"),
        )
        .expect("a prepared root");
    let budget = WalkBudget::new(10, crate::tree::walker_fake::deadline());
    let mut prefix = Vec::new();
    let found = find_secure(&source, &prepared, 0, &budget, &mut prefix)
        .expect("the fixture walk succeeds")
        .expect("a secure element exists");
    let (_, properties, evidence, _) = found;
    assert!(properties.is_secure());
    assert!(evidence.role.known().is_some());
}

fn find_secure(
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
    let children = enumerate_children(source, element, budget, &mut ignored)?;
    for (index, child) in children.iter().enumerate() {
        prefix.push(index);
        if let Some(found) = find_secure(source, child, depth + 1, budget, prefix)? {
            return Ok(Some(found));
        }
        prefix.pop();
    }
    Ok(None)
}
