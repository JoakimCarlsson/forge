//! The winit window and event loop of the forge engine.
//!
//! [`run`] owns the event loop and drives a [`WindowHandler`]; the handler
//! never sees a winit type except through the [`Window`] it is lent.

mod config;
mod handler;
mod host;

pub use config::WindowConfig;
pub use handler::WindowHandler;
pub use host::{Window, WindowError, run};
