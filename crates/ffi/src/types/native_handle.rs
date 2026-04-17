#[repr(C)]
pub struct AdNativeHandle {
    pub ptr: *const std::ffi::c_void,
}
