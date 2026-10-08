//! Frame timing resources and window-independent input events.

use fr_app::ecs as bevy_ecs;
use fr_input::{ButtonState, KeyEvent, PointerButton, ScrollDelta};

use fr_app::ecs::resource::Resource;

/// What a game sees of the frame being run.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// The seconds since the previous frame.
    pub delta_seconds: f32,
    /// The number of frames started so far, counting this one.
    pub index: u64,
    /// The window's drawable width in physical pixels.
    pub width: u32,
    /// The window's drawable height in physical pixels.
    pub height: u32,
    /// The physical pixels per logical pixel, which pointer positions are measured in.
    pub scale_factor: f32,
    /// How far this frame is into the next fixed step, in zero to one: the blend factor for
    /// drawing between the last two states of a fixed update.
    pub interpolation: f32,
}

/// One fixed step of the simulation, as handed to the fixed schedules.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct FixedStep {
    /// The number of fixed steps run before this one.
    pub index: u64,
    /// The length of the step in seconds, which is [`crate::WindowSettings::fixed_timestep`].
    pub delta_seconds: f32,
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
    /// The pointer moved by `dx`, `dy` in device units, also while the cursor is captured.
    PointerMotion {
        /// Horizontal motion, positive to the right.
        dx: f32,
        /// Vertical motion, positive downwards.
        dy: f32,
    },
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
