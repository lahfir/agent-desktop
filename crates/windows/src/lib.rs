#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod actions;
mod adapter;
mod input;
mod system;
mod tree;

pub use adapter::WindowsAdapter;
#[cfg(target_os = "windows")]
pub use system::com_runtime::bootstrap_hosted_library;
pub use system::com_runtime::{
    ensure_hosted_library_mta_and_dpi, ensure_owned_process_mta_and_dpi,
    is_mta_established_for_new_threads,
};
#[cfg(target_os = "windows")]
pub use system::private_file::WindowsPrivateFile;
