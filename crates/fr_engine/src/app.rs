//! The game-facing trait and frame description.

use fr_ui::{Div, Rgba, Theme, div};

/// What a game sees of the frame being run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// The seconds since the previous frame.
    pub delta_seconds: f32,
    /// The number of frames started so far, counting this one.
    pub index: u64,
    /// The window's drawable width in physical pixels.
    pub width: u32,
    /// The window's drawable height in physical pixels.
    pub height: u32,
}

/// A game: the state and rules the engine drives each frame.
///
/// The engine asks for a UI tree every frame with [`App::view`] and hands back
/// the messages that tree's controls send with [`App::message`]. The tree
/// never mutates the game; only [`App::message`] and [`App::update`] do.
pub trait App {
    /// What the controls in the UI tree send when the user activates them.
    type Message: Clone + 'static;

    /// Advances the game by one frame.
    fn update(&mut self, frame: &Frame);

    /// Applies one message sent by a control in the tree last built.
    fn message(&mut self, _message: Self::Message) {}

    /// Builds this frame's UI tree from the game's state, in `theme`.
    fn view(&self, _theme: &Theme) -> Div<Self::Message> {
        div()
    }

    /// The colour the window is cleared to before the UI is drawn.
    fn clear_color(&self, theme: &Theme) -> Rgba {
        theme.colors.background
    }
}
