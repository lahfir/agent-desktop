use crate::{
    PermissionReport, PermissionState,
    action::{DragParams, KeyCombo, MouseEvent, WindowOp},
    action_request::ActionRequest,
    action_result::ActionResult,
    element_state::ElementState,
    error::{AdapterError, ErrorCode},
    node::{AccessibilityNode, AppInfo, Rect, SurfaceInfo, WindowInfo},
    notification::{NotificationFilter, NotificationIdentity, NotificationInfo},
    refs::RefEntry,
};
use std::marker::PhantomData;

pub struct WindowFilter {
    pub focused_only: bool,
    pub app: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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

impl SnapshotSurface {
    pub fn is_window(surface: &Self) -> bool {
        matches!(surface, Self::Window)
    }
}

#[derive(Clone, Copy)]
pub struct TreeOptions {
    pub max_depth: u8,
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
    pub surface: SnapshotSurface,
    pub skeleton: bool,
}

impl Default for TreeOptions {
    fn default() -> Self {
        Self {
            max_depth: 10,
            include_bounds: false,
            interactive_only: false,
            compact: false,
            surface: SnapshotSurface::Window,
            skeleton: false,
        }
    }
}

impl TreeOptions {
    pub(crate) fn with_ref_identity_bounds(mut self) -> Self {
        self.include_bounds = true;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct LiveElement {
    pub state: Option<ElementState>,
    pub bounds: Option<Rect>,
    pub available_actions: Option<Vec<String>>,
}

pub(crate) fn optional_live_read<T>(
    result: Result<Option<T>, AdapterError>,
) -> Result<Option<T>, AdapterError> {
    match result {
        Ok(value) => Ok(value),
        Err(err) if is_live_read_unsupported(&err) => Ok(None),
        Err(err) => Err(err),
    }
}

fn is_live_read_unsupported(err: &AdapterError) -> bool {
    matches!(
        err.code,
        ErrorCode::PlatformNotSupported | ErrorCode::ActionNotSupported
    )
}

pub enum ScreenshotTarget {
    Screen(usize),
    /// Capture the frontmost window owned by this process ID.
    Window(i32),
    FullScreen,
}

pub struct NativeHandle {
    pub(crate) ptr: *const std::ffi::c_void,
    _not_send_sync: PhantomData<*const ()>,
}

impl NativeHandle {
    /// # Safety
    ///
    /// `ptr` must be a valid platform accessibility handle whose ownership is
    /// transferred to the caller. The adapter that creates the handle must
    /// document how it is released through [`PlatformAdapter::release_handle`].
    pub unsafe fn from_ptr(ptr: *const std::ffi::c_void) -> Self {
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
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("execute_action"))
    }

    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Err(AdapterError::not_supported("resolve_element_strict"))
    }

    /// Resolves an element under a caller deadline. Defaults to delegating
    /// to [`PlatformAdapter::resolve_element_strict`], ignoring the timeout,
    /// so adapters that implement only the un-timed variant still support
    /// `wait --element`. Override to honor the remaining budget.
    fn resolve_element_strict_with_timeout(
        &self,
        entry: &RefEntry,
        timeout: std::time::Duration,
    ) -> Result<NativeHandle, AdapterError> {
        tracing::trace!(
            ?timeout,
            "resolve_element_strict_with_timeout: default impl does not enforce the deadline; override to honor it"
        );
        self.resolve_element_strict(entry)
    }

    /// Releases a platform-specific element handle returned from
    /// `resolve_element`. Adapter methods that receive `&NativeHandle`
    /// borrow it only; they must not consume or release it. macOS
    /// implementations must `CFRelease` here to balance the `CFRetain`
    /// that happened during resolve. Windows/Linux consumers can leave
    /// this as the default no-op.
    fn release_handle(&self, _handle: &NativeHandle) -> Result<(), AdapterError> {
        Ok(())
    }

    fn permission_report(&self) -> PermissionReport {
        PermissionReport {
            accessibility: PermissionState::Denied {
                suggestion: "Platform adapter not available".into(),
            },
            screen_recording: PermissionState::Unknown,
            automation: PermissionState::NotRequired,
        }
    }

    fn unknown_accessibility_means_unsupported(&self) -> bool {
        true
    }

    fn request_permissions(&self) -> PermissionReport {
        self.permission_report()
    }

    fn focus_window(&self, _win: &WindowInfo) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("focus_window"))
    }

    /// Brings the application owning `pid` to the foreground. Best-effort guard
    /// invoked before physical (cursor/keyboard) input that targets a known
    /// element, so synthetic events land on the intended window rather than
    /// whatever happens to be frontmost.
    fn focus_app(&self, _pid: i32) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("focus_app"))
    }

    fn launch_app(&self, _id: &str, _timeout_ms: u64) -> Result<WindowInfo, AdapterError> {
        Err(AdapterError::not_supported("launch_app"))
    }

    fn close_app(&self, _id: &str, _force: bool) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("close_app"))
    }

    /// Reports whether closing `identifier` would terminate a process the OS
    /// depends on (window server, login session, shell). The set is
    /// inherently platform-specific, so each adapter owns its own list;
    /// core only asks. The default denies nothing.
    fn is_protected_process(&self, _identifier: &str) -> bool {
        false
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

    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        Err(AdapterError::not_supported("get_live_state"))
    }

    fn get_live_actions(
        &self,
        _handle: &NativeHandle,
    ) -> Result<Option<Vec<String>>, AdapterError> {
        Err(AdapterError::not_supported("get_live_actions"))
    }

    fn get_live_element(&self, handle: &NativeHandle) -> Result<LiveElement, AdapterError> {
        let live = LiveElement {
            state: optional_live_read(self.get_live_state(handle))?,
            bounds: optional_live_read(self.get_element_bounds(handle))?,
            available_actions: optional_live_read(self.get_live_actions(handle))?,
        };
        if live.state.is_none() && live.bounds.is_none() && live.available_actions.is_none() {
            return Err(AdapterError::not_supported("get_live_element"));
        }
        Ok(live)
    }

    fn press_key_for_app(
        &self,
        _app_name: &str,
        _combo: &crate::action::KeyCombo,
    ) -> Result<crate::action_result::ActionResult, AdapterError> {
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

    fn key_event(&self, _combo: &KeyCombo, _down: bool) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("key_event"))
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

    fn dismiss_all_notifications(
        &self,
        _app_filter: Option<&str>,
    ) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError> {
        Err(AdapterError::not_supported("dismiss_all_notifications"))
    }

    /// Press a named action button on the notification at `index`.
    ///
    /// `identity` lets the caller pin the targeted notification to an
    /// expected app / title fingerprint. Notification Center reindexes
    /// entries between listings, so index-only targeting can press the
    /// wrong button if a notification arrives or is dismissed between
    /// `list_notifications` and this call. When any identity field is
    /// `Some`, implementations must return
    /// `ErrorCode::NotificationNotFound` if the row at `index` does not
    /// match. Passing an empty identity (or `None`) preserves legacy
    /// index-only behavior for callers that reconcile themselves.
    fn notification_action(
        &self,
        _index: usize,
        _identity: Option<&NotificationIdentity>,
        _action_name: &str,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("notification_action"))
    }

    fn get_subtree(
        &self,
        _handle: &NativeHandle,
        _opts: &TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        Err(AdapterError::not_supported("get_subtree"))
    }
}
