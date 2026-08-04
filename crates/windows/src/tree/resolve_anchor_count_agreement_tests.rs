use crate::tree::fixture::{HostedFixture, ensure_test_apartment};
use crate::tree::walker_fake::deadline;
use agent_desktop_core::{
    LocatorMaterialization, LocatorResolveRequest, LocatorSelection, ObservationRoot, ProcessId,
    WindowInfo, commands::query::validate_selector, resolve_query,
};

fn fixture_window(fixture: &HostedFixture) -> WindowInfo {
    let pid = ProcessId::new(fixture.process_id());
    let token = crate::system::process_identity::token_for_pid(pid)
        .unwrap()
        .expect("a live fixture process has a token");
    WindowInfo {
        id: format!("w-{}", fixture.handle()),
        title: "agent-desktop fixture".into(),
        app: "fixture.exe".into(),
        pid,
        process_instance: Some(token),
        bounds: None,
        state: Default::default(),
    }
}

/// The dogfood report's own residual: "`find --count` vs materialized
/// agreement not asserted on a real app". A real application's count is
/// still off limits (the plan forbids naming one in CI), but the hosted
/// fixture is repo-controlled and deterministic, so it is exactly the
/// control this residual asks for. The fixture exposes two `textfield`
/// controls (the plain edit and the password field), so a real divergence
/// between the two selection modes - not a coincidental single-match count -
/// is what this pins.
#[test]
fn find_count_and_materialized_find_agree_on_the_same_query() {
    ensure_test_apartment();
    let fixture = HostedFixture::spawn().expect("a fixture host starts");
    let adapter = crate::adapter::WindowsAdapter::new();
    let window = fixture_window(&fixture);
    let query = validate_selector("textfield").expect("a role-only selector is valid");

    let counted = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window),
        &LocatorResolveRequest {
            selection: LocatorSelection::Count,
            deadline: deadline(),
            max_raw_depth: 50,
            materialization: LocatorMaterialization::None,
        },
    )
    .expect("count-mode resolution succeeds on the fixture");

    let materialized = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window),
        &LocatorResolveRequest {
            selection: LocatorSelection::All { limit: None },
            deadline: deadline(),
            max_raw_depth: 50,
            materialization: LocatorMaterialization::SelectedMatches,
        },
    )
    .expect("materialized resolution succeeds on the fixture");

    assert_eq!(
        counted.meta.total_matches, 2,
        "the fixture exposes exactly its plain and password textfields"
    );
    assert_eq!(
        materialized.meta.total_matches, counted.meta.total_matches,
        "materialized find must agree with count-only find on the same query"
    );
    assert_eq!(
        materialized.matches.len(),
        counted.meta.total_matches as usize,
        "the materialized match list must have one entry per counted match"
    );
    assert!(
        materialized
            .matches
            .iter()
            .all(|found| found.data.role == "textfield"),
        "every materialized match must be the counted role, not a different one that happened to tie the total"
    );
}
