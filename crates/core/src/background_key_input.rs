use crate::KeyCombo;

/// Keyboard input for one background delivery to an exact window.
#[derive(Debug, Clone)]
pub enum BackgroundKeyInput {
    /// One key press: key down then key up, with the modifiers carried as
    /// event flags on both events.
    Combo(KeyCombo),
    /// Literal text. Every character becomes its own key down/up pair that
    /// carries the character as a Unicode string, so spaces, punctuation, and
    /// non-ASCII text do not depend on the keyboard layout.
    Text(String),
}
