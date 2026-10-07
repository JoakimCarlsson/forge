//! The crate a game depends on: the [`App`] trait and the [`run`] entry point.
//!
//! The UI vocabulary a game builds its tree from is re-exported as [`ui`]. No
//! winit or wgpu type appears in this crate's public API.

mod app;
mod host;

pub use app::{App, Frame};
pub use fr_ui as ui;
pub use host::{EngineError, run};
