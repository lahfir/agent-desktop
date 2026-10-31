/// A key on the compositor's virtual keyboard: its evdev code and whether the
/// keysym sits on the Shift level of that key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EvdevKey {
    pub(crate) code: u32,
    pub(crate) shift: bool,
}
