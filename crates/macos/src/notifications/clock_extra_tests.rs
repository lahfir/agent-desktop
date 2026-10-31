use super::{CLOCK_IDENTIFIER, MAX_SEARCH_DEPTH, click_point, find_by_identifier};
use agent_desktop_core::{AdapterError, ErrorCode, Rect};

/// A menu bar shaped like macOS 27: node 0 is the extras bar, nodes 1-3 are
/// hosting views, and each hosting view holds one status item.
fn menu_bar_identifier(node: &u32) -> Option<String> {
    match node {
        11 => Some("com.apple.menuextra.wifi".into()),
        12 => Some("com.apple.menuextra.controlcenter".into()),
        13 => Some(CLOCK_IDENTIFIER.into()),
        _ => None,
    }
}

fn menu_bar_children(node: &u32) -> Vec<u32> {
    match node {
        0 => vec![1, 2, 3],
        1..=3 => vec![node + 10],
        _ => Vec::new(),
    }
}

#[test]
fn clock_inside_a_hosting_view_is_found_by_identifier() {
    let found = find_by_identifier(
        0_u32,
        CLOCK_IDENTIFIER,
        MAX_SEARCH_DEPTH,
        |node| Ok(menu_bar_identifier(node)),
        |node| Ok(menu_bar_children(node)),
    );

    assert_eq!(found.unwrap(), Some(13));
}

#[test]
fn a_localized_description_is_never_compared() {
    let found = find_by_identifier(
        0_u32,
        "Clock",
        MAX_SEARCH_DEPTH,
        |node| Ok(menu_bar_identifier(node)),
        |node| Ok(menu_bar_children(node)),
    );

    assert_eq!(found.unwrap(), None);
}

#[test]
fn search_stops_at_the_depth_limit() {
    let mut deepest_read = 0;
    let found = find_by_identifier(
        0_u32,
        CLOCK_IDENTIFIER,
        1,
        |node| {
            deepest_read = deepest_read.max(*node);
            Ok(menu_bar_identifier(node))
        },
        |node| Ok(menu_bar_children(node)),
    );

    assert_eq!(found.unwrap(), None);
    assert!(deepest_read < 10, "depth 2 must not be read");
}

#[test]
fn a_terminal_read_error_ends_the_search() {
    let found = find_by_identifier(
        0_u32,
        CLOCK_IDENTIFIER,
        MAX_SEARCH_DEPTH,
        |node| Ok(menu_bar_identifier(node)),
        |_| Err(AdapterError::permission_denied()),
    );

    assert_eq!(found.unwrap_err().code, ErrorCode::PermDenied);
}

#[test]
fn click_lands_on_the_clock_center() {
    let point = click_point(&Rect {
        x: 2440.0,
        y: 0.0,
        width: 90.0,
        height: 30.0,
    })
    .expect("a clock with area has a click point");

    assert_eq!(point.x, 2485.0);
    assert_eq!(point.y, 15.0);
}

#[test]
fn an_empty_rectangle_has_no_click_point() {
    for (width, height) in [(0.0, 30.0), (90.0, 0.0), (-1.0, 30.0)] {
        assert!(
            click_point(&Rect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            })
            .is_none()
        );
    }
}
