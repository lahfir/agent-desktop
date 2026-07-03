#[cfg(target_os = "macos")]
mod imp {
    use crate::tree::AXElement;
    use accessibility_sys::{AXUIElementPerformAction, kAXErrorSuccess};
    use agent_desktop_core::error::AdapterError;
    use core_foundation::{base::TCFType, string::CFString};

    pub fn scroll_into_view_impl(el: &AXElement) -> Result<(), AdapterError> {
        let action = CFString::new("AXScrollToVisible");
        let err = unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
        if err == kAXErrorSuccess {
            return Ok(());
        }
        Err(AdapterError::not_supported(
            "AXScrollToVisible is not available for this element",
        ))
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::AXElement;
    use agent_desktop_core::error::AdapterError;

    pub fn scroll_into_view_impl(_el: &AXElement) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("scroll_into_view"))
    }
}

pub use imp::scroll_into_view_impl;
