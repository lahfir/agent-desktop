use agent_desktop_core::{action::WindowOp, error::AdapterError, node::WindowInfo};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use accessibility_sys::{
        AXUIElementSetAttributeValue, kAXErrorSuccess, kAXPositionAttribute, kAXSizeAttribute,
        kAXValueTypeCGPoint, kAXValueTypeCGSize,
    };
    use agent_desktop_core::error::ErrorCode;
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};
    use core_graphics::geometry::{CGPoint, CGSize};
    use std::ffi::c_void;

    unsafe extern "C" {
        fn AXValueCreate(value_type: u32, value_ptr: *const c_void) -> *mut c_void;
    }

    pub fn execute(win: &WindowInfo, op: WindowOp) -> Result<(), AdapterError> {
        tracing::debug!(
            "system: window_op {:?} app={:?} title={:?}",
            op,
            win.app,
            win.title
        );
        let win_el = crate::tree::window_element_for(win.pid, &win.title);
        match op {
            WindowOp::Resize { width, height } => set_size(&win_el, width, height),
            WindowOp::Move { x, y } => set_position(&win_el, x, y),
            WindowOp::Minimize => set_minimized(&win_el, true),
            WindowOp::Maximize => maximize_to_main_display(&win_el),
            WindowOp::Restore => set_minimized(&win_el, false),
        }
    }

    fn maximize_to_main_display(el: &crate::tree::AXElement) -> Result<(), AdapterError> {
        let bounds = core_graphics::display::CGDisplay::main().bounds();
        set_position(el, bounds.origin.x, bounds.origin.y)?;
        set_size(el, bounds.size.width, bounds.size.height)
    }

    fn set_size(el: &crate::tree::AXElement, width: f64, height: f64) -> Result<(), AdapterError> {
        let size = CGSize::new(width, height);
        let ax_value =
            unsafe { AXValueCreate(kAXValueTypeCGSize, &size as *const _ as *const c_void) };
        if ax_value.is_null() {
            return Err(AdapterError::internal("Failed to create AXValue for size"));
        }
        let cf_attr = CFString::new(kAXSizeAttribute);
        let err = unsafe {
            AXUIElementSetAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), ax_value as _)
        };
        unsafe { core_foundation::base::CFRelease(ax_value as _) };
        if err != kAXErrorSuccess {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                format!("Resize failed (err={err})"),
            )
            .with_suggestion("Window may not support resizing. Try a different size."));
        }
        Ok(())
    }

    fn set_position(el: &crate::tree::AXElement, x: f64, y: f64) -> Result<(), AdapterError> {
        let point = CGPoint::new(x, y);
        let ax_value =
            unsafe { AXValueCreate(kAXValueTypeCGPoint, &point as *const _ as *const c_void) };
        if ax_value.is_null() {
            return Err(AdapterError::internal(
                "Failed to create AXValue for position",
            ));
        }
        let cf_attr = CFString::new(kAXPositionAttribute);
        let err = unsafe {
            AXUIElementSetAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), ax_value as _)
        };
        unsafe { core_foundation::base::CFRelease(ax_value as _) };
        if err != kAXErrorSuccess {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                format!("Move failed (err={err})"),
            )
            .with_suggestion(
                "Window may not support repositioning. Verify coordinates are on-screen.",
            ));
        }
        Ok(())
    }

    fn set_minimized(el: &crate::tree::AXElement, minimized: bool) -> Result<(), AdapterError> {
        let cf_attr = CFString::new("AXMinimized");
        let val = if minimized {
            CFBoolean::true_value()
        } else {
            CFBoolean::false_value()
        };
        let err = unsafe {
            AXUIElementSetAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), val.as_CFTypeRef())
        };
        if err != kAXErrorSuccess {
            let op = if minimized { "Minimize" } else { "Restore" };
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                format!("{op} failed (err={err})"),
            )
            .with_suggestion("Window may not support this operation. Try 'focus-window' first."));
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod raise {
    use accessibility_sys::{
        AXUIElementPerformAction, AXUIElementSetAttributeValue, kAXErrorSuccess,
    };
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};

    /// Raises a window element to the top of its app's window stack and
    /// confirms with a brief `AXMain` poll. CGEvents land on the topmost
    /// window at the click point, so a frontmost app is not enough when the
    /// target element lives in a background window. Best-effort: a window
    /// that refuses both `AXRaise` and a settable `AXMain` is left as-is.
    pub(crate) fn raise_window(window: &crate::tree::AXElement) {
        let raise = CFString::new("AXRaise");
        let raise_err = unsafe { AXUIElementPerformAction(window.0, raise.as_concrete_TypeRef()) };
        if raise_err != kAXErrorSuccess {
            let main_attr = CFString::new("AXMain");
            let ax_err = unsafe {
                AXUIElementSetAttributeValue(
                    window.0,
                    main_attr.as_concrete_TypeRef(),
                    CFBoolean::true_value().as_CFTypeRef(),
                )
            };
            if ax_err != kAXErrorSuccess {
                tracing::debug!("raise_window: AXMain fallback returned err={ax_err}");
            }
        }
        wait_until_main(window);
    }

    fn wait_until_main(window: &crate::tree::AXElement) {
        use std::time::{Duration, Instant};

        const POLL_INTERVAL: Duration = Duration::from_millis(5);
        const MAIN_DEADLINE: Duration = Duration::from_millis(50);

        let deadline = Instant::now() + MAIN_DEADLINE;
        loop {
            if crate::tree::copy_bool_attr(window, "AXMain") == Some(true) {
                return;
            }
            if Instant::now() >= deadline {
                return;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use raise::raise_window;

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn execute(_win: &WindowInfo, _op: WindowOp) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("window_op"))
    }
}

pub use imp::execute;
