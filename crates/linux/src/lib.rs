use agent_desktop_core::adapter::PlatformAdapter;

pub struct LinuxAdapter;

impl LinuxAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformAdapter for LinuxAdapter {}
