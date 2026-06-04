pub mod action;
pub mod action_request;
pub mod action_result;
pub mod action_step;
pub mod action_step_outcome;
pub mod actionability;
pub mod adapter;
pub mod capability;
pub mod commands;
pub mod context;
pub mod element_state;
pub mod error;
pub mod hints;
pub mod node;
pub mod notification;
pub mod output;
pub mod permission_report;
pub mod permission_state;
pub mod ref_action;
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
pub mod trace;
mod window_lookup;

pub use action::{
    Action, Direction, DragParams, KeyCombo, Modifier, MouseButton, MouseEvent, MouseEventKind,
    Point, WindowOp,
};
pub use action_request::{ActionRequest, InteractionPolicy};
pub use action_result::ActionResult;
pub use action_step::ActionStep;
pub use action_step_outcome::ActionStepOutcome;
pub use adapter::{
    ImageBuffer, ImageFormat, NativeHandle, PlatformAdapter, ScreenshotTarget, TreeOptions,
    WindowFilter,
};
pub use context::CommandContext;
pub use element_state::ElementState;
pub use error::{AdapterError, AppError, ErrorCode};
pub use node::{AccessibilityNode, AppInfo, Rect, WindowInfo};
pub use notification::{NotificationFilter, NotificationInfo};
pub use output::{AppContext, ErrorPayload, Response, WindowContext};
pub use permission_report::PermissionReport;
pub use permission_state::PermissionState;
pub use refs::{RefEntry, RefMap};
pub use refs_store::RefStore;
