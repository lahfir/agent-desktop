use super::*;
use crate::tree::walker_fake::{budget, window};
use agent_desktop_core::ObservationRoot;

/// The crate's own `get_children` retires end-of-siblings and a cross-process
/// fault through the same arm; `SetFocus` moves the desktop foreground (A3-4)
/// and is therefore not headless; ref allocation belongs to core alone.
#[test]
fn the_walk_issues_no_banned_call() {
    let sources = [
        include_str!("walker.rs"),
        include_str!("walker_enumerate.rs"),
        include_str!("walker_source.rs"),
        include_str!("walker_tests.rs"),
        include_str!("walker_fake.rs"),
        include_str!("walker_source_tests.rs"),
    ];
    let banned = [
        concat!("get_", "children"),
        concat!("Set", "Focus"),
        concat!("allocate_", "refs"),
    ];
    for source in sources {
        for line in source.lines() {
            let is_prose =
                line.trim_start().starts_with("///") || line.trim_start().starts_with("//!");
            for call in banned {
                assert!(
                    is_prose || !line.contains(call),
                    "the walk must never call {call}: {line}"
                );
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod windows_only {
    use super::*;
    use crate::tree::automation::root_from_hwnd;
    use crate::tree::fixture::HostedFixture;
    use crate::tree::walker_fake::deadline;

    /// The live half of the discriminator check. A walk whose classification is
    /// inverted reports this cross-process window incomplete, so an inverted
    /// discriminator fails here rather than passing silently.
    #[test]
    fn a_cross_process_fixture_walk_terminates_finds_controls_and_reports_complete() {
        crate::tree::fixture::ensure_test_apartment();
        let fixture = HostedFixture::spawn().expect("the fixture host starts");
        let root =
            root_from_hwnd(fixture.handle(), deadline()).expect("the fixture window resolves");
        let window = window();

        let outcome = walk_uia_subtree(&root, &ObservationRoot::Window(&window), budget(50))
            .expect("the walk assembles an observation");

        assert!(
            outcome.failures.is_empty(),
            "a live cross-process walk raised an enumeration failure"
        );
        assert!(
            outcome.tree.is_complete(),
            "a live cross-process walk reported an incomplete tree"
        );
        let projected = outcome
            .tree
            .into_accessibility_tree()
            .expect("core accepts a complete observation");
        assert!(
            !projected.children.is_empty(),
            "the walk found none of the fixture's created controls"
        );
    }

    /// Every node the live walk emits carries the seams 2.3 fills, so 2.3 can
    /// fill them without reshaping the traversal.
    #[test]
    fn the_live_walk_calls_the_vocabulary_seams_for_every_node() {
        crate::tree::fixture::ensure_test_apartment();
        let fixture = HostedFixture::spawn().expect("the fixture host starts");
        let root =
            root_from_hwnd(fixture.handle(), deadline()).expect("the fixture window resolves");
        let window = window();

        let outcome = walk_uia_subtree(&root, &ObservationRoot::Window(&window), budget(50))
            .expect("the walk assembles an observation");
        let projected = outcome
            .tree
            .into_accessibility_tree()
            .expect("core accepts a complete observation");

        assert_eq!(projected.role, "unknown");
        assert!(projected.presentation.available_actions.is_empty());
    }
}

#[cfg(not(target_os = "windows"))]
mod non_windows_only {
    use super::*;
    use crate::tree::element::{CannedElement, UIAElement};

    /// The canned arm exists so every module that calls the walk compiles and
    /// runs off Windows. It enumerates nothing, which is the exhaustion answer,
    /// so its walk is a single complete node.
    #[test]
    fn the_canned_source_walks_a_single_complete_node() {
        let window = window();
        let root = UIAElement::from(CannedElement);

        let outcome = walk_uia_subtree(&root, &ObservationRoot::Window(&window), budget(10))
            .expect("the canned walk assembles an observation");

        assert!(outcome.tree.is_complete());
        assert!(outcome.failures.is_empty());
        assert!(outcome.tree.into_accessibility_tree().is_ok());
    }
}
