//! The winit window and event loop of the forge engine.
//!
//! [`run`] owns the event loop and drives a [`WindowHandler`]; the handler
//! never sees a winit type: pointer, scroll, key and scale-factor events arrive
//! as the crate-owned types of [`PointerButton`], [`ScrollDelta`], [`KeyEvent`]
//! and plain numbers.

mod config;
mod handler;
mod host;
mod input;

pub use config::WindowConfig;
pub use handler::WindowHandler;
pub use host::{Window, WindowError, run};
pub use input::{ButtonState, Key, KeyEvent, Modifiers, PointerButton, ScrollDelta};
