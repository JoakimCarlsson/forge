//! The window host: the event loop's handler that drives the [`Editor`].

use std::time::Instant;

use fr_input::{ButtonState, KeyEvent, PointerButton, ScrollDelta};
use fr_render::Renderer;
use fr_window::{Window, WindowConfig, WindowHandler, run};

use super::editor::Editor;
use crate::options::EditorOptions;

/// The window size, in logical pixels, when none was remembered.
const DEFAULT_WINDOW: (u32, u32) = (1600, 900);

/// Connects the window's events to the editor and draws a frame per redraw.
struct EditorHost {
    /// The editor being driven.
    editor: Editor,
    /// When the previous frame began.
    last_frame: Instant,
    /// The drawable size in physical pixels.
    size: (u32, u32),
    /// The physical pixels per logical pixel.
    scale: f64,
    /// The failure that ends the loop.
    failure: Option<String>,
}

impl EditorHost {
    /// The window size in logical pixels.
    fn logical_size(&self) -> (u32, u32) {
        (
            (f64::from(self.size.0) / self.scale).round() as u32,
            (f64::from(self.size.1) / self.scale).round() as u32,
        )
    }
}

impl WindowHandler for EditorHost {
    /// Creates the renderer for the new window.
    fn created(&mut self, window: &Window) {
        self.size = window.size();
        self.scale = window.scale_factor();
        match Renderer::new(window.clone(), self.size.0, self.size.1, self.scale as f32) {
            Ok(mut renderer) => {
                renderer.set_vsync(true);
                self.editor.attach_renderer(renderer);
            }
            Err(error) => self.failure = Some(error.to_string()),
        }
    }

    /// Resizes the swapchain.
    fn resized(&mut self, width: u32, height: u32) {
        self.size = (width, height);
        self.editor.resize(width, height, self.scale as f32);
    }

    /// Rescales the swapchain's logical size.
    fn scale_factor_changed(&mut self, scale_factor: f64) {
        self.scale = scale_factor;
        self.editor
            .resize(self.size.0, self.size.1, scale_factor as f32);
    }

    /// Runs one frame.
    fn redraw(&mut self) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.editor.frame(delta);
    }

    /// Moves the pointer.
    fn pointer_moved(&mut self, x: f32, y: f32) {
        self.editor.pointer_moved(x, y);
    }

    /// Takes the pointer out of the window.
    fn pointer_left(&mut self) {
        self.editor.pointer_left();
    }

    /// Presses or releases a pointer button.
    fn pointer_button(&mut self, button: PointerButton, state: ButtonState) {
        self.editor.pointer_button(button, state);
    }

    /// Turns the wheel.
    fn scrolled(&mut self, delta: ScrollDelta) {
        self.editor.scrolled(delta);
    }

    /// Presses or releases a key.
    fn key(&mut self, event: &KeyEvent) {
        self.editor.key(event);
    }

    /// Ignores text: keys insert their own.
    fn text_input(&mut self, text: &str) {
        self.editor.text_input(text);
    }

    /// Asks the editor to quit, which may ask about unsaved changes.
    fn close_requested(&mut self) {
        self.editor.close_requested();
    }

    /// Keeps the window while a question is open.
    fn keeps_open(&self) -> bool {
        self.editor.keeps_open()
    }

    /// Whether the editor quit or something failed.
    fn wants_exit(&self) -> bool {
        self.failure.is_some() || self.editor.state.quit
    }
}

/// Opens the window and runs the editor until it ends.
///
/// # Errors
///
/// The message of the project that cannot be opened, the window or device that
/// cannot be created, or the failure that ended the loop.
pub fn run_window(options: &EditorOptions) -> Result<(), String> {
    let editor = Editor::start(options, true)?;
    let (width, height) = editor
        .state
        .settings
        .window_size()
        .unwrap_or(DEFAULT_WINDOW);
    let mut host = EditorHost {
        editor,
        last_frame: Instant::now(),
        size: (width, height),
        scale: 1.0,
        failure: None,
    };
    let config = WindowConfig {
        title: "forge".to_owned(),
        width,
        height,
    };
    let result = run(config, &mut host).map_err(|error| error.to_string());
    let size = host.logical_size();
    host.editor.shutdown(Some(size));
    result?;
    host.failure.map_or(Ok(()), Err)
}
