use crate::convert::string::c_to_string;
use crate::types::AdWindowInfo;

pub(crate) fn ad_window_to_core(w: &AdWindowInfo) -> agent_desktop_core::node::WindowInfo {
    agent_desktop_core::node::WindowInfo {
        id: unsafe { c_to_string(w.id) }.unwrap_or_default(),
        title: unsafe { c_to_string(w.title) }.unwrap_or_default(),
        app: unsafe { c_to_string(w.app_name) }.unwrap_or_default(),
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
