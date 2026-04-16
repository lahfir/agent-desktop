#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdModifier {
    Cmd = 0,
    Ctrl = 1,
    Alt = 2,
    Shift = 3,
}
