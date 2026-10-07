//! The game-facing trait and frame description.

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
pub trait App {
    /// Advances the game by one frame.
    fn update(&mut self, frame: &Frame);

    /// The colour the window is cleared to, as linear red, green, blue and alpha.
    fn clear_color(&self) -> [f64; 4] {
        [0.05, 0.05, 0.08, 1.0]
    }
}
