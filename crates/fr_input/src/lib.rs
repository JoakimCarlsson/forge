//! Pointer, scroll and keyboard value types of the forge engine.
//!
//! These are the types a window reports input in and the UI and games read
//! it as. They are plain data with no platform dependency; the crate that owns
//! the platform window converts into them.
//!
//! [`InputState`] gathers a frame's events into what a game reads: the pointer's
//! position and movement, held buttons, scroll and key events.
//!
//! The crate root is the facade and nothing else.

mod keyboard;
mod pointer;
mod state;
mod tracker;

pub use keyboard::{Key, KeyEvent, Modifiers};
pub use pointer::{PointerButton, ScrollDelta};
pub use state::ButtonState;
pub use tracker::InputState;
