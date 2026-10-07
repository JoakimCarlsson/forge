//! The crate a game depends on: the [`App`] trait and the [`run`] entry point.
//!
//! No winit or wgpu type appears in this crate's public API.

mod app;
mod host;

pub use app::{App, Frame};
pub use host::{EngineError, run};
