//! Frame timing and the fixed timestep accumulator of the forge engine.
//!
//! [`FrameClock`] measures frames; [`FixedStepper`] turns their lengths into
//! whole steps of a simulation that runs at its own rate. Nothing here knows a
//! window or a device.
//!
//! The crate root is the facade and nothing else.

mod clock;
mod fixed_step;

pub use clock::FrameClock;
pub use fixed_step::FixedStepper;
