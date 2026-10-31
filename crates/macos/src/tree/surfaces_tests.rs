use super::*;

#[test]
fn surface_node_limit_is_explicitly_incomplete() {
    let error = surface_limit_error();

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(error.details.expect("limit details")["complete"], false);
}

#[test]
fn menus_beyond_eight_wrappers_remain_discoverable() {
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    for target_depth in [9, 11, 30] {
        let found = find_menu_descendant_with(0, deadline, |depth| {
            Ok((*depth == target_depth, vec![depth + 1]))
        })
        .unwrap();
        assert_eq!(found, Some(target_depth));
    }
}

#[test]
fn cyclic_menu_search_remains_bounded_and_explicitly_incomplete() {
    let mut reads = 0;
    let error = find_menu_descendant_with(
        0,
        Instant::now() + std::time::Duration::from_secs(5),
        |node| {
            reads += 1;
            Ok((false, vec![*node]))
        },
    )
    .unwrap_err();
    assert_eq!(reads, MAX_SURFACE_NODES);
    assert_eq!(error.details.unwrap()["complete"], false);
}

#[test]
fn expired_menu_search_never_reads_a_node() {
    let result = find_menu_descendant_with::<()>((), Instant::now(), |_| {
        panic!("expired search must not call AX")
    });
    assert_eq!(result.unwrap_err().code, ErrorCode::Timeout);
}

#[test]
fn incomplete_content_branch_does_not_hide_a_menu_in_a_sibling() {
    let result = find_menu_descendant_with(
        0,
        Instant::now() + std::time::Duration::from_secs(5),
        |node| match node {
            0 => Ok((false, vec![1, 2])),
            1 => Err(surface_limit_error()),
            2 => Ok((true, vec![])),
            _ => unreachable!(),
        },
    );
    assert_eq!(result.unwrap(), Some(2));
}

#[test]
fn an_incomplete_branch_cannot_prove_menu_absence() {
    let result = find_menu_descendant_with(
        0,
        Instant::now() + std::time::Duration::from_secs(5),
        |node| match node {
            0 => Ok((false, vec![1, 2])),
            1 => Err(surface_limit_error()),
            _ => Ok((false, vec![])),
        },
    );
    assert_eq!(result.unwrap_err().details.unwrap()["complete"], false);
}

#[test]
fn shallow_menu_precedes_unbounded_content_and_permission_errors_stay_terminal() {
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    let found = find_menu_descendant_with(0, deadline, |node| match node {
        0 => Ok((false, vec![1, 2])),
        1 => Ok((false, vec![1])),
        _ => Ok((true, vec![])),
    });
    assert_eq!(found.unwrap(), Some(2));
    let denied = find_menu_descendant_with(0, deadline, |node| match node {
        0 => Ok((false, vec![1, 2])),
        1 => Err(AdapterError::new(ErrorCode::PermDenied, "denied")),
        _ => panic!("permission failure must stop all further AX reads"),
    });
    assert_eq!(denied.unwrap_err().code, ErrorCode::PermDenied);
}

#[test]
fn an_element_that_vanished_mid_walk_is_skipped_not_fatal() {
    let result = find_menu_descendant_with(
        0,
        Instant::now() + std::time::Duration::from_secs(5),
        |node| match node {
            0 => Ok((false, vec![1, 2])),
            1 => Err(AdapterError::new(
                ErrorCode::ElementNotFound,
                "row recycled",
            )),
            2 => Ok((false, vec![])),
            _ => unreachable!(),
        },
    );
    assert_eq!(result.unwrap(), None);
}

#[test]
fn a_vanished_sibling_does_not_hide_an_open_menu() {
    let result = find_menu_descendant_with(
        0,
        Instant::now() + std::time::Duration::from_secs(5),
        |node| match node {
            0 => Ok((false, vec![1, 2])),
            1 => Err(AdapterError::new(
                ErrorCode::ElementNotFound,
                "row recycled",
            )),
            2 => Ok((true, vec![])),
            _ => unreachable!(),
        },
    );
    assert_eq!(result.unwrap(), Some(2));
}
