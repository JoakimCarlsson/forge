//! The handler trait the event loop drives.

use crate::Window;

/// Receives the window's lifecycle and drives a frame per redraw.
pub trait WindowHandler {
    /// Called once the window exists, before the first frame.
    fn created(&mut self, window: &Window);

    /// Called when the drawable area changes, in physical pixels.
    fn resized(&mut self, width: u32, height: u32);

    /// Called for each frame; the window is redrawn again afterwards.
    fn redraw(&mut self);

    /// Called when the user asks to close the window; the loop exits afterwards.
    fn close_requested(&mut self) {}
}
