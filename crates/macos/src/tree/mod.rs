pub mod builder;
pub mod element;
pub mod resolve;
pub mod roles;
pub mod surfaces;

pub use builder::{build_subtree, window_element_for};
pub use element::{
    copy_ax_array, copy_element_attr, copy_string_attr, element_for_pid, read_bounds,
    resolve_element_name, AXElement, ABSOLUTE_MAX_DEPTH,
};
pub use resolve::{find_element_recursive, resolve_element_impl};
pub use roles::{ax_role_to_str, is_interactive_role};
pub use surfaces::{
    alert_for_pid, focused_surface_for_pid, is_menu_open, list_surfaces_for_pid,
    menu_element_for_pid, popover_for_pid, sheet_for_pid,
};
