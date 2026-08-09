use agent_desktop_core::{AdapterError, ErrorCode};
use rustc_hash::FxHashMap;
use std::time::Instant;

type AxWindowIdentity = (Option<String>, Option<i64>);

#[derive(Clone, Debug)]
pub(crate) struct WindowAxState {
    pub(crate) focused: Option<AxWindowIdentity>,
    pub(crate) minimized_by_id: FxHashMap<i64, bool>,
}

pub(crate) fn read_until(pid: i32, deadline: Instant) -> Result<WindowAxState, AdapterError> {
    let app = crate::tree::element_for_pid(pid);
    let focused = focused_identity(&app, pid, deadline)?;
    let mut minimized_by_id = FxHashMap::default();
    for window in crate::tree::surface_read::elements(&app, "AXWindows", deadline)? {
        if crate::tree::surface_read::string(&window, "AXRole", deadline)?.as_deref()
            != Some("AXWindow")
        {
            continue;
        }
        let window_id =
            match crate::system::window_resolve::ax_window_id_with_deadline(&window, deadline) {
                Ok(window_id) => window_id,
                Err(error) if crate::system::window_bridge::is_unavailable(&error) => break,
                Err(error) => return Err(error),
            };
        let Some(window_id) = window_id else {
            continue;
        };
        if let Some(minimized) =
            crate::tree::surface_read::boolean(&window, "AXMinimized", deadline)?
        {
            minimized_by_id.insert(window_id, minimized);
        }
    }
    Ok(WindowAxState {
        focused,
        minimized_by_id,
    })
}

pub(crate) fn read_frontmost_until(
    pid: i32,
    deadline: Instant,
) -> Result<WindowAxState, AdapterError> {
    let app = crate::tree::element_for_pid(pid);
    Ok(WindowAxState {
        focused: focused_identity(&app, pid, deadline)?,
        minimized_by_id: FxHashMap::default(),
    })
}

/// Frontmost-ness only selects whether a focused window is reported, and a busy
/// application answering "unknown" is not a reason to fail the command that
/// asked. Permission and API failures still propagate, because those describe
/// the caller's access rather than the application's state.
fn is_frontmost(app: &crate::tree::AXElement, deadline: Instant) -> Result<bool, AdapterError> {
    match crate::tree::surface_read::boolean(app, "AXFrontmost", deadline) {
        Ok(frontmost) => Ok(frontmost == Some(true)),
        Err(error) if error.code == ErrorCode::PermDenied => Err(error),
        Err(_) => Ok(false),
    }
}

fn focused_identity(
    app: &crate::tree::AXElement,
    pid: i32,
    deadline: Instant,
) -> Result<Option<AxWindowIdentity>, AdapterError> {
    if !is_frontmost(app, deadline)? {
        return Ok(None);
    }
    let Some(focused) = crate::tree::surface_read::element(app, "AXFocusedWindow", deadline)?
    else {
        return Ok(None);
    };
    let role = crate::tree::surface_read::string(&focused, "AXRole", deadline)?;
    let window = if role.as_deref() == Some("AXWindow") {
        focused
    } else if role.as_deref().is_some_and(is_transient_surface_role) {
        let Some(window) = crate::tree::surface_read::element(&focused, "AXWindow", deadline)?
        else {
            return Ok(None);
        };
        if crate::tree::surface_read::string(&window, "AXRole", deadline)?.as_deref()
            != Some("AXWindow")
        {
            return Ok(None);
        }
        window
    } else {
        return Err(AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Focused accessibility object was not a window",
        )
        .with_details(serde_json::json!({ "pid": pid, "complete": false, "role": role })));
    };
    let title = crate::tree::surface_read::string(&window, "AXTitle", deadline)?;
    let number = match crate::system::window_resolve::ax_window_id_with_deadline(&window, deadline)
    {
        Ok(number) => number,
        Err(error) if crate::system::window_bridge::is_unavailable(&error) => None,
        Err(error) => return Err(error),
    };
    Ok(Some((title, number)))
}

fn is_transient_surface_role(role: &str) -> bool {
    matches!(role, "AXSheet" | "AXDialog" | "AXAlert" | "AXPopover")
}

#[cfg(test)]
mod tests {
    use super::is_transient_surface_role;

    #[test]
    fn modal_surfaces_can_temporarily_own_focused_window() {
        for role in ["AXSheet", "AXDialog", "AXAlert", "AXPopover"] {
            assert!(is_transient_surface_role(role));
        }
        assert!(!is_transient_surface_role("AXButton"));
    }
}
