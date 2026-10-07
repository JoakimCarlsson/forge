//! Keys, modifiers and key events.

use crate::state::ButtonState;

/// The modifier keys held while an event happened.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    /// Either shift key.
    pub shift: bool,
    /// Either control key.
    pub control: bool,
    /// Either alt key.
    pub alt: bool,
    /// The logo, command or windows key.
    pub logo: bool,
}

/// A key, named for what it does rather than for where it sits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Key {
    /// A key that produces text, with the text it produces.
    Character(String),
    /// The enter or return key.
    Enter,
    /// The tab key.
    Tab,
    /// The space bar.
    Space,
    /// The escape key.
    Escape,
    /// The backspace key.
    Backspace,
    /// The delete key.
    Delete,
    /// The up arrow.
    ArrowUp,
    /// The down arrow.
    ArrowDown,
    /// The left arrow.
    ArrowLeft,
    /// The right arrow.
    ArrowRight,
    /// The home key.
    Home,
    /// The end key.
    End,
    /// Any other key.
    Other,
}

/// A key going down or coming up.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyEvent {
    /// Which key.
    pub key: Key,
    /// Whether it went down or came up.
    pub state: ButtonState,
    /// Whether this press is the keyboard repeating a held key.
    pub repeat: bool,
    /// The modifiers held at the time.
    pub modifiers: Modifiers,
}
