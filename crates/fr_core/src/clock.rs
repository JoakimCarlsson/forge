//! Per-frame timing.

use std::time::{Duration, Instant};

/// Measures the time between frames and counts them.
#[derive(Debug, Clone)]
pub struct FrameClock {
    /// The instant of the most recent [`FrameClock::tick`].
    last: Instant,
    /// The time between the two most recent ticks.
    delta: Duration,
    /// The number of ticks so far.
    frame: u64,
}

impl FrameClock {
    /// Creates a clock whose first frame starts now.
    #[must_use]
    pub fn new() -> Self {
        Self {
            last: Instant::now(),
            delta: Duration::ZERO,
            frame: 0,
        }
    }

    /// Starts a new frame and returns the time since the previous one.
    pub fn tick(&mut self) -> Duration {
        let now = Instant::now();
        self.delta = now - self.last;
        self.last = now;
        self.frame += 1;
        self.delta
    }

    /// The time between the two most recent ticks.
    #[must_use]
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// The time between the two most recent ticks, in seconds.
    #[must_use]
    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    /// The number of frames started so far.
    #[must_use]
    pub fn frame(&self) -> u64 {
        self.frame
    }
}

impl Default for FrameClock {
    /// Creates a clock whose first frame starts now.
    fn default() -> Self {
        Self::new()
    }
}
