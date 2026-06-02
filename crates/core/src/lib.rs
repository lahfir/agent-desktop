pub mod action;
pub mod actionability;
pub mod adapter;
pub mod commands;
pub mod error;
pub mod hints;
pub mod node;
pub mod notification;
pub mod output;
pub mod permission_report;
pub mod permission_state;
pub mod ref_alloc;
pub mod refs;
mod refs_lock;
pub mod refs_store;
#[cfg(test)]
mod refs_test_support;
pub(crate) mod resolved_element;
pub mod roles;
pub(crate) mod search_text;
pub mod snapshot;
pub mod snapshot_ref;
mod window_lookup;

pub use action::{
    Action, ActionRequest, ActionResult, Direction, DragParams, ElementState, InteractionPolicy,
    KeyCombo, Modifier, MouseButton, MouseEvent, MouseEventKind, Point, WindowOp,
};
pub use adapter::{
    ImageBuffer, ImageFormat, NativeHandle, PlatformAdapter, ScreenshotTarget, TreeOptions,
    WindowFilter,
};
pub use error::{AdapterError, AppError, ErrorCode};
pub use node::{AccessibilityNode, AppInfo, Rect, WindowInfo};
pub use notification::{NotificationFilter, NotificationInfo};
pub use output::{AppContext, ErrorPayload, Response, WindowContext};
pub use permission_report::PermissionReport;
pub use permission_state::PermissionState;
pub use refs::{RefEntry, RefMap};
pub use refs_store::RefStore;
