use std::marker::PhantomData;

pub struct NativeHandle {
    pub(crate) ptr: *const std::ffi::c_void,
    _not_send_sync: PhantomData<*const ()>,
}

impl NativeHandle {
    /// # Safety
    ///
    /// `ptr` must be a valid platform accessibility handle whose ownership is
    /// transferred to the caller. The adapter that creates the handle must
    /// document how it is released through [`crate::adapter::ActionOps::release_handle`].
    pub unsafe fn from_ptr(ptr: *const std::ffi::c_void) -> Self {
        Self {
            ptr,
            _not_send_sync: PhantomData,
        }
    }

    pub fn null() -> Self {
        Self {
            ptr: std::ptr::null(),
            _not_send_sync: PhantomData,
        }
    }

    /// Returns the raw platform pointer. For use by platform adapter crates only.
    /// Callers must not retain the pointer beyond the lifetime of this handle.
    pub fn as_raw(&self) -> *const std::ffi::c_void {
        self.ptr
    }
}
