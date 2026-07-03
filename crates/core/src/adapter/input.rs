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
}
