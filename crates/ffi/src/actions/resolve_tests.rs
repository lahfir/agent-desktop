use super::*;
use agent_desktop_core::error::ErrorCode;
use std::ffi::CString;

#[test]
fn ffi_ref_entry_preserves_description_identity() {
    let role = CString::new("button").unwrap();
    let name = CString::new("Primary").unwrap();
    let description = CString::new("Insert Shape").unwrap();
    let entry = AdRefEntry {
        pid: 42,
        role: role.as_ptr(),
        name: name.as_ptr(),
        description: description.as_ptr(),
        bounds_hash: 123,
        has_bounds_hash: true,
    };

    let core_entry = unsafe { core_ref_entry_from_ffi(&entry) }.unwrap();

    assert_eq!(core_entry.pid, 42);
    assert_eq!(core_entry.role, "button");
    assert_eq!(core_entry.name.as_deref(), Some("Primary"));
    assert_eq!(core_entry.description.as_deref(), Some("Insert Shape"));
    assert_eq!(core_entry.bounds_hash, Some(123));
}

#[test]
fn ffi_ref_entry_rejects_invalid_description_identity() {
    let role = CString::new("button").unwrap();
    let bad_description: [u8; 2] = [0xC3, 0x00];
    let entry = AdRefEntry {
        pid: 42,
        role: role.as_ptr(),
        name: std::ptr::null(),
        description: bad_description.as_ptr().cast(),
        bounds_hash: 0,
        has_bounds_hash: false,
    };

    let err = unsafe { core_ref_entry_from_ffi(&entry) }.unwrap_err();

    assert_eq!(err.code, ErrorCode::InvalidArgs);
    assert_eq!(err.message, "description is not valid UTF-8");
}
