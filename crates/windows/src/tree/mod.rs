pub mod automation;
pub mod cache;
pub mod element;
pub mod properties;
pub mod property_ids;

#[cfg(all(test, target_os = "windows"))]
mod fixture;
#[cfg(all(test, target_os = "windows"))]
mod fixture_window;
