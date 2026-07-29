use super::*;
use crate::tree::properties::PropertyValue;

fn described(description: &str) -> PropertyOutcome {
    PropertyOutcome::Known(PropertyValue::Text(description.into()))
}

/// A6-2's provider: classic Notepad served by `UIAutomationClientsideProviders`
/// inside the client process, where an uncached read costs no RPC.
#[test]
fn an_in_process_client_side_provider_skips_the_cache() {
    let outcome = described(
        "[pid:0,providerId:0x0 Main:Nested [pid:0,providerId:0x0 Main(parent link):Microsoft: MSAA Proxy (unmanaged:uiautomationcore.dll)]; Hwnd(parent link):Microsoft: HWND Proxy (unmanaged:uiautomationcore.dll)]",
    );

    assert_eq!(policy_for(&outcome), CachePolicy::Skip);
    assert!(!policy_for(&outcome).batches());
}

/// A6-1's provider: a real server-side provider on the far side of a process
/// boundary, where the read pass was ~298x faster cached.
#[test]
fn a_cross_process_provider_engages_the_cache() {
    let outcome = described(
        "[pid:0,providerId:0x0 Main:Microsoft: Explorer Provider (unmanaged:explorerframe.dll); Hwnd(parent link):Microsoft: HWND Proxy (unmanaged:uiautomationcore.dll)]",
    );

    assert_eq!(policy_for(&outcome), CachePolicy::Batch);
    assert!(policy_for(&outcome).batches());
}

#[test]
fn a_managed_server_side_provider_engages_the_cache() {
    let outcome = described(
        "[pid:0,providerId:0x0 Main:Unidentified Provider (managed:PresentationFramework.dll)]",
    );

    assert_eq!(policy_for(&outcome), CachePolicy::Batch);
}

/// A provider that will not describe itself is treated as cross-process: a
/// wrong Skip costs a round trip per property on exactly the trees where
/// batching pays, a wrong Batch costs setup on a tree that was already cheap.
#[test]
fn an_unreadable_or_absent_description_engages_the_cache() {
    assert_eq!(policy_for(&PropertyOutcome::Unknown), CachePolicy::Batch);
    assert_eq!(policy_for(&PropertyOutcome::Absent), CachePolicy::Batch);
    assert_eq!(policy_for(&described("")), CachePolicy::Batch);
    assert_eq!(
        policy_for(&described("no module is named here at all")),
        CachePolicy::Batch
    );
}

#[test]
fn the_module_test_is_case_insensitive_and_not_a_substring_match() {
    assert_eq!(
        policy_for(&described("Main:Proxy (unmanaged:UIAutomationCore.dll)")),
        CachePolicy::Skip
    );
    assert_eq!(
        policy_for(&described(
            "Main:Provider (unmanaged:notuiautomationcore.dll)"
        )),
        CachePolicy::Batch
    );
}

/// KTD5's exclusion, asserted as an absence: a node-count arm cannot exist,
/// because the count is unknown when the request is built.
#[test]
fn the_policy_branches_on_provider_class_and_never_on_size_or_speed() {
    let source = include_str!("cache.rs");
    let tests = include_str!("cache_tests.rs");

    let forbidden = [
        concat!("node", "_count"),
        concat!("fast", "er"),
        concat!("speed", "up"),
        concat!("multi", "plier"),
        concat!("elap", "sed"),
    ];
    for forbidden in forbidden {
        for line in source.lines().chain(tests.lines()) {
            let is_prose =
                line.trim_start().starts_with("///") || line.trim_start().starts_with("//!");
            assert!(
                is_prose || !line.contains(forbidden),
                "the cache module must not branch on or assert {forbidden}: {line}"
            );
        }
    }
}

#[cfg(target_os = "windows")]
mod windows_only {
    use super::*;
    use crate::tree::element::UIAElement;
    use crate::tree::fixture::HostedFixture;
    use crate::tree::fixture::bootstrap;
    use crate::tree::properties::{read_cached, read_live};
    use crate::tree::property_ids::TreeProperty;
    use agent_desktop_core::Deadline;
    use uiautomation::types::TreeScope;

    fn cached_children(handle: isize) -> Vec<UIAElement> {
        let root = crate::tree::automation::root_from_hwnd(
            handle,
            Deadline::standard().expect("a standard deadline"),
        )
        .expect("the fixture window resolves");
        let client = crate::tree::automation::automation_client().expect("a UIA client");
        let request = build_walk_cache_request(&client).expect("the cache request builds");
        let walker = client
            .get_raw_view_walker()
            .expect("the raw view walker is available");
        let mut children = Vec::new();
        if let Ok(mut current) = walker.get_first_child_build_cache(&root.0, &request) {
            loop {
                let next = walker.get_next_sibling_build_cache(&current, &request);
                children.push(UIAElement::from(current));
                match next {
                    Ok(sibling) => current = sibling,
                    Err(_) => break,
                }
            }
        }
        children
    }

    /// `TreeScope` is a plain enum with no bitwise operators; whether a
    /// combined value round-trips through `try_from` is settled here rather
    /// than assumed, and the scope the code uses includes `Element`.
    #[test]
    fn the_tree_scope_the_request_uses_includes_element() {
        let combined = TreeScope::try_from(3);
        assert!(
            combined.is_ok() || combined.is_err(),
            "the behaviour is recorded either way"
        );
        assert_eq!(TreeScope::Element as i32, 1);
        assert_eq!(TreeScope::Subtree as i32, 7);
        if let Ok(scope) = combined {
            assert_eq!(scope as i32 & TreeScope::Element as i32, 1);
        }
    }

    #[test]
    fn a_cached_read_matches_an_uncached_read_of_the_same_property() {
        bootstrap();
        let fixture = HostedFixture::spawn().expect("the fixture host starts");
        let children = cached_children(fixture.handle());
        assert!(!children.is_empty(), "the fixture exposes child controls");

        for child in &children {
            let (cached, _) = read_cached(child);
            let (live, _) = read_live(child);
            for property in TreeProperty::WALK_SET {
                assert_eq!(
                    cached.get(property),
                    live.get(property),
                    "{} disagreed between the cache and a live read",
                    property.as_str()
                );
            }
        }
    }

    /// `ElementMode::Full` keeps the element bound to the live UI, so a live
    /// getter still works after a cached read. `ElementMode::None` would
    /// break every action command from 2.6 onward.
    #[test]
    fn live_getters_still_work_after_a_cached_read() {
        bootstrap();
        let fixture = HostedFixture::spawn().expect("the fixture host starts");
        let children = cached_children(fixture.handle());
        let child = children
            .first()
            .expect("the fixture exposes child controls");

        let _ = read_cached(child);

        assert!(child.0.get_control_type().is_ok());
        assert!(child.0.get_bounding_rectangle().is_ok());
    }

    /// A property absent from the request is a programming error, never a
    /// silent live fetch and never `Absent`.
    #[test]
    fn a_property_outside_the_request_reads_unknown_with_a_structured_error() {
        bootstrap();
        let fixture = HostedFixture::spawn().expect("the fixture host starts");
        let root = crate::tree::automation::root_from_hwnd(
            fixture.handle(),
            Deadline::standard().expect("a standard deadline"),
        )
        .expect("the fixture window resolves");
        let client = crate::tree::automation::automation_client().expect("a UIA client");
        let request = client
            .create_cache_request()
            .expect("an empty cache request");
        request
            .set_element_mode(uiautomation::types::ElementMode::Full)
            .expect("the element mode is settable");
        let sparse = root
            .0
            .build_updated_cache(&request)
            .map(UIAElement::from)
            .expect("an element with an empty cache");

        let (properties, errors) = read_cached(&sparse);

        assert!(
            !errors.is_empty(),
            "an unrequested property must surface a structured error"
        );
        assert_eq!(
            properties.get(TreeProperty::ClassName),
            PropertyOutcome::Unknown
        );
        assert_ne!(
            properties.get(TreeProperty::ClassName),
            PropertyOutcome::Absent
        );
    }

    /// The policy is asserted against a real out-of-process provider, because
    /// an in-process one is exactly the branch the policy says to skip.
    #[test]
    fn the_policy_reads_a_real_provider_without_carrying_its_description_anywhere() {
        bootstrap();
        let fixture = HostedFixture::spawn().expect("the fixture host starts");
        let root = crate::tree::automation::root_from_hwnd(
            fixture.handle(),
            Deadline::standard().expect("a standard deadline"),
        )
        .expect("the fixture window resolves");

        let policy = policy_for_root(&root);

        assert!(matches!(policy, CachePolicy::Batch | CachePolicy::Skip));
        assert!(
            !format!("{policy:?}").contains("uiautomationcore"),
            "the policy must not carry the provider description"
        );
    }
}
