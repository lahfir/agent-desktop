use crate::convert::string::c_to_str;
use crate::types::AdWindowInfo;

pub(crate) fn ad_window_to_core(w: &AdWindowInfo) -> agent_desktop_core::node::WindowInfo {
    agent_desktop_core::node::WindowInfo {
        id: unsafe { c_to_str(w.id) }.unwrap_or("").to_string(),
        title: unsafe { c_to_str(w.title) }.unwrap_or("").to_string(),
        app: unsafe { c_to_str(w.app_name) }.unwrap_or("").to_string(),
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
    }
}
