use agent_desktop_core::{error::AdapterError, node::AppInfo};

pub fn list_apps_impl() -> Result<Vec<AppInfo>, AdapterError> {
    #[cfg(target_os = "macos")]
    {
        Ok(crate::system::app_inventory::list_apps())
    }
    #[cfg(not(target_os = "macos"))]
    Err(AdapterError::not_supported("list_apps"))
}

#[cfg(target_os = "macos")]
pub(crate) fn pid_for_app_name(app_name: &str) -> Option<i32> {
    crate::system::app_inventory::pid_for_app_name(app_name)
}

#[cfg(target_os = "macos")]
pub(crate) fn pids_for_app_name(app_name: &str) -> Vec<i32> {
    crate::system::app_inventory::pids_for_app_name(app_name)
}
