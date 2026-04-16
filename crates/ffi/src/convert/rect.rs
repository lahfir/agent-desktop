use crate::types::AdRect;
use agent_desktop_core::node::Rect;

#[allow(dead_code)]
pub(crate) fn rect_to_c(r: &Rect) -> AdRect {
    AdRect {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}
