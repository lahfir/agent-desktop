pub mod action;
pub mod action_request;
pub mod action_result;
pub mod action_step;
pub mod action_step_outcome;
pub(crate) mod actionability;
pub mod adapter;
pub mod adapter_session;
pub mod capability;
pub mod clipboard_content;
pub mod commands;
pub mod context;
pub mod display_info;
pub mod element_state;
pub mod error;
pub mod hints;
pub mod hit_test;
pub mod image_buffer;
pub mod interaction_policy;
pub mod launch_options;
pub mod live_element;
pub mod locator;
pub mod native_handle;
pub mod node;
pub mod notification;
pub mod output;
pub mod permission_report;
pub mod permission_state;
pub mod process_state;
pub mod ref_action;
pub mod ref_action_wait;
pub mod ref_alloc;
pub mod ref_identity;
pub mod refs;
mod refs_lock;
pub mod refs_store;
#[cfg(test)]
mod refs_test_support;
pub(crate) mod resolved_element;
pub mod roles;
pub mod screenshot_target;
pub(crate) mod search_text;
pub mod session;
pub mod session_affinity;
pub mod signals;
pub mod snapshot;
pub mod snapshot_ref;
pub mod snapshot_surface;
pub mod state;
pub mod step_mechanism;
pub(crate) mod trace;
pub(crate) mod trace_artifacts;
pub mod trace_read;
pub mod trace_sanitize;
pub mod tree_options;
pub mod window_filter;
mod window_lookup;

pub use action::{
    Action, Direction, DragParams, KeyCombo, Modifier, MouseButton, MouseEvent, MouseEventKind,
    Point, WindowOp,
};
pub use action_request::ActionRequest;
pub use action_result::ActionResult;
pub use action_step::ActionStep;
pub use action_step_outcome::ActionStepOutcome;
pub use adapter::{
    ImageBuffer, ImageFormat, NativeHandle, PlatformAdapter, ScreenshotTarget, TreeOptions,
    WindowFilter,
};
pub use context::{CommandContext, WaitSelector};
pub use display_info::DisplayInfo;
pub use element_state::ElementState;
pub use error::{AdapterError, AppError, ErrorCode};
pub use hit_test::HitTestResult;
pub use image_buffer::parse_png_dimensions;
pub use interaction_policy::InteractionPolicy;
pub use node::{AccessibilityNode, AppInfo, Rect, WindowInfo};
pub use notification::{NotificationFilter, NotificationInfo};
pub use output::{ErrorPayload, Response};
pub use permission_report::PermissionReport;
pub use permission_state::PermissionState;
pub use refs::{RefEntry, RefMap};
pub use refs_store::RefStore;
pub use step_mechanism::StepMechanism;
pub use trace_sanitize::sanitize_trace_value;
