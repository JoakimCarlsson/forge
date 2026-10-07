//! What a game sees of the step or frame being run.

/// What a game sees of the frame being run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameInfo {
    /// The seconds since the previous frame.
    pub delta_seconds: f32,
    /// The number of frames started so far, counting this one.
    pub index: u64,
    /// The window's drawable width in physical pixels.
    pub width: u32,
    /// The window's drawable height in physical pixels.
    pub height: u32,
    /// How far this frame is into the next fixed step, in zero to one: the blend
    /// factor the bodies are drawn between their last two steps with.
    pub interpolation: f32,
}

/// One fixed step of the simulation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedFrameInfo {
    /// The number of fixed steps run before this one.
    pub index: u64,
    /// The length of the step in seconds, which is `App::fixed_timestep`.
    pub delta_seconds: f32,
}
