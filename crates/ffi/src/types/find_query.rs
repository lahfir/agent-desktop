use std::os::raw::c_char;

#[repr(C)]
pub struct AdFindQuery {
    pub role: *const c_char,
    pub name_substring: *const c_char,
    pub value_substring: *const c_char,
}
