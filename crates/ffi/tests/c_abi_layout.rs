mod common;

use common::{AdPoint, AdRect, AdRefEntry};
use std::mem::{MaybeUninit, align_of, offset_of, size_of};

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
fn ref_entry_layout_is_guarded_for_c_consumers() {
    assert_eq!(
        agent_desktop_ffi::types::ref_entry::AD_REF_ENTRY_SIZE,
        size_of::<AdRefEntry>()
    );
    assert_eq!(
        unsafe { common::ad_ref_entry_size() },
        size_of::<AdRefEntry>()
    );
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
