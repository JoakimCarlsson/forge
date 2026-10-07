//! Pointer buttons and scrolling.

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

/// What the pointer does while a window has it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CursorMode {
    /// The pointer is visible and moves freely.
    #[default]
    Normal,
    /// The pointer is hidden and kept in the window, and motion is reported as relative deltas.
    Captured,
}

/// How far a wheel or touchpad scrolled, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollDelta {
    /// Distance along the x axis; positive moves the content right.
    pub x: f32,
    /// Distance along the y axis; positive moves the content down.
    pub y: f32,
}
