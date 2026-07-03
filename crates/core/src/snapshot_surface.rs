#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSurface {
    #[default]
    Window,
    Focused,
    Menu,
    Menubar,
    Sheet,
    Popover,
    Alert,
}

impl SnapshotSurface {
    pub fn is_window(surface: &Self) -> bool {
        matches!(surface, Self::Window)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Focused => "focused",
            Self::Menu => "menu",
            Self::Menubar => "menubar",
            Self::Sheet => "sheet",
            Self::Popover => "popover",
            Self::Alert => "alert",
        }
    }
}
