pub mod action;
pub mod adapter;
pub mod commands;
pub mod error;
pub mod node;
pub mod output;
pub mod refs;
pub mod snapshot;

pub use action::{Action, ActionResult, Direction, ElementState, KeyCombo, Modifier};
pub use adapter::{
    ImageBuffer, ImageFormat, NativeHandle, PermissionStatus, PlatformAdapter, ScreenshotTarget,
    TreeOptions, WindowFilter,
};
pub use error::{AdapterError, AppError, ErrorCode};
pub use node::{AccessibilityNode, AppInfo, Rect, WindowInfo};
pub use output::{AppContext, ErrorPayload, Response, WindowContext};
pub use refs::{RefEntry, RefMap};
