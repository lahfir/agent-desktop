use super::*;
use crate::types::AdPoint;

#[test]
fn test_mouse_button_mapping() {
    assert!(matches!(
        mouse_button_from_c(AdMouseButton::Left),
        CoreMouseButton::Left
    ));
    assert!(matches!(
        mouse_button_from_c(AdMouseButton::Right),
        CoreMouseButton::Right
    ));
    assert!(matches!(
        mouse_button_from_c(AdMouseButton::Middle),
        CoreMouseButton::Middle
    ));
}

#[test]
fn valid_discriminants_convert_to_typed_enums() {
    let ev = AdMouseEvent {
        kind: AdMouseEventKind::Click as i32,
        point: AdPoint { x: 10.0, y: 20.0 },
        button: AdMouseButton::Left as i32,
        click_count: 2,
    };
    assert!(matches!(
        AdMouseButton::from_c(ev.button),
        Some(AdMouseButton::Left)
    ));
    assert!(matches!(
        AdMouseEventKind::from_c(ev.kind),
        Some(AdMouseEventKind::Click)
    ));
}

#[test]
fn invalid_discriminants_reject_without_ub() {
    let ev = AdMouseEvent {
        kind: 999,
        point: AdPoint { x: 0.0, y: 0.0 },
        button: -5,
        click_count: 0,
    };
    assert!(AdMouseButton::from_c(ev.button).is_none());
    assert!(AdMouseEventKind::from_c(ev.kind).is_none());
}

/// F10 regression: `ad_mouse_event` hardcoded `modifiers: Vec::new()` with no
/// way for an FFI caller to supply a chord at all. These cover the additive
/// `modifiers_from_c` parser and `build_mouse_event`'s threading of its
/// output, which `ad_mouse_event_with_modifiers` relies on.
#[test]
fn modifiers_from_c_empty_when_count_zero() {
    let result = unsafe { modifiers_from_c(std::ptr::null(), 0) }.unwrap();
    assert!(result.is_empty());
}

#[test]
fn modifiers_from_c_maps_all_four_discriminants_in_order() {
    let raw: [i32; 4] = [
        AdModifier::Cmd as i32,
        AdModifier::Ctrl as i32,
        AdModifier::Alt as i32,
        AdModifier::Shift as i32,
    ];
    let result = unsafe { modifiers_from_c(raw.as_ptr(), 4) }.unwrap();
    assert_eq!(
        result,
        vec![
            CoreModifier::Cmd,
            CoreModifier::Ctrl,
            CoreModifier::Alt,
            CoreModifier::Shift,
        ]
    );
}

#[test]
fn modifiers_from_c_rejects_null_pointer_with_positive_count() {
    let result = unsafe { modifiers_from_c(std::ptr::null(), 2) };
    assert!(result.is_err());
}

#[test]
fn modifiers_from_c_rejects_count_exceeding_cap() {
    let raw: [i32; 5] = [0, 1, 2, 3, 0];
    let result = unsafe { modifiers_from_c(raw.as_ptr(), 5) };
    assert!(result.is_err());
}

#[test]
fn modifiers_from_c_rejects_invalid_discriminant() {
    let raw: [i32; 1] = [999];
    let result = unsafe { modifiers_from_c(raw.as_ptr(), 1) };
    assert!(result.is_err());
}

/// The core regression check: `build_mouse_event` must carry whatever
/// modifiers it was given through unchanged, not silently drop them the way
/// this function's predecessor did.
#[test]
fn build_mouse_event_carries_requested_modifiers_through_unchanged() {
    let ev = AdMouseEvent {
        kind: AdMouseEventKind::Click as i32,
        point: AdPoint { x: 1.0, y: 2.0 },
        button: AdMouseButton::Left as i32,
        click_count: 1,
    };
    let core_event = build_mouse_event(&ev, vec![CoreModifier::Cmd, CoreModifier::Shift]).unwrap();
    assert_eq!(
        core_event.modifiers,
        vec![CoreModifier::Cmd, CoreModifier::Shift]
    );
}

#[test]
fn build_mouse_event_rejects_invalid_button_discriminant() {
    let ev = AdMouseEvent {
        kind: AdMouseEventKind::Click as i32,
        point: AdPoint { x: 1.0, y: 2.0 },
        button: -5,
        click_count: 1,
    };
    assert!(build_mouse_event(&ev, Vec::new()).is_err());
}
