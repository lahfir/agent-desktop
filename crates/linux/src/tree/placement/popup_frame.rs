use crate::system::process::toolkit_id::ToolkitId;
use crate::system::shell::container::MENU;
use crate::tree::properties::toolkit_trust::{ToolkitKind, kind};

/// L32-43: the GTK4 menu frame belongs to an override-redirect popup,
/// independently of whether its node is marked as an action target.
pub(crate) fn uses_popup_origin(role: Option<u32>, toolkit: &ToolkitId) -> bool {
    role == Some(MENU) && kind(toolkit) == ToolkitKind::Gtk4
}

#[cfg(test)]
#[path = "popup_frame_tests.rs"]
mod tests;
