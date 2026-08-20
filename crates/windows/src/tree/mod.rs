pub mod actions;
pub mod automation;
pub(crate) mod cache;
pub mod chromium;
pub mod descriptor;
pub(crate) mod hit_test;
pub(crate) mod live_read;
pub mod name_evidence;
pub(crate) mod observe;
pub(crate) mod resolve;
pub(crate) mod resolve_anchor;
pub(crate) mod resolve_match;
pub(crate) mod resolve_search;
pub mod roles;
pub mod states;
pub mod surfaces;
pub mod wrapper;

#[cfg(test)]
mod captures;
pub mod element;
pub mod element_properties;
pub mod properties;
pub mod property_ids;
pub mod property_outcome;
pub mod walker;
pub(crate) mod walker_enumerate;
pub mod walker_source;

#[cfg(test)]
mod walker_fake;

#[cfg(test)]
#[path = "hit_test_scan_tests.rs"]
mod hit_test_scan_tests;

#[cfg(all(test, target_os = "windows"))]
pub(crate) mod fixture;
#[cfg(all(test, target_os = "windows"))]
pub(crate) mod fixture_clipboard;
#[cfg(all(test, target_os = "windows"))]
pub(crate) mod fixture_menu;
#[cfg(all(test, target_os = "windows"))]
pub(crate) mod fixture_modal;
#[cfg(all(test, target_os = "windows"))]
pub(crate) mod fixture_overlay;
#[cfg(all(test, target_os = "windows"))]
pub(crate) mod fixture_pattern;
#[cfg(all(test, target_os = "windows"))]
pub(crate) mod fixture_window;
