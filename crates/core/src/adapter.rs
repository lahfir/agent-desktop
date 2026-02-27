use crate::{
    action::{Action, ActionResult, DragParams, MouseEvent, WindowOp},
    error::AdapterError,
    node::{AccessibilityNode, AppInfo, Rect, SurfaceInfo, WindowInfo},
    notification::{NotificationFilter, NotificationInfo},
    refs::RefEntry,
};
use std::marker::PhantomData;

pub struct WindowFilter {
    pub focused_only: bool,
    pub app: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum SnapshotSurface {
    #[default]
    Window,
    Focused,
    Menu,
    Menubar,
    Sheet,
    Popover,
    Alert,
}

pub struct TreeOptions {
    pub max_depth: u8,
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
    pub surface: SnapshotSurface,
}

impl Default for TreeOptions {
    fn default() -> Self {
        Self {
            max_depth: 10,
            include_bounds: false,
            interactive_only: false,
            compact: false,
            surface: SnapshotSurface::Window,
        }
    }
}

pub enum ScreenshotTarget {
    Screen(usize),
    /// Capture the frontmost window owned by this process ID.
    Window(i32),
    FullScreen,
}

pub enum PermissionStatus {
    Granted,
    Denied { suggestion: String },
}

pub struct NativeHandle {
    pub(crate) ptr: *const std::ffi::c_void,
    _not_send_sync: PhantomData<*const ()>,
}

impl NativeHandle {
    pub fn from_ptr(ptr: *const std::ffi::c_void) -> Self {
        Self {
            ptr,
            _not_send_sync: PhantomData,
        }
    }

    pub fn null() -> Self {
        Self {
            ptr: std::ptr::null(),
            _not_send_sync: PhantomData,
        }
    }
}

impl NativeHandle {
    /// Returns the raw platform pointer. For use by platform adapter crates only.
    /// Callers must not retain the pointer beyond the lifetime of this handle.
    pub fn as_raw(&self) -> *const std::ffi::c_void {
        self.ptr
    }
}

// SAFETY: Phase 1 is single-threaded CLI. NativeHandle is never sent across thread
// boundaries. The unsafe impls are required for use with dyn PlatformAdapter (which
// is Send + Sync). Remove in Phase 4 when async daemon is introduced.
unsafe impl Send for NativeHandle {}
unsafe impl Sync for NativeHandle {}

pub struct ImageBuffer {
    pub data: Vec<u8>,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}

pub enum ImageFormat {
    Png,
    Jpg,
}

impl ImageFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpg => "jpg",
        }
    }
}

pub trait PlatformAdapter: Send + Sync {
    fn list_windows(&self, _filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_windows"))
    }

    fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_apps"))
    }

    fn get_tree(
        &self,
        _win: &WindowInfo,
        _opts: &TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        Err(AdapterError::not_supported("get_tree"))
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _action: Action,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("execute_action"))
    }

    fn resolve_element(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Err(AdapterError::not_supported("resolve_element"))
    }

    fn check_permissions(&self) -> PermissionStatus {
        PermissionStatus::Denied {
            suggestion: "Platform adapter not available".into(),
        }
    }

    fn focus_window(&self, _win: &WindowInfo) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("focus_window"))
    }

    fn launch_app(&self, _id: &str, _timeout_ms: u64) -> Result<WindowInfo, AdapterError> {
        Err(AdapterError::not_supported("launch_app"))
    }

    fn close_app(&self, _id: &str, _force: bool) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("close_app"))
    }

    fn screenshot(&self, _target: ScreenshotTarget) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("screenshot"))
    }

    fn get_clipboard(&self) -> Result<String, AdapterError> {
        Err(AdapterError::not_supported("get_clipboard"))
    }

    fn set_clipboard(&self, _text: &str) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("set_clipboard"))
    }

    fn focused_window(&self) -> Result<Option<WindowInfo>, AdapterError> {
        Err(AdapterError::not_supported("focused_window"))
    }

    fn get_live_value(&self, _handle: &NativeHandle) -> Result<Option<String>, AdapterError> {
        Err(AdapterError::not_supported("get_live_value"))
    }

    fn press_key_for_app(
        &self,
        _app_name: &str,
        _combo: &crate::action::KeyCombo,
    ) -> Result<crate::action::ActionResult, AdapterError> {
        Err(AdapterError::not_supported("press_key_for_app"))
    }

    fn wait_for_menu(&self, _pid: i32, _open: bool, _timeout_ms: u64) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("wait_for_menu"))
    }

    fn list_surfaces(&self, _pid: i32) -> Result<Vec<SurfaceInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_surfaces"))
    }

    fn get_element_bounds(&self, _handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> {
        Err(AdapterError::not_supported("get_element_bounds"))
    }

    fn window_op(&self, _win: &WindowInfo, _op: WindowOp) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("window_op"))
    }

    fn mouse_event(&self, _event: MouseEvent) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("mouse_event"))
    }

    fn drag(&self, _params: DragParams) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("drag"))
    }

    fn clear_clipboard(&self) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("clear_clipboard"))
    }

    fn list_notifications(
        &self,
        _filter: &NotificationFilter,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_notifications"))
    }

    fn dismiss_notification(
        &self,
        _index: usize,
        _app_filter: Option<&str>,
    ) -> Result<NotificationInfo, AdapterError> {
        Err(AdapterError::not_supported("dismiss_notification"))
    }

    fn notification_action(
        &self,
        _index: usize,
        _action_name: &str,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("notification_action"))
    }
}
