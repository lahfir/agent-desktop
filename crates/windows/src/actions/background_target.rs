//! Window and element reads for the window-message `type` leg in
//! `background_keyboard`: whether the target lives in a Chromium top-level
//! window, that window's state, and the target's focus and caret.
//!
//! The emptiness check reads `ValuePattern`, not the text range: Chromium
//! answers an empty field's document range with the field's accessible
//! name. A password field is never read and counts as empty.

#[cfg(target_os = "windows")]
mod imp {
    use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
    use uiautomation::patterns::{UITextPattern, UIValuePattern};
    use uiautomation::types::TextPatternRangeEndpoint;
    use windows::Win32::Foundation::HWND;
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetDesktopWindow, GetForegroundWindow, IsIconic,
    };

    use crate::system::window::window_ops::window_class_name;
    use crate::tree::automation::{automation_client, uia_error};
    use crate::tree::element::UIAElement;
    use crate::tree::walk::walker::DEFAULT_MAX_RAW_DEPTH;
    use crate::tree::walk::walker_source::parent_step;

    const CHROMIUM_WINDOW_CLASS: &str = "Chrome_WidgetWin_1";

    pub(crate) fn chromium_root(
        element: &UIAElement,
        deadline: Deadline,
    ) -> Result<Option<isize>, AdapterError> {
        let Some(top) = top_level_handle(element, deadline)? else {
            return Ok(None);
        };
        let is_chromium = window_class_name(top as _).as_deref() == Some(CHROMIUM_WINDOW_CLASS);
        Ok(is_chromium.then_some(top))
    }

    /// The highest ancestor window handle below the desktop. Chromium parents
    /// its render-widget child window to a hidden `Chrome_WidgetWin_0` while
    /// the browser window is inactive, so the first handle above the element
    /// and its `GA_ROOT` name that hidden window, not the visible one.
    fn top_level_handle(
        element: &UIAElement,
        deadline: Deadline,
    ) -> Result<Option<isize>, AdapterError> {
        let client = automation_client()?;
        let walker = client
            .get_raw_view_walker()
            .map_err(|error| uia_error(&error, "read the target's window ancestors"))?;
        let desktop = unsafe { GetDesktopWindow() } as isize;
        let mut top = None;
        let mut current = element.clone();
        for _ in 0..DEFAULT_MAX_RAW_DEPTH {
            deadline.ensure_remaining()?;
            let handle = current
                .0
                .get_native_window_handle()
                .map(|handle| {
                    let hwnd: HWND = handle.into();
                    hwnd.0 as isize
                })
                .unwrap_or(0);
            if handle != 0 && handle != desktop {
                top = Some(handle);
            }
            current = match parent_step(&walker, &current) {
                Ok(Some(parent)) => parent,
                Ok(None) => return Ok(top),
                Err(()) => return Err(unreadable_ancestor()),
            };
        }
        Err(unreadable_ancestor())
    }

    fn unreadable_ancestor() -> AdapterError {
        AdapterError::new(
            ErrorCode::StaleRef,
            "The window hosting the input target could not be read",
        )
        .with_suggestion("Run 'snapshot' to refresh, then retry with the updated ref.")
        .with_disposition(DeliverySemantics::not_delivered())
    }

    pub(crate) fn is_minimized(root: isize) -> bool {
        unsafe { IsIconic(root as _) != 0 }
    }

    pub(crate) fn is_foreground(root: isize) -> bool {
        unsafe { GetForegroundWindow() as isize == root }
    }

    pub(crate) fn screen_to_client(root: isize, x: i32, y: i32) -> Option<(i32, i32)> {
        let mut point = POINT { x, y };
        let converted = unsafe { ScreenToClient(root as _, &mut point) };
        (converted != 0).then_some((point.x, point.y))
    }

    pub(crate) fn has_keyboard_focus(element: &UIAElement) -> bool {
        element.0.has_keyboard_focus().unwrap_or(false)
    }

    pub(crate) fn holds_text(element: &UIAElement) -> bool {
        if element.0.is_password().unwrap_or(true) {
            return false;
        }
        element
            .0
            .get_pattern::<UIValuePattern>()
            .and_then(|pattern| pattern.get_value())
            .is_ok_and(|value| !value.is_empty())
    }

    pub(crate) fn select_end_of_text(element: &UIAElement) -> bool {
        let Ok(pattern) = element.0.get_pattern::<UITextPattern>() else {
            return false;
        };
        let Ok(range) = pattern.get_document_range() else {
            return false;
        };
        range
            .move_endpoint_by_range(
                TextPatternRangeEndpoint::Start,
                &range,
                TextPatternRangeEndpoint::End,
            )
            .and_then(|()| range.select())
            .is_ok()
    }

    /// Whether no text follows the caret. Chromium reports the caret's end
    /// endpoint as past the document's end when the two coincide, so the
    /// endpoints are not compared; the text between them is read instead.
    pub(crate) fn caret_at_end(element: &UIAElement) -> bool {
        let Ok(pattern) = element.0.get_pattern::<UITextPattern>() else {
            return false;
        };
        let (Ok(selection), Ok(tail)) = (pattern.get_selection(), pattern.get_document_range())
        else {
            return false;
        };
        let Some(caret) = selection.first() else {
            return false;
        };
        tail.move_endpoint_by_range(
            TextPatternRangeEndpoint::Start,
            caret,
            TextPatternRangeEndpoint::End,
        )
        .and_then(|()| tail.get_text(-1))
        .is_ok_and(|text| text.is_empty())
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use agent_desktop_core::{AdapterError, Deadline};

    use crate::tree::element::UIAElement;

    pub(crate) fn chromium_root(
        _element: &UIAElement,
        _deadline: Deadline,
    ) -> Result<Option<isize>, AdapterError> {
        Ok(None)
    }

    pub(crate) fn is_minimized(_root: isize) -> bool {
        false
    }

    pub(crate) fn is_foreground(_root: isize) -> bool {
        false
    }

    pub(crate) fn screen_to_client(_root: isize, _x: i32, _y: i32) -> Option<(i32, i32)> {
        None
    }

    pub(crate) fn has_keyboard_focus(_element: &UIAElement) -> bool {
        false
    }

    pub(crate) fn holds_text(_element: &UIAElement) -> bool {
        false
    }

    pub(crate) fn select_end_of_text(_element: &UIAElement) -> bool {
        false
    }

    pub(crate) fn caret_at_end(_element: &UIAElement) -> bool {
        false
    }
}

pub(crate) use imp::{
    caret_at_end, chromium_root, has_keyboard_focus, holds_text, is_foreground, is_minimized,
    screen_to_client, select_end_of_text,
};
