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
mod classification {
    use super::super::{classify_find, classify_find_all, gate_landmarks};
    use crate::system::hresult::{UIA_E_ELEMENTNOTAVAILABLE, UIA_E_TIMEOUT};
    use crate::tree::automation::{ERR_NOTFOUND, ERR_TIMEOUT};
    use agent_desktop_core::ErrorCode;
    use uiautomation::Error;

    fn hresult_error(code: i32) -> Error {
        Error::from(windows::core::HRESULT(code))
    }

    /// Exhaustion is what a real `find_first` on a region with no match
    /// reports, so the not-found answer must stay `None` - the same answer
    /// the old blanket mapping gave, minus the faults it also swallowed.
    #[test]
    fn a_genuinely_not_found_search_is_absence_not_a_fault() {
        let exhausted = Error::from(windows::core::Error::empty());
        assert!(
            classify_find(Err(exhausted), "probe")
                .expect("exhaustion is absence")
                .is_none()
        );
        assert!(
            classify_find(Err(Error::new(ERR_NOTFOUND, "test")), "probe")
                .expect("a raced-away subtree is absence")
                .is_none()
        );
        assert!(
            classify_find(Err(hresult_error(UIA_E_ELEMENTNOTAVAILABLE)), "probe")
                .expect("a vanished element is absence")
                .is_none()
        );
        assert!(
            classify_find_all(Err(Error::new(ERR_NOTFOUND, "test")), "probe")
                .expect("absence for the multi-match search too")
                .is_empty()
        );
    }

    /// The transient faults the old `Err(_) => Ok(None)` mapping swallowed:
    /// each must surface with its own code, never read as a confident
    /// "the center does not carry it".
    #[test]
    fn a_transient_search_failure_is_an_honest_error_not_a_confident_negative() {
        for error in [
            Error::new(ERR_TIMEOUT, "test"),
            hresult_error(UIA_E_TIMEOUT),
        ] {
            let surfaced = match classify_find(Err(error), "probe") {
                Err(surfaced) => surfaced,
                Ok(_) => panic!("a transient fault must surface, never read as absence"),
            };
            assert_eq!(surfaced.code, ErrorCode::Timeout);
        }
        let emptied = match classify_find_all(Err(Error::new(ERR_TIMEOUT, "test")), "probe") {
            Err(surfaced) => surfaced,
            Ok(_) => panic!("a transient fault must surface, never read as an empty match set"),
        };
        assert_eq!(emptied.code, ErrorCode::Timeout);
    }

    /// The landmark gate's distinction: absence (what a missing landmark
    /// reports) flows into the gate and is refused as an unrecognized tree -
    /// PLATFORM_NOT_SUPPORTED, never an empty listing - while a transient
    /// fault never reaches the gate at all because the search surfaces
    /// first. Reverting the search classification to blanket-absence fails
    /// `a_transient_search_failure_is_an_honest_error_not_a_confident_negative`.
    #[test]
    fn an_absent_landmark_reaches_the_gate_which_refuses_rather_than_lists_empty() {
        let absent = classify_find(Err(Error::new(ERR_NOTFOUND, "test")), "probe")
            .expect("a missing landmark is absence");
        assert!(absent.is_none());

        let shape = gate_landmarks(absent.is_some(), false)
            .expect_err("a center with no landmarks must be refused");
        assert_eq!(shape.code, ErrorCode::PlatformNotSupported);
    }
}

#[cfg(target_os = "windows")]
mod live {
    use agent_desktop_core::{Deadline, InteractionPolicy};

    use super::super::{find_by_id, read_entries};
    use crate::notifications::session::ActionCenterSession;
    use crate::notifications::toast_support;
    use crate::system::raise_oracle::{responded_since, witness_desktop};
    use crate::system::shell_surface_kinds::{EMPTY_CENTER_LANDMARKS, MAIN_LIST_VIEW};
    use crate::system::test_support::{SHELL_SURFACE_LOCK, or_skip_shell};
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
        let witness = witness_desktop();
        let Some(session) = or_skip_shell(
            "action center open for the reader count check",
            ActionCenterSession::open(headed(), deadline(15_000)),
            || responded_since(&witness),
        ) else {
            return;
        };
        let _staged = toast_support::StagedToast::stage();
        let Some(listed) = toast_support::wait_until_listed_held(session.hwnd(), deadline(30_000))
        else {
            eprintln!("skip toast staging: this desktop's toast staging produced no entry");
            return;
        };
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
        let witness = witness_desktop();
        let Some(session) = or_skip_shell(
            "action center open for the empty-center read",
            ActionCenterSession::open(headed(), deadline(15_000)),
            || responded_since(&witness),
        ) else {
            return;
        };
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

/// Uses the crate's own hosted fixture window - never the shell's Action
/// Center - so this case stays deterministic.
///
/// `name_of` and `control_type`'s own fix has no equivalent test here: a
/// property read through an element resolved before its owning process was
/// killed was measured, on this host, to answer `Ok` with a stale value
/// (`""` for the name, `Pane` for the control type) rather than fail, so
/// there is no in-repo, deterministic way to make that read fail the way
/// `find_by_id`'s searches fail reliably.
#[cfg(target_os = "windows")]
mod fixture_backed {
    use agent_desktop_core::{Deadline, ErrorCode};

    use super::super::{MAX_WALK_DEPTH, walk};
    use crate::tree::fixture::{HostedFixture, bootstrap};

    fn deadline(ms: u64) -> Deadline {
        Deadline::after(ms).expect("deadline")
    }

    /// `walk` checks its depth cap before it ever touches `element`'s
    /// children, so a real but otherwise ordinary fixture element proves the
    /// cap's own behavior without needing an eleven-level-deep tree.
    #[test]
    fn a_walk_past_the_depth_cap_surfaces_as_an_error_not_a_silent_stop() {
        bootstrap();
        let fixture = HostedFixture::spawn().expect("a fixture host starts");
        let root = crate::tree::automation::root_from_hwnd(fixture.handle(), deadline(10_000))
            .expect("the fixture window resolves");
        let mut entries = Vec::new();

        let error = walk(
            &root,
            None,
            MAX_WALK_DEPTH + 1,
            &mut entries,
            deadline(10_000),
        )
        .expect_err("a walk past the depth cap must surface as an error, not stop silently");

        assert_eq!(error.code, ErrorCode::AppUnresponsive);
        let details = error
            .details
            .clone()
            .expect("the depth truncation must be an honest, surfaced detail");
        assert_eq!(details["complete"], false);
        assert!(entries.is_empty());
    }
}
