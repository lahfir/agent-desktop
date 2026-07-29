mod adapter;
pub(crate) mod com_runtime;
pub(crate) mod dpi;
pub(crate) mod hresult;
pub(crate) mod permissions;
#[cfg(target_os = "windows")]
pub(crate) mod private_file;
pub(crate) mod session;
