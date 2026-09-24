use crate::{
    AdapterError, BackgroundPointerReport, ClipboardContent, ClipboardFormat, Deadline, DragParams,
    InteractionLease, KeyCombo, MouseEvent, WindowInfo,
};

/// `get_clipboard`/`set_clipboard` were removed pre-1.0 in favor of
/// `get_clipboard_content`/`set_clipboard_content`; the C ABI
/// (`ad_get_clipboard`/`ad_set_clipboard`) is unaffected.
pub trait InputOps: Send + Sync {
    fn mouse_event(
        &self,
        _event: MouseEvent,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("mouse_event"))
    }

    /// Posts `event` straight to the process that owns `window` without moving
    /// the system cursor. Leaving the app inactive and keyboard focus where it
    /// was is best effort, not a guarantee: the returned report carries the
    /// frontmost application sampled before and after delivery plus the focus
    /// guard outcome (summarized by `focus_change`) as evidence of whether
    /// focus moved.
    ///
    /// Callers must have re-verified `window` (pid, process instance, and
    /// exact window id) under `lease` and checked that the point lies inside
    /// its bounds. Only `Move` and `Click` events are meaningful. The effect
    /// itself is never verified here.
    fn background_mouse_event(
        &self,
        _window: &WindowInfo,
        _event: MouseEvent,
        _lease: &InteractionLease,
    ) -> Result<BackgroundPointerReport, AdapterError> {
        Err(AdapterError::not_supported("background_mouse_event"))
    }

    fn key_event(
        &self,
        _combo: &KeyCombo,
        _down: bool,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("key_event"))
    }

    fn drag(&self, _params: DragParams, _lease: &InteractionLease) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("drag"))
    }

    fn clear_clipboard(&self, _lease: &InteractionLease) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("clear_clipboard"))
    }

    /// Reads the requested clipboard representation. Returns `Ok(None)`
    /// when the pasteboard has no data of the requested shape (or, for
    /// `Auto`, no data at all) — a normal, non-error outcome distinct from
    /// `Err(not_supported)`, which means this platform never implements
    /// clipboard reads.
    fn get_clipboard_content(
        &self,
        _format: ClipboardFormat,
        _deadline: Deadline,
    ) -> Result<Option<ClipboardContent>, AdapterError> {
        Err(AdapterError::not_supported("get_clipboard_content"))
    }

    fn set_clipboard_content(
        &self,
        _content: &ClipboardContent,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("set_clipboard_content"))
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
