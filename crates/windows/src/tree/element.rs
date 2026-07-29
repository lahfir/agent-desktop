use agent_desktop_core::{AdapterError, ErrorCode, NativeHandle};

#[cfg(target_os = "windows")]
mod imp {
    /// Owns one UI Automation element for the crate.
    ///
    /// Refcounting is delegated on purpose. `uiautomation::UIElement` wraps a
    /// `windows-core` COM interface whose `Clone` is `AddRef` and whose `Drop`
    /// is `Release`, so a hand-written pair here would release twice. The
    /// encapsulation the macOS wrapper establishes is kept: the inner field is
    /// crate-visible only, there is no `Copy`, and there is no raw accessor.
    #[derive(Clone)]
    pub struct UIAElement(pub(crate) uiautomation::UIElement);

    impl From<uiautomation::UIElement> for UIAElement {
        fn from(element: uiautomation::UIElement) -> Self {
            Self(element)
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    /// Canned stand-in so every tree module compiles on a non-Windows lane.
    #[derive(Clone)]
    pub struct UIAElement(pub(crate) CannedElement);

    #[derive(Clone, Default)]
    pub struct CannedElement;

    impl From<CannedElement> for UIAElement {
        fn from(element: CannedElement) -> Self {
            Self(element)
        }
    }
}

pub use imp::UIAElement;

#[cfg(not(target_os = "windows"))]
pub use imp::CannedElement;

impl UIAElement {
    /// Moves ownership of the element into a core `NativeHandle`.
    ///
    /// By value, so a caller cannot keep a second reference to the same
    /// wrapper through this path.
    pub fn into_native_handle(self) -> NativeHandle {
        NativeHandle::new(self)
    }
}

/// Borrows the UI Automation element carried by a core `NativeHandle`.
///
/// A handle built by another platform, or an empty one, is rejected rather
/// than reinterpreted: `downcast_ref` is a type check, never a pointer cast.
pub fn uia_element(handle: &NativeHandle) -> Result<&UIAElement, AdapterError> {
    handle.downcast_ref::<UIAElement>().ok_or_else(|| {
        AdapterError::new(
            ErrorCode::InvalidArgs,
            "Native handle does not contain a Windows UI Automation element",
        )
        .with_details(serde_json::json!({
            "kind": "invalid_native_handle",
            "platform": "windows",
            "empty": handle.is_null()
        }))
    })
}

#[cfg(test)]
#[path = "element_tests.rs"]
mod tests;
