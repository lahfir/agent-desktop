pub struct MacOSAdapter;

impl MacOSAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOSAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn with_borrowed_ax_element<T>(
    handle: &agent_desktop_core::adapter::NativeHandle,
    f: impl FnOnce(&crate::tree::AXElement) -> T,
) -> T {
    use std::mem::ManuallyDrop;

    let el = ManuallyDrop::new(crate::tree::AXElement(
        handle.as_raw() as accessibility_sys::AXUIElementRef
    ));
    f(&el)
}
