use serde::{Deserialize, Serialize};

use crate::ProcessId;

/// How an application presents itself to the user, so an agent can tell a
/// window-owning app from one that only appears on a hotkey or lives in the
/// menu bar or tray.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppPresentation {
    /// Owns ordinary windows and appears in the Dock or taskbar.
    Foreground,
    /// No Dock or taskbar entry. Menu bar and tray items live here, as do
    /// overlays summoned by a hotkey; their windows may exist only while shown.
    Background,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub pid: ProcessId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_instance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<AppPresentation>,
}
