use super::*;
use core_foundation::{base::TCFType, number::CFNumber, string::CFString};

#[test]
fn ns_string_rejects_null() {
    assert_eq!(unsafe { ns_string(std::ptr::null_mut()) }, None);
}

#[test]
fn ns_string_rejects_non_string_object() {
    let number = CFNumber::from(42);
    let value = number.as_CFTypeRef() as Id;

    assert_eq!(unsafe { ns_string(value) }, None);
}

#[test]
fn ns_string_accepts_cf_string_object() {
    let string = CFString::new("Mail");
    let value = string.as_CFTypeRef() as Id;

    assert_eq!(unsafe { ns_string(value) }.as_deref(), Some("Mail"));
}

#[test]
fn autorelease_pool_ignores_null_push_result() {
    drop(AutoreleasePool(None));
}
