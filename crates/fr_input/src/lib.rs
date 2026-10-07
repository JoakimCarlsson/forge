//! Pointer, scroll and keyboard value types of the forge engine.
//!
//! These are the types a window reports input in and the UI and games read
//! it as. They are plain data with no platform dependency; the crate that owns
//! the platform window converts into them.
//!
//! The crate root is the facade and nothing else.

mod keyboard;
mod pointer;
mod state;

pub use keyboard::{Key, KeyEvent, Modifiers};
pub use pointer::{CursorMode, PointerButton, ScrollDelta};
pub use state::ButtonState;
