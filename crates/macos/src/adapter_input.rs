use agent_desktop_core::{
    action::{DragParams, KeyCombo, Modifier, MouseEvent},
    adapter::InputOps,
    clipboard_content::{ClipboardContent, ClipboardFormat},
    error::AdapterError,
};

use crate::adapter::MacOSAdapter;

impl InputOps for MacOSAdapter {
    fn mouse_event(&self, event: MouseEvent) -> Result<(), AdapterError> {
        crate::input::mouse::synthesize_mouse(event)
    }

    fn key_event(&self, combo: &KeyCombo, down: bool) -> Result<(), AdapterError> {
        crate::input::keyboard::synthesize_key_state(combo, down)
    }

    fn drag(&self, params: DragParams) -> Result<(), AdapterError> {
        crate::input::mouse::synthesize_drag(params)
    }

    fn clear_clipboard(&self) -> Result<(), AdapterError> {
        crate::input::clipboard::clear()
    }

    fn get_clipboard_content(
        &self,
        format: ClipboardFormat,
    ) -> Result<Option<ClipboardContent>, AdapterError> {
        crate::input::clipboard::get_content(format)
    }

    fn set_clipboard_content(&self, content: &ClipboardContent) -> Result<(), AdapterError> {
        crate::input::clipboard::set_content(content)
    }

    fn mouse_wheel(
        &self,
        x: f64,
        y: f64,
        dy: i32,
        dx: i32,
        modifiers: &[Modifier],
    ) -> Result<(), AdapterError> {
        crate::input::mouse::synthesize_scroll_at(x, y, dy, dx, modifiers)
    }
}
