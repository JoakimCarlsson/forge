//! The game-facing trait, frame description and input events.

use fr_render::Scene;
use fr_ui::{Div, Rgba, Theme, div};
use fr_window::{ButtonState, KeyEvent, PointerButton, ScrollDelta};

use crate::Assets;

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

/// Input the UI did not take, in logical pixels.
///
/// A press that lands on a UI region, and the scrolling over one, never reach
/// the game; releases always do, so a drag is never left half done.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    /// The pointer moved to `x`, `y` inside the window.
    PointerMoved {
        /// Horizontal position from the left edge.
        x: f32,
        /// Vertical position from the top edge.
        y: f32,
    },
    /// The pointer left the window.
    PointerLeft,
    /// A pointer button went down or came up.
    PointerButton {
        /// Which button.
        button: PointerButton,
        /// Whether it went down or came up.
        state: ButtonState,
    },
    /// A wheel or touchpad scrolled.
    Scrolled(ScrollDelta),
    /// A key went down or came up.
    Key(KeyEvent),
}

/// A game: the state and rules the engine drives each frame.
///
/// The engine asks for a UI tree every frame with [`App::view`] and hands back
/// the messages that tree's controls send with [`App::message`]. The tree
/// never mutates the game; only [`App::message`] and [`App::update`] do.
pub trait App {
    /// What the controls in the UI tree send when the user activates them.
    type Message: Clone + 'static;

    /// Called once the renderer exists, before the first frame, to load assets.
    fn init(&mut self, _assets: &mut Assets) {}

    /// Advances the game by one frame.
    fn update(&mut self, frame: &Frame);

    /// Applies one message sent by a control in the tree last built.
    fn message(&mut self, _message: Self::Message) {}

    /// Builds this frame's UI tree from the game's state, in `theme`.
    fn view(&self, _theme: &Theme) -> Div<Self::Message> {
        div()
    }

    /// Describes this frame's 3D scene in `scene`, which arrives with no lights
    /// or instances and the camera and ambient light of the previous frame.
    ///
    /// A scene with no instances draws no 3D, and the UI is drawn over it either way.
    fn scene(&self, _scene: &mut Scene) {}

    /// Whether presenting waits for the display's refresh. Asked every frame;
    /// frames are uncapped unless this returns true.
    fn vsync(&self) -> bool {
        false
    }

    /// Handles input the UI did not take.
    fn input(&mut self, _input: &Input) {}

    /// The colour the window is cleared to before the UI is drawn.
    fn clear_color(&self, theme: &Theme) -> Rgba {
        theme.colors.background
    }
}
