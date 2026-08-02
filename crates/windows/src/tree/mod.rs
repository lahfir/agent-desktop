pub mod actions;
pub mod automation;
pub(crate) mod cache;
pub mod descriptor;
pub mod name_evidence;
pub mod roles;
pub mod states;

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

#[cfg(all(test, target_os = "windows"))]
mod fixture;
#[cfg(all(test, target_os = "windows"))]
mod fixture_window;
