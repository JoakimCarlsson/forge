//! Frame timing and shared value types of the forge engine.
//!
//! This crate has no window or graphics dependency, so everything in it is
//! testable headlessly.

mod clock;

pub use clock::FrameClock;
