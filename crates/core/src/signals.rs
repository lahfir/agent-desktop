use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignalBaseline {
    pub focused_app: Option<String>,
    pub focused_window_title: Option<String>,
    pub window_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DesktopSignal {
    AppActivated { app: String },
    WindowFocused { window_id: String, title: String },
    WindowClosed { window_id: String },
}
