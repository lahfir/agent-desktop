use agent_desktop_core::error::AdapterError;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::tree::surfaces::is_menu_open;

    pub fn wait_for_menu(pid: i32, open: bool, timeout_ms: u64) -> Result<(), AdapterError> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            if is_menu_open(pid) == open {
                return Ok(());
            }
            if Instant::now() >= deadline {
                let msg = if open {
                    format!("No context menu opened within {timeout_ms}ms")
                } else {
                    format!("Context menu did not close within {timeout_ms}ms")
                };
                return Err(AdapterError::timeout(msg));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn wait_for_menu(_pid: i32, _open: bool, _timeout_ms: u64) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("wait_for_menu"))
    }
}

pub use imp::wait_for_menu;
