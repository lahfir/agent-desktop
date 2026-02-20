use agent_desktop_core::{action::WindowOp, error::AdapterError, node::WindowInfo};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use agent_desktop_core::error::ErrorCode;
    use accessibility_sys::{
        kAXErrorSuccess, kAXPositionAttribute, kAXSizeAttribute,
        AXUIElementPerformAction, AXUIElementSetAttributeValue,
        kAXValueTypeCGPoint, kAXValueTypeCGSize,
    };
    use core_foundation::{
        base::TCFType,
        boolean::CFBoolean,
        string::CFString,
    };
    use core_graphics::geometry::{CGPoint, CGSize};
    use std::ffi::c_void;

    extern "C" {
        fn AXValueCreate(value_type: u32, value_ptr: *const c_void) -> *mut c_void;
    }

    pub fn execute(win: &WindowInfo, op: WindowOp) -> Result<(), AdapterError> {
        let win_el = crate::tree::window_element_for(win.pid, &win.title);
        match op {
            WindowOp::Resize { width, height } => set_size(&win_el, width, height),
            WindowOp::Move { x, y } => set_position(&win_el, x, y),
            WindowOp::Minimize => set_minimized(&win_el, true),
            WindowOp::Maximize => press_zoom_button(&win_el),
            WindowOp::Restore => set_minimized(&win_el, false),
        }
    }

    fn set_size(
        el: &crate::tree::AXElement,
        width: f64,
        height: f64,
    ) -> Result<(), AdapterError> {
        let size = CGSize::new(width, height);
        let ax_value = unsafe {
            AXValueCreate(kAXValueTypeCGSize, &size as *const _ as *const c_void)
        };
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
            ));
        }
        Ok(())
    }

    fn set_position(
        el: &crate::tree::AXElement,
        x: f64,
        y: f64,
    ) -> Result<(), AdapterError> {
        let point = CGPoint::new(x, y);
        let ax_value = unsafe {
            AXValueCreate(kAXValueTypeCGPoint, &point as *const _ as *const c_void)
        };
        if ax_value.is_null() {
            return Err(AdapterError::internal("Failed to create AXValue for position"));
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
            ));
        }
        Ok(())
    }

    fn set_minimized(
        el: &crate::tree::AXElement,
        minimized: bool,
    ) -> Result<(), AdapterError> {
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
            ));
        }
        Ok(())
    }

    fn press_zoom_button(el: &crate::tree::AXElement) -> Result<(), AdapterError> {
        let zoom = crate::tree::copy_element_attr(el, "AXZoomButton");
        match zoom {
            Some(btn) => {
                let action = CFString::new("AXPress");
                let err = unsafe {
                    AXUIElementPerformAction(btn.0, action.as_concrete_TypeRef())
                };
                if err != kAXErrorSuccess {
                    return Err(AdapterError::new(
                        ErrorCode::ActionFailed,
                        format!("Zoom button press failed (err={err})"),
                    ));
                }
                Ok(())
            }
            None => Err(AdapterError::new(
                ErrorCode::ActionNotSupported,
                "Window has no zoom button",
            )
            .with_suggestion("Window may not support maximizing")),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn execute(_win: &WindowInfo, _op: WindowOp) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("window_op"))
    }
}

pub use imp::execute;
