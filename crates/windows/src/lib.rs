use agent_desktop_core::adapter::PlatformAdapter;

pub struct WindowsAdapter;

impl WindowsAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformAdapter for WindowsAdapter {}
