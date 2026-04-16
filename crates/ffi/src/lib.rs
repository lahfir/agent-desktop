//! # agent-desktop FFI
//!
//! C-ABI surface over `PlatformAdapter`. Exposes
//! `libagent_desktop_ffi.{dylib,so,dll}` to Python / Swift / Go / Node /
//! C++ consumers.
//!
//! ## ⚠ Thread safety (macOS)
//!
//! **Every FFI entry other than `ad_adapter_create`, `ad_adapter_destroy`,
//! `ad_last_error_*`, and the `ad_free_*` family must be invoked on the
//! process's main thread.** macOS accessibility and Cocoa APIs require
//! this and will misbehave silently on worker threads. Debug builds
//! assert this constraint; release builds do not (no-op `debug_assert!`)
//! but violators invoke undefined behavior.
//!
//! ## Build profile
//!
//! The cdylib must be built with the workspace's `release-ffi` profile:
//!
//! ```text
//! cargo build --profile release-ffi -p agent-desktop-ffi
//! ```
//!
//! The workspace `release` profile keeps `panic = "abort"` to hold the
//! CLI under its size budget; the cdylib needs `panic = "unwind"` so the
//! `trap_panic` boundary actually catches. Both profiles coexist.
//!
//! ## Error model
//!
//! Every `AdResult`-returning fn sets thread-local last-error details on
//! failure. The pointer returned by `ad_last_error_message()` survives
//! any number of subsequent successful calls on the same thread; only
//! the next *failing* call rotates it. Matches POSIX `errno` semantics.

pub(crate) mod actions;
pub(crate) mod adapter;
pub(crate) mod apps;
pub(crate) mod convert;
pub(crate) mod enum_validation;
pub mod error;
pub(crate) mod ffi_try;
pub(crate) mod input;
pub(crate) mod main_thread;
pub(crate) mod screenshot;
pub(crate) mod surfaces;
pub(crate) mod tree;
pub mod types;
pub(crate) mod windows;

pub use adapter::AdAdapter;
pub use error::AdResult;
pub use types::action::AdAction;
pub use types::action_kind::AdActionKind;
pub use types::action_result::AdActionResult;
pub use types::app_info::AdAppInfo;
pub use types::direction::AdDirection;
pub use types::drag_params::AdDragParams;
pub use types::element_state::AdElementState;
pub use types::image_buffer::AdImageBuffer;
pub use types::image_format::AdImageFormat;
pub use types::key_combo::AdKeyCombo;
pub use types::modifier::AdModifier;
pub use types::mouse_button::AdMouseButton;
pub use types::mouse_event::AdMouseEvent;
pub use types::mouse_event_kind::AdMouseEventKind;
pub use types::native_handle::AdNativeHandle;
pub use types::node::AdNode;
pub use types::node_tree::AdNodeTree;
pub use types::point::AdPoint;
pub use types::rect::AdRect;
pub use types::ref_entry::AdRefEntry;
pub use types::screenshot_kind::AdScreenshotKind;
pub use types::screenshot_target::AdScreenshotTarget;
pub use types::scroll_params::AdScrollParams;
pub use types::snapshot_surface::AdSnapshotSurface;
pub use types::surface_info::AdSurfaceInfo;
pub use types::tree_options::AdTreeOptions;
pub use types::window_info::AdWindowInfo;
pub use types::window_op::AdWindowOp;
pub use types::window_op_kind::AdWindowOpKind;
