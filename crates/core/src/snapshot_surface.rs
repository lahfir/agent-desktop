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
    Desktop,
    Taskbar,
    SystemTray,
    QuickSettings,
    NotificationCenter,
    Toolbar,
    Dock,
    Spotlight,
    MenuBarExtras,
    SystemTrayOverflow,
    StartMenu,
    ActionCenter,
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
            Self::Desktop => "desktop",
            Self::Taskbar => "taskbar",
            Self::SystemTray => "system_tray",
            Self::QuickSettings => "quick_settings",
            Self::NotificationCenter => "notification_center",
            Self::Toolbar => "toolbar",
            Self::Dock => "dock",
            Self::Spotlight => "spotlight",
            Self::MenuBarExtras => "menu_bar_extras",
            Self::SystemTrayOverflow => "system_tray_overflow",
            Self::StartMenu => "start_menu",
            Self::ActionCenter => "action_center",
        }
    }
}
