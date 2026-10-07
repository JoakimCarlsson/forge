//! Glue between the app, the window and the renderer.

use std::fmt;

use fr_core::FrameClock;
use fr_render::{RenderError, Renderer};
use fr_window::{Window, WindowConfig, WindowError, WindowHandler};

use crate::{App, Frame};

/// A failure to start or run the engine.
#[derive(Debug)]
pub enum EngineError {
    /// The window or its event loop failed.
    Window(WindowError),
    /// The graphics device could not be set up.
    Render(RenderError),
}

impl fmt::Display for EngineError {
    /// Writes the failure with the cause that produced it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Window(error) => write!(f, "{error}"),
            Self::Render(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for EngineError {
    /// The underlying window or render error.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Window(error) => Some(error),
            Self::Render(error) => Some(error),
        }
    }
}

/// Connects an [`App`] to the window, the clock and the renderer.
struct Host<A> {
    /// The game being driven.
    app: A,
    /// The renderer, once the window exists.
    renderer: Option<Renderer>,
    /// Times each frame.
    clock: FrameClock,
    /// The current drawable size in physical pixels.
    size: (u32, u32),
    /// The first failure, reported when the loop ends.
    error: Option<RenderError>,
}

impl<A: App> WindowHandler for Host<A> {
    /// Creates the renderer for the new window.
    fn created(&mut self, window: &Window) {
        self.size = window.size();
        match Renderer::new(window.clone(), self.size.0, self.size.1) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => self.error = Some(error),
        }
    }

    /// Resizes the swapchain.
    fn resized(&mut self, width: u32, height: u32) {
        self.size = (width, height);
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(width, height);
        }
    }

    /// Updates the game, then draws the frame.
    fn redraw(&mut self) {
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        self.clock.tick();
        self.app.update(&Frame {
            delta_seconds: self.clock.delta_seconds(),
            index: self.clock.frame(),
            width: self.size.0,
            height: self.size.1,
        });
        renderer.clear(self.app.clear_color());
    }
}

/// Opens a window titled `title` and runs `app` until the window is closed.
///
/// # Errors
///
/// Returns [`EngineError`] when the window or the graphics device cannot be set up.
pub fn run<A: App>(title: &str, app: A) -> Result<(), EngineError> {
    let mut host = Host {
        app,
        renderer: None,
        clock: FrameClock::new(),
        size: (0, 0),
        error: None,
    };
    let config = WindowConfig {
        title: title.to_owned(),
        ..WindowConfig::default()
    };
    fr_window::run(config, HostRef(&mut host)).map_err(EngineError::Window)?;
    host.error
        .map_or(Ok(()), |error| Err(EngineError::Render(error)))
}

/// Lends a [`Host`] to the window loop so its error can be read afterwards.
struct HostRef<'a, A>(&'a mut Host<A>);

impl<A: App> WindowHandler for HostRef<'_, A> {
    /// Forwards to the host.
    fn created(&mut self, window: &Window) {
        self.0.created(window);
    }

    /// Forwards to the host.
    fn resized(&mut self, width: u32, height: u32) {
        self.0.resized(width, height);
    }

    /// Forwards to the host.
    fn redraw(&mut self) {
        self.0.redraw();
    }
}
