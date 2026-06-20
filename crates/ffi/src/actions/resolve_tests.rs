use super::*;
use agent_desktop_core::error::ErrorCode;
use std::ffi::CString;

fn test_ref_entry() -> AdRefEntry {
    AdRefEntry {
        pid: 0,
        role: std::ptr::null(),
        name: std::ptr::null(),
        value: std::ptr::null(),
        description: std::ptr::null(),
        states: std::ptr::null(),
        state_count: 0,
        available_actions: std::ptr::null(),
        available_action_count: 0,
        bounds: crate::types::AdRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        has_bounds: false,
        bounds_hash: 0,
        has_bounds_hash: false,
        source_app: std::ptr::null(),
        source_window_id: std::ptr::null(),
        source_window_title: std::ptr::null(),
        source_surface: 0,
        root_ref: std::ptr::null(),
        path_is_absolute: false,
        path: std::ptr::null(),
        path_count: 0,
    }
}

#[test]
fn ffi_ref_entry_preserves_description_identity() {
    let role = CString::new("button").unwrap();
    let name = CString::new("Primary").unwrap();
    let value = CString::new("On").unwrap();
    let description = CString::new("Insert Shape").unwrap();
    let state = CString::new("focused").unwrap();
    let action = CString::new("Click").unwrap();
    let source_app = CString::new("Finder").unwrap();
    let window_id = CString::new("w-1").unwrap();
    let window_title = CString::new("Documents").unwrap();
    let root_ref = CString::new("@e1").unwrap();
    let states = [state.as_ptr()];
    let actions = [action.as_ptr()];
    let path = [1_u32, 2, 3];
    let mut entry = test_ref_entry();
    entry.pid = 42;
    entry.role = role.as_ptr();
    entry.name = name.as_ptr();
    entry.value = value.as_ptr();
    entry.description = description.as_ptr();
    entry.states = states.as_ptr();
    entry.state_count = states.len();
    entry.available_actions = actions.as_ptr();
    entry.available_action_count = actions.len();
    entry.bounds = crate::types::AdRect {
        x: 1.0,
        y: 2.0,
        width: 3.0,
        height: 4.0,
    };
    entry.has_bounds = true;
    entry.bounds_hash = 123;
    entry.has_bounds_hash = true;
    entry.source_app = source_app.as_ptr();
    entry.source_window_id = window_id.as_ptr();
    entry.source_window_title = window_title.as_ptr();
    entry.source_surface = 5;
    entry.root_ref = root_ref.as_ptr();
    entry.path_is_absolute = true;
    entry.path = path.as_ptr();
    entry.path_count = path.len();

    let core_entry = unsafe { core_ref_entry_from_ffi(&entry) }.unwrap();

    assert_eq!(core_entry.pid, 42);
    assert_eq!(core_entry.role, "button");
    assert_eq!(core_entry.name.as_deref(), Some("Primary"));
    assert_eq!(core_entry.value.as_deref(), Some("On"));
    assert_eq!(core_entry.description.as_deref(), Some("Insert Shape"));
    assert_eq!(core_entry.states, ["focused"]);
    assert_eq!(core_entry.available_actions, ["Click"]);
    assert_eq!(core_entry.bounds.unwrap().width, 3.0);
    assert_eq!(core_entry.bounds_hash, Some(123));
    assert_eq!(core_entry.source_app.as_deref(), Some("Finder"));
    assert_eq!(core_entry.source_window_id.as_deref(), Some("w-1"));
    assert_eq!(core_entry.source_window_title.as_deref(), Some("Documents"));
    assert_eq!(
        core_entry.source_surface,
        agent_desktop_core::adapter::SnapshotSurface::Popover
    );
    assert_eq!(core_entry.root_ref.as_deref(), Some("@e1"));
    assert!(core_entry.path_is_absolute);
    assert_eq!(core_entry.path.as_slice(), &[1, 2, 3]);
}

#[test]
fn ffi_ref_entry_rejects_invalid_description_identity() {
    let role = CString::new("button").unwrap();
    let bad_description: [u8; 2] = [0xC3, 0x00];
    let mut entry = test_ref_entry();
    entry.pid = 42;
    entry.role = role.as_ptr();
    entry.description = bad_description.as_ptr().cast();

    let err = unsafe { core_ref_entry_from_ffi(&entry) }.unwrap_err();

    assert_eq!(err.code, ErrorCode::InvalidArgs);
    assert_eq!(err.message, "description is not valid UTF-8");
}

#[test]
fn ffi_ref_entry_rejects_invalid_array_pointer() {
    let role = CString::new("button").unwrap();
    let mut entry = test_ref_entry();
    entry.role = role.as_ptr();
    entry.state_count = 1;

    let err = unsafe { core_ref_entry_from_ffi(&entry) }.unwrap_err();

    assert_eq!(err.code, ErrorCode::InvalidArgs);
    assert_eq!(err.message, "states count is nonzero but pointer is null");
}

#[test]
fn ffi_ref_entry_rejects_unknown_surface() {
    let role = CString::new("button").unwrap();
    let mut entry = test_ref_entry();
    entry.role = role.as_ptr();
    entry.source_surface = 99;

    let err = unsafe { core_ref_entry_from_ffi(&entry) }.unwrap_err();

    assert_eq!(err.code, ErrorCode::InvalidArgs);
    assert_eq!(err.message, "invalid source_surface discriminant");
}

fn string_array_of(len: usize) -> (Vec<CString>, Vec<*const std::os::raw::c_char>) {
    let owned: Vec<CString> = (0..len)
        .map(|i| CString::new(format!("item-{i}")).unwrap())
        .collect();
    let ptrs = owned.iter().map(|s| s.as_ptr()).collect();
    (owned, ptrs)
}

#[test]
fn ffi_ref_entry_rejects_oversized_state_count() {
    let role = CString::new("button").unwrap();
    let (_owned, ptrs) = string_array_of(crate::types::ref_entry::AD_MAX_REF_STATES + 1);
    let mut entry = test_ref_entry();
    entry.role = role.as_ptr();
    entry.states = ptrs.as_ptr();
    entry.state_count = ptrs.len();

    let err = unsafe { core_ref_entry_from_ffi(&entry) }.unwrap_err();

    assert_eq!(err.code, ErrorCode::InvalidArgs);
    assert!(err.message.contains("AD_MAX_REF_STATES"));
}

#[test]
fn ffi_ref_entry_rejects_oversized_action_count() {
    let role = CString::new("button").unwrap();
    let (_owned, ptrs) = string_array_of(crate::types::ref_entry::AD_MAX_REF_ACTIONS + 1);
    let mut entry = test_ref_entry();
    entry.role = role.as_ptr();
    entry.available_actions = ptrs.as_ptr();
    entry.available_action_count = ptrs.len();

    let err = unsafe { core_ref_entry_from_ffi(&entry) }.unwrap_err();

    assert_eq!(err.code, ErrorCode::InvalidArgs);
    assert!(err.message.contains("AD_MAX_REF_ACTIONS"));
}

#[test]
fn ffi_ref_entry_rejects_oversized_path_count() {
    let role = CString::new("button").unwrap();
    let path: Vec<u32> = (0..(crate::types::ref_entry::AD_MAX_REF_PATH_DEPTH as u32 + 1)).collect();
    let mut entry = test_ref_entry();
    entry.role = role.as_ptr();
    entry.path = path.as_ptr();
    entry.path_count = path.len();

    let err = unsafe { core_ref_entry_from_ffi(&entry) }.unwrap_err();

    assert_eq!(err.code, ErrorCode::InvalidArgs);
    assert!(err.message.contains("AD_MAX_REF_PATH_DEPTH"));
}

#[test]
fn ffi_ref_entry_rejects_unterminated_name_within_byte_cap() {
    let role = CString::new("button").unwrap();
    let unterminated = vec![b'a'; crate::convert::string::MAX_C_STRING_BYTES + 1];
    let mut entry = test_ref_entry();
    entry.role = role.as_ptr();
    entry.name = unterminated.as_ptr().cast();

    let err = unsafe { core_ref_entry_from_ffi(&entry) }.unwrap_err();

    assert_eq!(err.code, ErrorCode::InvalidArgs);
    assert!(err.message.contains("name exceeds AD_MAX_STRING_BYTES"));
}
