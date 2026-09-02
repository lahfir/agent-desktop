use super::{OverlayMonitor, monitor_for_point, resting_point, to_physical};
use agent_desktop_core::{Point, Rect};

fn monitor(x: f64, y: f64, width: f64, height: f64, scale: f64, primary: bool) -> OverlayMonitor {
    OverlayMonitor {
        bounds: Rect {
            x,
            y,
            width,
            height,
        },
        work_area: Rect {
            x,
            y,
            width,
            height: height - 40.0,
        },
        scale,
        is_primary: primary,
    }
}

/// The arrangement A29-6 records as unpresentable on this rig: two monitors,
/// one of them scaled. Covered here precisely because it cannot be observed.
fn scaled_pair() -> Vec<OverlayMonitor> {
    vec![
        monitor(0.0, 0.0, 1920.0, 1080.0, 1.0, true),
        monitor(1920.0, 0.0, 1280.0, 800.0, 1.5, false),
    ]
}

#[test]
fn a_point_inside_the_second_monitor_selects_it_rather_than_the_primary() {
    let monitors = scaled_pair();

    let selected = monitor_for_point(
        &monitors,
        &Point {
            x: 2400.0,
            y: 400.0,
        },
    )
    .expect("a point on a monitor selects it");

    assert_eq!(selected, &monitors[1]);
}

#[test]
fn a_point_inside_the_primary_selects_the_primary() {
    let monitors = scaled_pair();

    assert_eq!(
        monitor_for_point(&monitors, &Point { x: 100.0, y: 100.0 }).expect("selects"),
        &monitors[0]
    );
}

/// A monitor left of or above the primary has negative coordinates, so a
/// mapping that assumed a zero origin would put the cursor on the wrong
/// screen entirely.
#[test]
fn a_monitor_at_a_negative_origin_is_selected_and_mapped_correctly() {
    let monitors = vec![
        monitor(0.0, 0.0, 1920.0, 1080.0, 1.0, true),
        monitor(-1280.0, -200.0, 1280.0, 800.0, 2.0, false),
    ];

    let selected = monitor_for_point(&monitors, &Point { x: -600.0, y: 0.0 }).expect("selects");
    assert_eq!(selected, &monitors[1]);

    let physical = to_physical(selected, &Point { x: -600.0, y: 0.0 });
    assert_eq!(physical.x, -1280.0 + (-600.0 - -1280.0) * 2.0);
    assert_eq!(physical.y, -200.0 + (0.0 - -200.0) * 2.0);
}

/// A point between two non-adjacent monitors belongs to neither. Answering
/// deterministically matters more than which one wins: an enumeration-order
/// answer would move with the desktop.
#[test]
fn a_point_in_the_gap_selects_the_nearest_monitor_deterministically() {
    let monitors = vec![
        monitor(0.0, 0.0, 800.0, 600.0, 1.0, true),
        monitor(2000.0, 0.0, 800.0, 600.0, 1.0, false),
    ];
    let gap = Point { x: 900.0, y: 300.0 };

    let first = monitor_for_point(&monitors, &gap).expect("selects");
    let reversed: Vec<_> = monitors.iter().rev().cloned().collect();
    let second = monitor_for_point(&reversed, &gap).expect("selects");

    assert_eq!(first, &monitors[0], "the nearer monitor wins");
    assert_eq!(
        second, &monitors[0],
        "and wins regardless of enumeration order"
    );
}

#[test]
fn a_scaled_monitor_maps_a_logical_point_to_its_physical_pixel() {
    let monitors = scaled_pair();
    let scaled = &monitors[1];
    let logical = Point {
        x: 2000.0,
        y: 100.0,
    };

    let physical = to_physical(scaled, &logical);
    assert_eq!(physical.x, 1920.0 + 80.0 * 1.5);
    assert_eq!(physical.y, 0.0 + 100.0 * 1.5);

    let unscaled = to_physical(&monitors[0], &logical);
    assert_eq!(
        unscaled, logical,
        "an unscaled monitor maps a point to itself, so the scaling above belongs to the \
         monitor rather than to the function"
    );
}

#[test]
fn an_unscaled_monitor_maps_a_point_to_itself() {
    let monitors = scaled_pair();

    let mapped = to_physical(&monitors[0], &Point { x: 640.0, y: 480.0 });

    assert_eq!(mapped, Point { x: 640.0, y: 480.0 });
}

#[test]
fn the_resting_point_is_the_primary_work_areas_midpoint() {
    let monitors = scaled_pair();

    assert_eq!(
        resting_point(&monitors).expect("a resting point"),
        Point { x: 960.0, y: 520.0 }
    );
}

/// An empty list is a stated failure rather than a silent primary, because
/// "no monitors" is a fact a caller has to be able to act on.
#[test]
fn an_empty_monitor_list_selects_nothing_rather_than_guessing() {
    assert!(monitor_for_point(&[], &Point { x: 0.0, y: 0.0 }).is_none());
    assert!(resting_point(&[]).is_none());
}
