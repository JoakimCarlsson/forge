//! The handler trait the event loop drives.

use fr_input::{ButtonState, KeyEvent, PointerButton, ScrollDelta};

use crate::Window;

/// Receives the window's lifecycle and input, and drives a frame per redraw.
///
/// Pointer positions and scroll distances are in logical pixels; the drawable
/// size in [`WindowHandler::resized`] is in physical pixels.
pub trait WindowHandler {
    /// Called once the window exists, before the first frame.
    fn created(&mut self, window: &Window);

    /// Called when the drawable area changes, in physical pixels.
    fn resized(&mut self, width: u32, height: u32);

    /// Called when the physical pixels per logical pixel change.
    fn scale_factor_changed(&mut self, scale_factor: f64);

    /// Called for each frame; the window is redrawn again afterwards.
    fn redraw(&mut self);

    /// Called when the pointer moves to `x`, `y` inside the window.
    fn pointer_moved(&mut self, x: f32, y: f32);

    /// Called when the pointer leaves the window.
    fn pointer_left(&mut self);

    /// Called with the relative pointer motion in device units, which keeps arriving while the
    /// cursor is captured.
    fn pointer_motion(&mut self, _dx: f32, _dy: f32) {}

    /// Called when a pointer button goes down or comes up.
    fn pointer_button(&mut self, button: PointerButton, state: ButtonState);

    /// Called when a wheel or touchpad scrolls.
    fn scrolled(&mut self, delta: ScrollDelta);

    /// Called when a key goes down or comes up.
    fn key(&mut self, event: &KeyEvent);

    /// Called with the text a key press produced.
    fn text_input(&mut self, text: &str);

    /// Whether the event loop should exit after a handler call.
    fn should_exit(&self) -> bool {
        false
    }

    /// Called when the user asks to close the window; the loop exits afterwards.
    fn close_requested(&mut self) {}
}

impl<H: WindowHandler + ?Sized> WindowHandler for &mut H {
    /// Forwards to the borrowed handler.
    fn created(&mut self, window: &Window) {
        (**self).created(window);
    }

    /// Forwards to the borrowed handler.
    fn resized(&mut self, width: u32, height: u32) {
        (**self).resized(width, height);
    }

    /// Forwards to the borrowed handler.
    fn scale_factor_changed(&mut self, scale_factor: f64) {
        (**self).scale_factor_changed(scale_factor);
    }

    /// Forwards to the borrowed handler.
    fn redraw(&mut self) {
        (**self).redraw();
    }

    /// Forwards to the borrowed handler.
    fn pointer_moved(&mut self, x: f32, y: f32) {
        (**self).pointer_moved(x, y);
    }

    /// Forwards to the borrowed handler.
    fn pointer_left(&mut self) {
        (**self).pointer_left();
    }

    /// Forwards to the borrowed handler.
    fn pointer_motion(&mut self, dx: f32, dy: f32) {
        (**self).pointer_motion(dx, dy);
    }

    /// Forwards to the borrowed handler.
    fn pointer_button(&mut self, button: PointerButton, state: ButtonState) {
        (**self).pointer_button(button, state);
    }

    /// Forwards to the borrowed handler.
    fn scrolled(&mut self, delta: ScrollDelta) {
        (**self).scrolled(delta);
    }

    /// Forwards to the borrowed handler.
    fn key(&mut self, event: &KeyEvent) {
        (**self).key(event);
    }

    /// Forwards to the borrowed handler.
    fn text_input(&mut self, text: &str) {
        (**self).text_input(text);
    }

    /// Forwards the request to stop the event loop.
    fn should_exit(&self) -> bool {
        (**self).should_exit()
    }

    /// Forwards to the borrowed handler.
    fn close_requested(&mut self) {
        (**self).close_requested();
    }
}
