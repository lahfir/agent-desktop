use agent_desktop_core::node::Rect;

use super::AXElement;

pub struct TreeBuildContext {
    pub(crate) focused: Option<AXElement>,
    pub(crate) window_bounds: Option<Rect>,
    include_bounds: bool,
}

impl TreeBuildContext {
    pub fn for_pid(pid: i32, include_bounds: bool) -> Self {
        let app = super::element_for_pid(pid);
        Self {
            focused: super::copy_element_attr(&app, "AXFocusedUIElement"),
            window_bounds: None,
            include_bounds,
        }
    }

    pub fn empty(include_bounds: bool) -> Self {
        Self {
            focused: None,
            window_bounds: None,
            include_bounds,
        }
    }

    pub(crate) fn child_context(&self, window_bounds: Option<Rect>) -> Self {
        Self {
            focused: self.focused.clone(),
            window_bounds: window_bounds.or(self.window_bounds),
            include_bounds: self.include_bounds,
        }
    }

    pub(crate) fn bounds_for(
        &self,
        bounds: Option<agent_desktop_core::node::Rect>,
    ) -> Option<agent_desktop_core::node::Rect> {
        if self.include_bounds { bounds } else { None }
    }
}
