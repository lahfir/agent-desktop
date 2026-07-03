#[cfg(target_os = "macos")]
mod imp {
    use crate::actions::ax_helpers;
    use crate::tree::AXElement;
    use agent_desktop_core::error::AdapterError;

    pub fn scroll_into_view_impl(el: &AXElement) -> Result<(), AdapterError> {
        if ax_helpers::try_ax_action(el, "AXScrollToVisible") {
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
