mod adapter;
pub(crate) mod com_runtime;
pub(crate) mod dpi;
pub(crate) mod hresult;
pub(crate) mod permissions;
#[cfg(target_os = "windows")]
pub(crate) mod private_file;
pub(crate) mod process_identity;
pub(crate) mod session;
pub(crate) mod window_enum;
pub(crate) mod window_identity;
pub(crate) mod window_ops;
