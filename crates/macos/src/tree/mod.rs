pub mod action_list;
pub(crate) mod attributes;
pub mod ax_element;
pub(crate) mod ax_value;
pub mod build_context;
pub mod builder;
pub mod capabilities;
pub mod element;
pub mod element_bounds;
pub(crate) mod element_dedupe;
pub(crate) mod node_attrs;
pub mod resolve;
mod resolve_bounds;
mod resolve_classify;
mod resolve_deadline;
mod resolve_identity;
mod resolve_roots;
mod resolve_search;
pub mod roles;
pub mod surfaces;

pub(crate) use attributes::{
    copy_ax_array, copy_bool_attr, copy_element_attr, copy_i64_attr, copy_string_attr,
    copy_value_typed,
};
pub use ax_element::AXElement;
pub use build_context::TreeBuildContext;
pub use builder::{build_subtree, window_element_for};
pub use capabilities::same_element;
pub use element::{element_for_pid, resolve_element_name};
pub use element_bounds::read_bounds;
pub(crate) use node_attrs::NodeAttrs;
pub use surfaces::{
    alert_for_pid, focused_surface_for_pid, menu_element_for_pid, menubar_for_pid, popover_for_pid,
    sheet_for_pid,
};
