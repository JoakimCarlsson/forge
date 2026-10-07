//! Whether a button or key is down.

/// Whether a button or key went down or came back up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonState {
    /// The button or key went down.
    Pressed,
    /// The button or key came up.
    Released,
}
