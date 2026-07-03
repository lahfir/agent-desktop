use agent_desktop_core::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};

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

impl ObservationOps for LinuxAdapter {}
impl ActionOps for LinuxAdapter {}
impl InputOps for LinuxAdapter {}
impl SystemOps for LinuxAdapter {}
