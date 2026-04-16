#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdSnapshotSurface {
    Window = 0,
    Focused = 1,
    Menu = 2,
    Menubar = 3,
    Sheet = 4,
    Popover = 5,
    Alert = 6,
}
