/// What a keyboard delivery holds down between keysym events: how many Shift
/// keysyms the caller pressed, and the keys this backend wrapped in its own
/// Shift press.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PressState {
    pub(crate) explicit_shift: u32,
    pub(crate) wrapped: Vec<u32>,
}
