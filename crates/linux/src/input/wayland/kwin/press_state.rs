/// What a keyboard delivery holds down between keysym events: how many Shift
/// and how many Control, Alt or Super keysyms the caller pressed, and the keys
/// this backend wrapped in its own Shift press.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PressState {
    pub(crate) explicit_shift: u32,
    pub(crate) chord_modifiers: u32,
    pub(crate) wrapped: Vec<u32>,
}
