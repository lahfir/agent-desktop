use crate::convert::string::{optional_adapter_string, required_adapter_string};
use crate::types::AdWindowInfo;
use agent_desktop_core::error::AdapterError;

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
    let id = required_adapter_string(w.id, "window id")?;
    let title = required_adapter_string(w.title, "window title")?;
    let app = optional_adapter_string(w.app_name, "window app_name")?.unwrap_or_default();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::string::MAX_C_STRING_BYTES;
    use crate::types::AdRect;
    use agent_desktop_core::error::ErrorCode;
    use std::ffi::CString;

    #[test]
    fn window_app_name_rejects_oversized_string() {
        let id = CString::new("w-1").unwrap();
        let title = CString::new("Main").unwrap();
        let mut app = vec![b'a'; MAX_C_STRING_BYTES + 1];
        app.push(0);
        let win = window(id.as_ptr(), title.as_ptr(), app.as_ptr().cast());

        let err = ad_window_to_core(&win).unwrap_err();

        assert_eq!(err.code, ErrorCode::InvalidArgs);
        assert!(err.message.contains("window app_name exceeds"));
    }

    #[test]
    fn window_app_name_rejects_invalid_utf8() {
        let id = CString::new("w-1").unwrap();
        let title = CString::new("Main").unwrap();
        let app = [0xff_u8, 0x00];
        let win = window(id.as_ptr(), title.as_ptr(), app.as_ptr().cast());

        let err = ad_window_to_core(&win).unwrap_err();

        assert_eq!(err.code, ErrorCode::InvalidArgs);
        assert!(err.message.contains("window app_name is not valid UTF-8"));
    }

    fn window(
        id: *const std::os::raw::c_char,
        title: *const std::os::raw::c_char,
        app_name: *const std::os::raw::c_char,
    ) -> AdWindowInfo {
        AdWindowInfo {
            id,
            title,
            app_name,
            pid: 7,
            bounds: AdRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            has_bounds: false,
            is_focused: false,
        }
    }
}
