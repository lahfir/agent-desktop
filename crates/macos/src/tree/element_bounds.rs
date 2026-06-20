use agent_desktop_core::node::Rect;

use super::AXElement;

#[cfg(target_os = "macos")]
pub(crate) fn rect_from_parts(
    point: core_graphics::geometry::CGPoint,
    size: core_graphics::geometry::CGSize,
) -> Option<Rect> {
    if !point.x.is_finite()
        || !point.y.is_finite()
        || !size.width.is_finite()
        || !size.height.is_finite()
    {
        return None;
    }
    Some(Rect {
        x: point.x,
        y: point.y,
        width: size.width,
        height: size.height,
    })
}

#[cfg(target_os = "macos")]
pub fn read_bounds(el: &AXElement) -> Option<Rect> {
    use accessibility_sys::{
        AXUIElementCopyAttributeValue, AXValueGetValue, kAXErrorSuccess, kAXPositionAttribute,
        kAXSizeAttribute, kAXValueTypeCGPoint, kAXValueTypeCGSize,
    };
    use core_foundation::{
        base::{CFRelease, CFTypeRef, TCFType},
        string::CFString,
    };
    use core_graphics::geometry::{CGPoint, CGSize};
    use std::ffi::c_void;

    let pos_cf = CFString::new(kAXPositionAttribute);
    let mut pos_ref: CFTypeRef = std::ptr::null_mut();
    let pos_ok =
        unsafe { AXUIElementCopyAttributeValue(el.0, pos_cf.as_concrete_TypeRef(), &mut pos_ref) };
    if pos_ok != kAXErrorSuccess || pos_ref.is_null() {
        return None;
    }

    let mut point = CGPoint::new(0.0, 0.0);
    let got_pos = unsafe {
        AXValueGetValue(
            pos_ref as _,
            kAXValueTypeCGPoint,
            &mut point as *mut _ as *mut c_void,
        )
    };
    unsafe { CFRelease(pos_ref) };
    if !got_pos {
        return None;
    }

    let size_cf = CFString::new(kAXSizeAttribute);
    let mut size_ref: CFTypeRef = std::ptr::null_mut();
    let size_ok = unsafe {
        AXUIElementCopyAttributeValue(el.0, size_cf.as_concrete_TypeRef(), &mut size_ref)
    };
    if size_ok != kAXErrorSuccess || size_ref.is_null() {
        return None;
    }

    let mut size = CGSize::new(0.0, 0.0);
    let got_size = unsafe {
        AXValueGetValue(
            size_ref as _,
            kAXValueTypeCGSize,
            &mut size as *mut _ as *mut c_void,
        )
    };
    unsafe { CFRelease(size_ref) };
    if !got_size {
        return None;
    }

    rect_from_parts(point, size)
}

#[cfg(not(target_os = "macos"))]
pub fn read_bounds(_el: &AXElement) -> Option<Rect> {
    None
}
