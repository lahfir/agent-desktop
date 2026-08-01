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

/// The bans on instantiating a pattern or keying on a display string, over
/// the files that would be tempted to break them.
///
/// Pattern-derived state is read as ordinary batched properties. The
/// crate-idiomatic route is `get_pattern`, a COM `QueryInterface` per node per
/// pattern - roughly 1,100 extra cross-process calls on a 220-node tree
/// against a walk tuned to batch - and `UICacheRequest::add_pattern` is the
/// same cost moved into the request. Neither exists here, and this is what
/// keeps it that way.
///
/// `LocalizedControlType` is banned as a map key separately: Microsoft
/// documents it as an OS-locale-dependent or provider-chosen display string,
/// so a role keyed on it breaks on a non-English Windows.
#[test]
fn the_vocabulary_instantiates_no_pattern_and_keys_on_no_display_string() {
    let sources = [
        include_str!("roles.rs"),
        include_str!("actions.rs"),
        include_str!("states.rs"),
        include_str!("property_ids.rs"),
        include_str!("properties.rs"),
        include_str!("cache.rs"),
        include_str!("element_properties.rs"),
    ];
    let banned = [
        concat!("get_", "pattern"),
        concat!("add_", "pattern"),
        concat!("Localized", "ControlType"),
    ];
    for source in sources {
        for line in source.lines() {
            let is_prose =
                line.trim_start().starts_with("///") || line.trim_start().starts_with("//!");
            for call in banned {
                assert!(
                    is_prose || !line.contains(call),
                    "the vocabulary must never reach for {call}: {line}"
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

    /// The seams now carry a vocabulary, and this is the assertion that says
    /// so. It asserted the opposite before the vocabulary wiring landed -
    /// `role == "unknown"` and an empty action list - so leaving it green
    /// would have meant the wiring never landed.
    ///
    /// It asserts on kind, never on count or shape: which controls a provider
    /// exposes is an `app/provider` fact, so the claim is that *some* node
    /// resolved a real role and *some* node advertises an affordance, not how
    /// many of either.
    #[test]
    fn the_live_walk_emits_real_vocabulary_for_the_nodes_it_reads() {
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

        let mut nodes = Vec::new();
        collect(&projected, &mut nodes);

        assert!(
            nodes.iter().any(|node| node.role != "unknown"),
            "every node resolved to the unknown role, so the role seam is still empty"
        );
        assert!(
            nodes
                .iter()
                .any(|node| !node.presentation.available_actions.is_empty()),
            "no node advertised an affordance, so the action seam is still empty"
        );
        assert!(
            nodes
                .iter()
                .any(|node| !node.presentation.states.is_empty()),
            "no node carried a state token, so the states plumbing is still empty"
        );
        assert!(
            nodes.iter().any(|node| node.role == "button"),
            "the fixture creates a BUTTON control, whose class is known independently of the map"
        );
    }

    /// The trap the affordance-gated action rule exists to prevent, asserted
    /// against a real provider rather than a fake:
    /// `IsLegacyIAccessiblePatternAvailable` is true on
    /// every element here, so an adapter that treated availability as an
    /// affordance would advertise an action on every node.
    #[test]
    fn a_live_walk_does_not_advertise_an_affordance_on_every_node() {
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

        let mut nodes = Vec::new();
        collect(&projected, &mut nodes);

        assert!(
            nodes
                .iter()
                .any(|node| node.presentation.available_actions.is_empty()),
            "every node advertised an affordance, which is the 141-of-141 defect"
        );
    }

    /// The three-run stability clause of the Verification Contract's "Live
    /// vocabulary" row: a live fixture walk's role, action list and state
    /// tokens must reproduce across three consecutive walks of the same
    /// provider, not merely appear once.
    ///
    /// Only self-consistency is asserted - run two and three reproduce run
    /// one, node for node - never a specific role, count, or token, which
    /// would pin an `app/provider` fact this repo's tests must not carry.
    #[test]
    fn the_live_walk_reproduces_the_same_vocabulary_across_three_runs() {
        crate::tree::fixture::ensure_test_apartment();
        let fixture = HostedFixture::spawn().expect("the fixture host starts");
        let root =
            root_from_hwnd(fixture.handle(), deadline()).expect("the fixture window resolves");
        let window = window();

        let mut runs = Vec::new();
        for _ in 0..3 {
            let outcome = walk_uia_subtree(&root, &ObservationRoot::Window(&window), budget(50))
                .expect("the walk assembles an observation");
            let projected = outcome
                .tree
                .into_accessibility_tree()
                .expect("core accepts a complete observation");

            let mut nodes = Vec::new();
            collect(&projected, &mut nodes);
            runs.push(
                nodes
                    .iter()
                    .map(|node| {
                        (
                            node.role.clone(),
                            node.presentation.available_actions.clone(),
                            node.presentation.states.clone(),
                        )
                    })
                    .collect::<Vec<_>>(),
            );
        }

        assert_eq!(
            runs[0], runs[1],
            "the second run's vocabulary diverged from the first"
        );
        assert_eq!(
            runs[0], runs[2],
            "the third run's vocabulary diverged from the first"
        );
    }

    fn collect<'a>(
        node: &'a agent_desktop_core::AccessibilityNode,
        into: &mut Vec<&'a agent_desktop_core::AccessibilityNode>,
    ) {
        into.push(node);
        for child in &node.children {
            collect(child, into);
        }
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
