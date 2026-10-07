//! How a text field of a panel stopped being typed into.

/// What ended the typing in a field, which decides what the panel does with
/// the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEnd {
    /// Enter was pressed.
    Enter,
    /// A press landed outside every field, so the keyboard let go.
    Blur,
    /// Escape was pressed.
    Escape,
}

impl TextEnd {
    /// Whether the text typed stands: it does for Enter and a blur, and is
    /// dropped by Escape.
    pub const fn commits(self) -> bool {
        !matches!(self, Self::Escape)
    }
}
