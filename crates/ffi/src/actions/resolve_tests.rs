use super::*;
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
