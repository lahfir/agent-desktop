//! Where a text element's insertion point sits on screen, read from the text
//! system rather than inferred from the element's bounds: the bounds of the
//! character before the insertion point, or, after a line break or in an
//! empty field, the start of the line it opens.

#[cfg(target_os = "macos")]
mod imp {
    use crate::tree::AXElement;
    use agent_desktop_core::{AdapterError, Deadline, ErrorCode, Rect};

    const CARET_WIDTH: f64 = 2.0;
    const MIN_CARET_HEIGHT: f64 = 10.0;

    /// The insertion caret of the element with keyboard focus in `pid`, or in
    /// the focused app when `pid` is `None`.
    pub(crate) fn focused_insertion_caret(
        pid: Option<i32>,
        deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        let owner = match pid {
            Some(pid) => crate::tree::element_for_pid(pid),
            None => AXElement(unsafe { accessibility_sys::AXUIElementCreateSystemWide() }),
        };
        let focused = super::super::attributes::copy_element_attr_result(
            &owner,
            "AXFocusedUIElement",
            deadline,
        )
        .map_err(|error| {
            AdapterError::new(
                ErrorCode::ActionFailed,
                "Could not read the focused element",
            )
            .with_details(serde_json::json!({ "ax_error": error }))
        })?;
        Ok(focused.and_then(|focused| insertion_caret(&focused, deadline)))
    }

    pub(crate) fn insertion_caret(element: &AXElement, deadline: Deadline) -> Option<Rect> {
        let selection = super::super::attributes::selected_text_range(element, deadline)?;
        let at = selection.end;
        if at == 0 {
            return bounds_for_range(element, 0, 1, deadline)
                .or_else(|| bounds_for_range(element, 0, 0, deadline))
                .map(|first| caret(first.x, &first));
        }
        let previous = bounds_for_range(element, at - 1, 1, deadline)?;
        if string_for_range(element, at - 1, 1, deadline).as_deref() != Some("\n") {
            return Some(caret(previous.x + previous.width, &previous));
        }
        let left = bounds_for_range(element, 0, 1, deadline).map_or(previous.x, |first| first.x);
        Some(Rect {
            x: left,
            y: previous.y + previous.height,
            width: CARET_WIDTH,
            height: previous.height.max(MIN_CARET_HEIGHT),
        })
    }

    fn caret(x: f64, line: &Rect) -> Rect {
        Rect {
            x,
            y: line.y,
            width: CARET_WIDTH,
            height: line.height.max(MIN_CARET_HEIGHT),
        }
    }

    fn bounds_for_range(
        element: &AXElement,
        location: usize,
        length: usize,
        deadline: Deadline,
    ) -> Option<Rect> {
        use accessibility_sys::{AXValueGetValue, kAXValueTypeCGRect};
        use core_graphics::geometry::CGRect;

        let raw = copy_for_range(element, "AXBoundsForRange", location, length, deadline)?;
        let mut rect = CGRect::new(
            &core_graphics::geometry::CGPoint::new(0.0, 0.0),
            &core_graphics::geometry::CGSize::new(0.0, 0.0),
        );
        let decoded = unsafe {
            AXValueGetValue(
                raw.as_CFTypeRef() as _,
                kAXValueTypeCGRect,
                (&mut rect as *mut CGRect).cast(),
            )
        };
        (decoded && rect.size.height > 0.0).then_some(Rect {
            x: rect.origin.x,
            y: rect.origin.y,
            width: rect.size.width,
            height: rect.size.height,
        })
    }

    fn string_for_range(
        element: &AXElement,
        location: usize,
        length: usize,
        deadline: Deadline,
    ) -> Option<String> {
        use core_foundation::string::CFString;

        let raw = copy_for_range(element, "AXStringForRange", location, length, deadline)?;
        raw.downcast::<CFString>().map(|text| text.to_string())
    }

    use core_foundation::base::TCFType;

    fn copy_for_range(
        element: &AXElement,
        attribute: &str,
        location: usize,
        length: usize,
        deadline: Deadline,
    ) -> Option<core_foundation::base::CFType> {
        use accessibility_sys::{AXValueCreate, kAXErrorSuccess, kAXValueTypeCFRange};
        use core_foundation::{base::CFType, string::CFString};
        use core_foundation_sys::base::CFRange;

        let range = CFRange {
            location: isize::try_from(location).ok()?,
            length: isize::try_from(length).ok()?,
        };
        let parameter =
            unsafe { AXValueCreate(kAXValueTypeCFRange, (&range as *const CFRange).cast()) };
        if parameter.is_null() {
            return None;
        }
        let parameter = unsafe { CFType::wrap_under_create_rule(parameter as _) };
        let attribute = CFString::new(attribute);
        let (error, raw) = super::super::ax_ipc::copy_parameterized_attribute_value(
            element,
            attribute.as_concrete_TypeRef(),
            parameter.as_CFTypeRef(),
            deadline,
        );
        if raw.is_null() {
            return None;
        }
        let value = unsafe { CFType::wrap_under_create_rule(raw) };
        (error == kAXErrorSuccess).then_some(value)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::AXElement;
    use agent_desktop_core::{AdapterError, Deadline, Rect};

    pub(crate) fn insertion_caret(_element: &AXElement, _deadline: Deadline) -> Option<Rect> {
        None
    }

    pub(crate) fn focused_insertion_caret(
        _pid: Option<i32>,
        _deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Err(AdapterError::not_supported("get_focused_insertion_caret"))
    }
}

pub(crate) use imp::{focused_insertion_caret, insertion_caret};
