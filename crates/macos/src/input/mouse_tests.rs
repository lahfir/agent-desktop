use super::{Modifier, create_event, event_flags};
use core_graphics::event::{CGEventFlags, CGEventType, CGMouseButton};
use core_graphics::geometry::CGPoint;

/// F10 regression: `synthesize_mouse` previously never read `event.modifiers`
/// at all, so a chorded `mouse-click --modifiers cmd` was a silent no-op —
/// the receiving app saw a plain click. These pin the modifier -> CGEventFlags
/// bit mapping that `synthesize_mouse` and `synthesize_scroll_at` both rely on.
#[test]
fn event_flags_maps_cmd_to_command_bit() {
    assert_eq!(
        event_flags(&[Modifier::Cmd]),
        CGEventFlags::CGEventFlagCommand
    );
}

#[test]
fn event_flags_maps_shift_to_shift_bit() {
    assert_eq!(
        event_flags(&[Modifier::Shift]),
        CGEventFlags::CGEventFlagShift
    );
}

#[test]
fn event_flags_maps_alt_to_alternate_bit() {
    assert_eq!(
        event_flags(&[Modifier::Alt]),
        CGEventFlags::CGEventFlagAlternate
    );
}

#[test]
fn event_flags_maps_ctrl_to_control_bit() {
    assert_eq!(
        event_flags(&[Modifier::Ctrl]),
        CGEventFlags::CGEventFlagControl
    );
}

#[test]
fn event_flags_combines_multiple_modifiers_via_bitwise_or() {
    let combined = event_flags(&[Modifier::Cmd, Modifier::Shift]);
    assert_eq!(
        combined,
        CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagShift
    );
    assert!(combined.contains(CGEventFlags::CGEventFlagCommand));
    assert!(combined.contains(CGEventFlags::CGEventFlagShift));
    assert!(!combined.contains(CGEventFlags::CGEventFlagAlternate));
}

#[test]
fn event_flags_empty_slice_yields_no_flags() {
    assert_eq!(event_flags(&[]), CGEventFlags::empty());
}

/// Exercises the real `create_event` path `synthesize_mouse`/`synthesize_click`
/// dispatch through, not a standalone copy of the mapping. If the
/// `event.set_flags(flags)` call were dropped from `create_event_with_source`,
/// the returned event would carry whatever the ambient/default flags are
/// instead of the requested chord, and this assertion would fail.
#[test]
fn create_event_carries_requested_modifier_flags() {
    let flags = event_flags(&[Modifier::Cmd, Modifier::Ctrl]);
    let event = create_event(
        CGEventType::LeftMouseDown,
        CGPoint::new(0.0, 0.0),
        CGMouseButton::Left,
        flags,
    )
    .expect("CGEvent construction must not require Accessibility permission");

    assert_eq!(event.get_flags(), flags);
}

/// The "restore after" guarantee: a chorded event followed by a plain one
/// must not leak the chord onto the later event. Because every call computes
/// its flags fresh from its own `modifiers` slice and applies them
/// unconditionally (rather than only when non-empty), there is no shared
/// state through which a prior call's flags could survive into the next.
#[test]
fn create_event_after_chorded_call_carries_no_stale_flags_for_unmodified_click() {
    let chorded_flags = event_flags(&[Modifier::Cmd, Modifier::Shift, Modifier::Alt]);
    let _chorded = create_event(
        CGEventType::LeftMouseDown,
        CGPoint::new(0.0, 0.0),
        CGMouseButton::Left,
        chorded_flags,
    )
    .unwrap();

    let plain_flags = event_flags(&[]);
    let plain = create_event(
        CGEventType::LeftMouseUp,
        CGPoint::new(0.0, 0.0),
        CGMouseButton::Left,
        plain_flags,
    )
    .unwrap();

    assert_eq!(plain.get_flags(), CGEventFlags::empty());
}
