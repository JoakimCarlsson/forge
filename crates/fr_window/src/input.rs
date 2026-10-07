//! Input events as the window reports them, in types the crate owns.

use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};

/// How many logical pixels one line of a wheel scroll stands for.
const LINE_PIXELS: f32 = 40.0;

/// A pointer button.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PointerButton {
    /// The main button, normally the left one.
    Primary,
    /// The context button, normally the right one.
    Secondary,
    /// The wheel button.
    Middle,
    /// Any other button, by the number the platform gives it.
    Other(u16),
}

impl PointerButton {
    /// The button winit reports as `button`.
    pub(crate) fn from_winit(button: MouseButton) -> Self {
        match button {
            MouseButton::Left => Self::Primary,
            MouseButton::Right => Self::Secondary,
            MouseButton::Middle => Self::Middle,
            MouseButton::Back => Self::Other(8),
            MouseButton::Forward => Self::Other(9),
            MouseButton::Other(number) => Self::Other(number),
        }
    }
}

/// Whether a button or key went down or came back up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonState {
    /// The button or key went down.
    Pressed,
    /// The button or key came up.
    Released,
}

impl ButtonState {
    /// The state winit reports as `state`.
    pub(crate) fn from_winit(state: ElementState) -> Self {
        match state {
            ElementState::Pressed => Self::Pressed,
            ElementState::Released => Self::Released,
        }
    }
}

/// How far a wheel or touchpad scrolled, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollDelta {
    /// Distance along the x axis; positive moves the content right.
    pub x: f32,
    /// Distance along the y axis; positive moves the content down.
    pub y: f32,
}

impl ScrollDelta {
    /// The delta winit reports as `delta` on a window at `scale_factor`.
    pub(crate) fn from_winit(delta: MouseScrollDelta, scale_factor: f64) -> Self {
        match delta {
            MouseScrollDelta::LineDelta(x, y) => Self {
                x: x * LINE_PIXELS,
                y: y * LINE_PIXELS,
            },
            MouseScrollDelta::PixelDelta(position) => Self {
                x: (position.x / scale_factor) as f32,
                y: (position.y / scale_factor) as f32,
            },
        }
    }
}

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

impl Modifiers {
    /// The modifiers winit reports as `state`.
    pub(crate) fn from_winit(state: ModifiersState) -> Self {
        Self {
            shift: state.shift_key(),
            control: state.control_key(),
            alt: state.alt_key(),
            logo: state.super_key(),
        }
    }
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

impl Key {
    /// The key winit reports as `key`.
    pub(crate) fn from_winit(key: &WinitKey) -> Self {
        match key {
            WinitKey::Character(text) => Self::Character(text.to_string()),
            WinitKey::Named(NamedKey::Enter) => Self::Enter,
            WinitKey::Named(NamedKey::Tab) => Self::Tab,
            WinitKey::Named(NamedKey::Space) => Self::Space,
            WinitKey::Named(NamedKey::Escape) => Self::Escape,
            WinitKey::Named(NamedKey::Backspace) => Self::Backspace,
            WinitKey::Named(NamedKey::Delete) => Self::Delete,
            WinitKey::Named(NamedKey::ArrowUp) => Self::ArrowUp,
            WinitKey::Named(NamedKey::ArrowDown) => Self::ArrowDown,
            WinitKey::Named(NamedKey::ArrowLeft) => Self::ArrowLeft,
            WinitKey::Named(NamedKey::ArrowRight) => Self::ArrowRight,
            WinitKey::Named(NamedKey::Home) => Self::Home,
            WinitKey::Named(NamedKey::End) => Self::End,
            _ => Self::Other,
        }
    }
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
