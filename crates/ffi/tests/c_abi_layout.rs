mod common;

use common::{AdAction, AdActionResult, AdActionStep, AdElementState, AdPoint, AdRect, AdRefEntry};
use std::mem::{MaybeUninit, align_of, offset_of, size_of};

#[test]
fn action_layout_is_guarded_for_c_consumers() {
    assert_eq!(agent_desktop_ffi::types::action::AD_ACTION_SIZE, 96);
    assert_eq!(
        unsafe { common::ad_action_size() },
        agent_desktop_ffi::types::action::AD_ACTION_SIZE
    );
    assert_eq!(size_of::<AdAction>(), 96);
    assert_eq!(align_of::<AdAction>(), align_of::<usize>());

    let offsets = [
        offset_of!(AdAction, kind),
        offset_of!(AdAction, text),
        offset_of!(AdAction, scroll),
        offset_of!(AdAction, key),
        offset_of!(AdAction, drag),
    ];
    assert_eq!(offsets[0], 0);
    assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]));

    let copied = unsafe {
        let action = MaybeUninit::<AdAction>::zeroed().assume_init();
        std::ptr::read(&action as *const AdAction)
    };
    assert_eq!(copied.kind, 0);
    assert_eq!(copied.drag.drop_delay_ms, 0);
}

#[test]
fn action_result_layout_is_guarded_for_c_consumers() {
    assert_eq!(
        agent_desktop_ffi::types::action_result::AD_ACTION_RESULT_SIZE,
        40
    );
    assert_eq!(
        unsafe { common::ad_action_result_size() },
        agent_desktop_ffi::types::action_result::AD_ACTION_RESULT_SIZE
    );
    assert_eq!(size_of::<AdActionStep>(), 16);
    assert_eq!(align_of::<AdActionStep>(), align_of::<usize>());
    assert_eq!(size_of::<AdActionResult>(), 40);
    assert_eq!(align_of::<AdActionResult>(), align_of::<usize>());
    assert_eq!(offset_of!(AdActionResult, action), 0);
    assert_eq!(offset_of!(AdActionResult, ref_id), 8);
    assert_eq!(offset_of!(AdActionResult, post_state), 16);
    assert_eq!(offset_of!(AdActionResult, steps), 24);
    assert_eq!(offset_of!(AdActionResult, step_count), 32);
    assert_eq!(offset_of!(AdActionStep, label), 0);
    assert_eq!(offset_of!(AdActionStep, outcome), 8);
}

#[test]
fn element_state_layout_is_guarded_for_c_consumers() {
    assert_eq!(
        agent_desktop_ffi::types::element_state::AD_ELEMENT_STATE_SIZE,
        32
    );
    assert_eq!(
        unsafe { common::ad_element_state_size() },
        agent_desktop_ffi::types::element_state::AD_ELEMENT_STATE_SIZE
    );
    assert_eq!(size_of::<AdElementState>(), 32);
    assert_eq!(align_of::<AdElementState>(), align_of::<usize>());
    assert_eq!(offset_of!(AdElementState, role), 0);
}

#[test]
fn rect_and_point_layouts_are_memcpyable() {
    let rect = AdRect {
        x: 1.25,
        y: -2.5,
        width: 640.0,
        height: 480.0,
    };
    let copied = unsafe { std::ptr::read(&rect as *const AdRect) };
    assert_eq!(copied.x, 1.25);
    assert_eq!(copied.y, -2.5);
    assert_eq!(copied.width, 640.0);
    assert_eq!(copied.height, 480.0);

    let point = AdPoint { x: 3.0, y: 4.0 };
    let copied = unsafe { std::ptr::read(&point as *const AdPoint) };
    assert_eq!(copied.x, 3.0);
    assert_eq!(copied.y, 4.0);
}

#[test]
fn ref_entry_input_caps_match_the_published_header_values() {
    assert_eq!(agent_desktop_ffi::types::ref_entry::AD_MAX_REF_STATES, 64);
    assert_eq!(agent_desktop_ffi::types::ref_entry::AD_MAX_REF_ACTIONS, 32);
    assert_eq!(
        agent_desktop_ffi::types::ref_entry::AD_MAX_REF_PATH_DEPTH,
        128
    );
}

#[test]
fn ref_entry_layout_is_guarded_for_c_consumers() {
    assert_eq!(agent_desktop_ffi::types::ref_entry::AD_REF_ENTRY_SIZE, 192);
    assert_eq!(
        unsafe { common::ad_ref_entry_size() },
        agent_desktop_ffi::types::ref_entry::AD_REF_ENTRY_SIZE
    );
    assert_eq!(size_of::<AdRefEntry>(), 192);
    assert_eq!(align_of::<AdRefEntry>(), align_of::<usize>());
    assert_eq!(offset_of!(AdRefEntry, pid), 0);

    let offsets = [
        offset_of!(AdRefEntry, pid),
        offset_of!(AdRefEntry, role),
        offset_of!(AdRefEntry, name),
        offset_of!(AdRefEntry, value),
        offset_of!(AdRefEntry, description),
        offset_of!(AdRefEntry, states),
        offset_of!(AdRefEntry, state_count),
        offset_of!(AdRefEntry, available_actions),
        offset_of!(AdRefEntry, available_action_count),
        offset_of!(AdRefEntry, bounds),
        offset_of!(AdRefEntry, has_bounds),
        offset_of!(AdRefEntry, bounds_hash),
        offset_of!(AdRefEntry, has_bounds_hash),
        offset_of!(AdRefEntry, source_app),
        offset_of!(AdRefEntry, source_window_id),
        offset_of!(AdRefEntry, source_window_title),
        offset_of!(AdRefEntry, source_surface),
        offset_of!(AdRefEntry, root_ref),
        offset_of!(AdRefEntry, path_is_absolute),
        offset_of!(AdRefEntry, path),
        offset_of!(AdRefEntry, path_count),
    ];
    assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]));

    let copied = unsafe {
        let entry = MaybeUninit::<AdRefEntry>::zeroed().assume_init();
        std::ptr::read(&entry as *const AdRefEntry)
    };
    assert_eq!(copied.pid, 0);
    assert_eq!(copied.path_count, 0);
}
