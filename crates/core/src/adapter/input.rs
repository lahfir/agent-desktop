use crate::{
    action::{DragParams, KeyCombo, MouseEvent},
    error::AdapterError,
};

pub trait InputOps: Send + Sync {
    fn mouse_event(&self, _event: MouseEvent) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("mouse_event"))
    }

    fn key_event(&self, _combo: &KeyCombo, _down: bool) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("key_event"))
    }

    fn drag(&self, _params: DragParams) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("drag"))
    }

    fn get_clipboard(&self) -> Result<String, AdapterError> {
        Err(AdapterError::not_supported("get_clipboard"))
    }

    fn set_clipboard(&self, _text: &str) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("set_clipboard"))
    }

    fn clear_clipboard(&self) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("clear_clipboard"))
    }

    fn get_clipboard_content(
        &self,
        _format: crate::clipboard_content::ClipboardFormat,
    ) -> Result<crate::clipboard_content::ClipboardContent, AdapterError> {
        Err(AdapterError::not_supported("get_clipboard_content"))
    }

    fn set_clipboard_content(
        &self,
        _content: &crate::clipboard_content::ClipboardContent,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("set_clipboard_content"))
    }

    fn mouse_wheel(
        &self,
        _x: f64,
        _y: f64,
        _dy: i32,
        _dx: i32,
        _modifiers: &[crate::action::Modifier],
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("mouse_wheel"))
    }
}
