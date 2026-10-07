//! The winit event loop and window.

use std::fmt;
use std::sync::Arc;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{WindowAttributes, WindowId};

use crate::{WindowConfig, WindowHandler};

/// A failure to create the event loop, the window, or to run the loop.
#[derive(Debug)]
pub enum WindowError {
    /// The event loop could not be created or failed while running.
    EventLoop(winit::error::EventLoopError),
    /// The window could not be created.
    Window(winit::error::OsError),
}

impl fmt::Display for WindowError {
    /// Writes the failure with the cause that produced it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventLoop(error) => write!(f, "event loop failed: {error}"),
            Self::Window(error) => write!(f, "window creation failed: {error}"),
        }
    }
}

impl std::error::Error for WindowError {
    /// The underlying winit error.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EventLoop(error) => Some(error),
            Self::Window(error) => Some(error),
        }
    }
}

/// A shared handle to the native window, usable as a graphics surface target.
#[derive(Clone)]
pub struct Window {
    /// The winit window, shared so a surface can keep it alive.
    inner: Arc<winit::window::Window>,
}

impl Window {
    /// The drawable size in physical pixels.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        let size = self.inner.inner_size();
        (size.width, size.height)
    }

    /// Asks for the window to be redrawn.
    pub fn request_redraw(&self) {
        self.inner.request_redraw();
    }
}

impl HasWindowHandle for Window {
    /// The native window handle.
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle()
    }
}

impl HasDisplayHandle for Window {
    /// The native display handle.
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.inner.display_handle()
    }
}

/// Adapts a [`WindowHandler`] to winit's application interface.
struct Host<H> {
    /// The window creation parameters.
    config: WindowConfig,
    /// The caller's handler.
    handler: H,
    /// The window, once the loop has resumed.
    window: Option<Window>,
    /// The first failure, reported when the loop ends.
    error: Option<WindowError>,
}

impl<H: WindowHandler> ApplicationHandler for Host<H> {
    /// Creates the window on first resume and hands it to the handler.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = WindowAttributes::default()
            .with_title(self.config.title.clone())
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height));
        match event_loop.create_window(attributes) {
            Ok(inner) => {
                let window = Window {
                    inner: Arc::new(inner),
                };
                self.handler.created(&window);
                window.request_redraw();
                self.window = Some(window);
            }
            Err(error) => {
                self.error = Some(WindowError::Window(error));
                event_loop.exit();
            }
        }
    }

    /// Translates a winit window event into a handler call.
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.handler.close_requested();
                event_loop.exit();
            }
            WindowEvent::Resized(size) => self.handler.resized(size.width, size.height),
            WindowEvent::RedrawRequested => {
                self.handler.redraw();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

/// Opens a window and runs `handler` until the window is closed.
///
/// # Errors
///
/// Returns [`WindowError`] when the event loop or the window cannot be created.
pub fn run<H: WindowHandler>(config: WindowConfig, handler: H) -> Result<(), WindowError> {
    let event_loop = EventLoop::new().map_err(WindowError::EventLoop)?;
    let mut host = Host {
        config,
        handler,
        window: None,
        error: None,
    };
    event_loop
        .run_app(&mut host)
        .map_err(WindowError::EventLoop)?;
    host.error.map_or(Ok(()), Err)
}
