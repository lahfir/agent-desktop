pub mod automation;
pub mod element;

#[cfg(all(test, target_os = "windows"))]
mod fixture;
#[cfg(all(test, target_os = "windows"))]
mod fixture_window;
