//! A fixed timestep accumulator for simulation that runs at its own rate.

/// Turns variable frame times into a whole number of fixed steps per frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedStepper {
    /// The length of one step in seconds.
    step: f32,
    /// The most steps one frame may run before the backlog is dropped.
    max_steps: u32,
    /// The time handed in that has not been consumed by a step.
    accumulator: f32,
}

impl FixedStepper {
    /// A stepper of `step` seconds that runs at most `max_steps` steps per frame.
    #[must_use]
    pub fn new(step: f32, max_steps: u32) -> Self {
        Self {
            step,
            max_steps,
            accumulator: 0.0,
        }
    }

    /// The length of one step in seconds.
    #[must_use]
    pub fn step(&self) -> f32 {
        self.step
    }

    /// Changes the length of a step, keeping the unconsumed time.
    pub fn set_step(&mut self, step: f32) {
        self.step = step;
    }

    /// Adds `delta` seconds of frame time and returns how many steps are due. Time beyond
    /// the step limit is dropped so a slow frame cannot cause a spiral of ever more steps.
    pub fn advance(&mut self, delta: f32) -> u32 {
        if self.step <= 0.0 {
            self.accumulator = 0.0;
            return 0;
        }
        self.accumulator += delta.max(0.0);
        let due = (self.accumulator / self.step).floor();
        let steps = (due as u32).min(self.max_steps);
        self.accumulator -= steps as f32 * self.step;
        if due as u32 > self.max_steps {
            self.accumulator = self.accumulator.min(self.step);
        }
        steps
    }

    /// How far the unconsumed time has progressed into the next step, in zero to one; the
    /// blend factor for drawing between the last two states.
    #[must_use]
    pub fn alpha(&self) -> f32 {
        if self.step <= 0.0 {
            return 0.0;
        }
        (self.accumulator / self.step).clamp(0.0, 1.0)
    }
}
