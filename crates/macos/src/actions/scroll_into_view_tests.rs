use agent_desktop_core::{Direction, Rect};

use super::{direction_for_visibility, intersects, rect_has_area, scroll_effect_observed};

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn area_requires_finite_positive_dimensions_and_coordinates() {
    assert!(rect_has_area(rect(0.0, 0.0, 10.0, 10.0)));
    assert!(!rect_has_area(rect(0.0, 0.0, 0.0, 10.0)));
    assert!(!rect_has_area(rect(0.0, 0.0, -1.0, 10.0)));
    assert!(!rect_has_area(rect(f64::NAN, 0.0, 10.0, 10.0)));
    assert!(!rect_has_area(rect(0.0, 0.0, f64::INFINITY, 10.0)));
}

#[test]
fn intersection_requires_positive_overlapping_area() {
    let window = rect(0.0, 0.0, 100.0, 100.0);

    assert!(intersects(rect(10.0, 10.0, 20.0, 20.0), window));
    assert!(intersects(rect(90.0, 90.0, 20.0, 20.0), window));
    assert!(intersects(rect(80.0, 10.0, 20.0, 20.0), window));
    assert!(!intersects(rect(101.0, 10.0, 20.0, 20.0), window));
    assert!(!intersects(rect(100.0, 10.0, 20.0, 20.0), window));
}

#[test]
fn offscreen_direction_uses_global_viewport_edges() {
    let viewport = rect(1496.0, 87.0, 1496.0, 937.0);

    assert!(matches!(
        direction_for_visibility(rect(2030.0, 1026.0, 73.0, 24.0), viewport),
        Some(Direction::Down)
    ));
    assert!(matches!(
        direction_for_visibility(rect(2030.0, 80.0, 73.0, 24.0), viewport),
        Some(Direction::Up)
    ));
    assert!(direction_for_visibility(rect(1500.0, 100.0, 73.0, 24.0), viewport).is_none());
}

#[test]
fn acknowledged_scroll_without_geometry_change_is_not_delivery() {
    let before = rect(2622.0, 1063.0, 177.0, 24.0);

    assert!(!scroll_effect_observed(Some(before), Some(before)));
    assert!(scroll_effect_observed(
        Some(before),
        Some(rect(2622.0, 900.0, 177.0, 24.0))
    ));
}
