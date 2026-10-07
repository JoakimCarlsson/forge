//! The winit window and event loop of the forge engine.
//!
//! [`run`] owns the event loop and drives a [`WindowHandler`]; the handler
//! never sees a winit type: pointer, scroll, key and scale-factor events arrive
//! as the `fr_input` types and plain numbers.

mod config;
mod convert;
mod handler;
mod host;

pub use config::WindowConfig;
pub use handler::WindowHandler;
pub use host::{Window, WindowError, run};
