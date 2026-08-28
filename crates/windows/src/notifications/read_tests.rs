use agent_desktop_core::ErrorCode;

use super::{CenterShape, gate_landmarks, unrecognized_center_error};

#[test]
fn a_center_carrying_the_notification_list_walks_it() {
    assert!(matches!(
        gate_landmarks(true, false),
        Ok(CenterShape::WalkList)
    ));
    assert!(
        matches!(gate_landmarks(true, true), Ok(CenterShape::WalkList)),
        "the list wins whenever it is present, whatever the empty-state landmarks do"
    );
}

#[test]
fn an_empty_center_is_a_legitimate_zero_entry_answer() {
    assert!(matches!(
        gate_landmarks(false, true),
        Ok(CenterShape::EmptyCenter)
    ));
}

#[test]
fn a_center_with_no_landmarks_is_refused_not_listed_as_empty() {
    let shape = gate_landmarks(false, false).expect_err("an unrecognized tree must be refused");

    assert_eq!(shape.code, ErrorCode::PlatformNotSupported);
    assert_eq!(
        unrecognized_center_error().code,
        ErrorCode::PlatformNotSupported,
        "the refusal is the landmark gate's own answer, not a silent empty listing"
    );
}

#[test]
fn the_refusal_names_the_build_and_the_missing_landmark() {
    let error = unrecognized_center_error();

    let detail = error
        .platform_detail
        .as_deref()
        .expect("the refusal must carry its reason");
    assert!(
        detail.contains("MainListView"),
        "the detail names the missing landmark's AutomationId: {detail}"
    );
    assert!(
        detail.contains("Windows build"),
        "the detail names the build the shape was measured against: {detail}"
    );
}

#[cfg(target_os = "windows")]
mod live {
    use agent_desktop_core::{Deadline, InteractionPolicy};

    use super::super::{find_by_id, read_entries};
    use crate::notifications::session::ActionCenterSession;
    use crate::notifications::toast_support;
    use crate::system::shell_surface_kinds::{EMPTY_CENTER_LANDMARKS, MAIN_LIST_VIEW};
    use crate::system::test_support::SHELL_SURFACE_LOCK;
    use crate::tree::element::UIAElement;

    fn deadline(ms: u64) -> Deadline {
        Deadline::after(ms).expect("deadline")
    }

    fn headed() -> InteractionPolicy {
        InteractionPolicy::headed()
    }

    /// Counts the list's items with an independent query - a control-type
    /// condition over the whole subtree - so the count the reader returns is
    /// asserted against the tree rather than against the reader's own walk.
    fn independent_list_item_count(root: &UIAElement) -> Option<usize> {
        use uiautomation::types::{ControlType, TreeScope, UIProperty};
        use uiautomation::variants::Variant;

        let client = crate::tree::automation::automation_client().ok()?;
        let list = match find_by_id(root, TreeScope::Descendants, MAIN_LIST_VIEW) {
            Ok(Some(list)) => list,
            Ok(None) => {
                let empty = EMPTY_CENTER_LANDMARKS.iter().any(|landmark| {
                    find_by_id(root, TreeScope::Descendants, landmark)
                        .ok()
                        .flatten()
                        .is_some()
                });
                return empty.then_some(0);
            }
            Err(_) => return None,
        };
        let condition = client
            .create_property_condition(
                UIProperty::ControlType,
                Variant::from(ControlType::ListItem as i32),
                None,
            )
            .ok()?;
        let items = list.0.find_all(TreeScope::Descendants, &condition).ok()?;
        Some(items.len())
    }

    #[test]
    fn the_reader_s_count_matches_the_tree_s_own_list_item_count() {
        crate::tree::fixture::bootstrap();
        let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        toast_support::clear_center(deadline(20_000));
        let session = ActionCenterSession::open(headed(), deadline(15_000))
            .expect("the center opens for a headed caller");
        let _staged = toast_support::StagedToast::stage();
        let listed = toast_support::wait_until_listed_held(session.hwnd(), deadline(30_000));
        assert_eq!(
            listed.len(),
            1,
            "the staged toast is the only entry after the reset"
        );

        let root = crate::tree::automation::root_from_hwnd(session.hwnd(), deadline(10_000))
            .expect("the open center roots");
        let tree_count = independent_list_item_count(&root).expect("the center's tree is readable");
        let read = read_entries(&root, deadline(20_000)).expect("the recognized tree reads");
        session.close().expect("the session restores the surface");

        assert_eq!(
            read.len(),
            tree_count,
            "the reader must see exactly the entries the tree carries - a walk bug that drops entries fails here"
        );
    }

    #[test]
    fn an_empty_center_is_a_zero_entry_answer_not_a_refusal() {
        crate::tree::fixture::bootstrap();
        let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        toast_support::clear_center(deadline(20_000));

        let session = ActionCenterSession::open(headed(), deadline(15_000))
            .expect("the center opens for a headed caller");
        let root = crate::tree::automation::root_from_hwnd(session.hwnd(), deadline(10_000))
            .expect("the open center roots");
        let entries = read_entries(&root, deadline(20_000)).expect(
            "an empty center is a legitimate zero-entry answer, never the landmark refusal",
        );
        session.close().expect("the session restores the surface");

        assert!(
            entries.is_empty(),
            "the center was reset before this test; any entry here is a staging leak"
        );
    }
}
