use crate::convert::string::c_to_string;
use crate::types::AdWindowInfo;
use agent_desktop_core::error::{AdapterError, ErrorCode};

/// Converts an `AdWindowInfo` from C into the core `WindowInfo`.
///
/// The `id` and `title` fields are mandatory in the ABI contract — null
/// or non-UTF-8 inputs would silently coerce to an empty string and
/// match the wrong window. The function returns `InvalidArgs` so the
/// caller can propagate the error to the consumer instead.
///
/// `app_name` is allowed to be empty (some Electron apps report blank
/// window owners) and is filled in from the platform adapter as needed.
pub(crate) fn ad_window_to_core(
    w: &AdWindowInfo,
) -> Result<agent_desktop_core::node::WindowInfo, AdapterError> {
    let id = unsafe { c_to_string(w.id) }.ok_or_else(|| {
        AdapterError::new(ErrorCode::InvalidArgs, "window id is null or invalid UTF-8")
    })?;
    let title = unsafe { c_to_string(w.title) }.ok_or_else(|| {
        AdapterError::new(
            ErrorCode::InvalidArgs,
            "window title is null or invalid UTF-8",
        )
    })?;
    let app = unsafe { c_to_string(w.app_name) }.unwrap_or_default();
    Ok(agent_desktop_core::node::WindowInfo {
        id,
        title,
        app,
        pid: w.pid,
        bounds: if w.has_bounds {
            Some(agent_desktop_core::node::Rect {
                x: w.bounds.x,
                y: w.bounds.y,
                width: w.bounds.width,
                height: w.bounds.height,
            })
        } else {
            None
        },
        is_focused: w.is_focused,
    })
}
