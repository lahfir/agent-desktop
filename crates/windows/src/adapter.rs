use agent_desktop_core::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};

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

impl ObservationOps for WindowsAdapter {}
impl ActionOps for WindowsAdapter {}
impl InputOps for WindowsAdapter {}
impl SystemOps for WindowsAdapter {}
