use serde::{Deserialize, Serialize};

use crate::ProcessId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefProcess {
    pub pid: ProcessId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_instance: Option<String>,
}

impl RefProcess {
    /// Returns the saved process identity when its generation is nonempty.
    pub fn identity(&self) -> Option<crate::ProcessIdentity> {
        self.process_instance
            .as_deref()
            .filter(|instance| !instance.is_empty())
            .map(|instance| crate::ProcessIdentity::new(self.pid, instance))
    }
}
