//! Glue between the app, the window and the renderer.

use std::fmt;

use fr_input::{ButtonState, CursorMode, KeyEvent, PointerButton, ScrollDelta};
use fr_math::{Point, Size};
use fr_render::{DrawList, RenderError, Renderer, Scene};
use fr_time::FrameClock;
use fr_ui::{Theme, Ui};
use fr_window::{Window, WindowConfig, WindowError, WindowHandler};

use crate::uploads::Uploads;
use crate::{App, Assets, Frame, Input, Inputs, RenderScene, UiMessages, WindowSettings};

/// A failure to start or run the engine.
#[derive(Debug)]
pub enum EngineError {
    /// The window or its event loop failed.
    Window(WindowError),
    /// The graphics device could not be set up.
    Render(RenderError),
    /// An application system reported a failure.
    App(String),
}

impl fmt::Display for EngineError {
    /// Writes the failure with the cause that produced it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Window(error) => write!(f, "{error}"),
            Self::Render(error) => write!(f, "{error}"),
            Self::App(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for EngineError {
    /// The underlying window or render error.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Window(error) => Some(error),
            Self::Render(error) => Some(error),
            Self::App(_) => None,
        }
    }
}

/// Connects an [`App`] to the window, the clock, the renderer and the UI.
struct Host<M: Clone + Send + Sync + 'static> {
    /// The game being driven.
    app: App<M>,
    /// The window, once it exists.
    window: Option<Window>,
    /// The cursor mode the window was last given.
    cursor_mode: CursorMode,
    /// The renderer, once the window exists.
    renderer: Option<Renderer>,
    /// The UI state that outlives each frame's tree.
    ui: Ui<M>,
    /// The primitives of the frame being built, reused from frame to frame.
    list: DrawList,
    /// The 3D scene the app describes each frame, reused from frame to frame.
    scene: Scene,
    /// Times each frame.
    clock: FrameClock,
    /// Synchronizes CPU assets to renderer resources.
    uploads: Uploads,
    /// The current drawable size in physical pixels.
    size: (u32, u32),
    /// The physical pixels per logical pixel.
    scale_factor: f64,
    /// The first failure, reported when the loop ends.
    error: Option<EngineError>,
}

impl<M: Clone + Send + Sync + 'static> Host<M> {
    /// Passes the renderer the latest size and scale factor.
    fn resize_renderer(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(self.size.0, self.size.1, self.scale_factor as f32);
        }
    }

    /// Gives the window the cursor mode the app asks for when it changed.
    fn apply_cursor_mode(&mut self) {
        let mode = self.app.world().resource::<WindowSettings>().cursor_mode;
        if mode != self.cursor_mode
            && let Some(window) = &self.window
        {
            window.set_cursor_mode(mode);
            self.cursor_mode = mode;
        }
    }

    /// Queues input for the next frame's input systems.
    fn enqueue(&mut self, input: Input) {
        self.app.world_mut().resource_mut::<Inputs>().0.push(input);
    }

    /// Applies the message a control sent, if it sent one.
    fn deliver(&mut self, message: Option<M>) {
        if let Some(message) = message {
            self.app
                .world_mut()
                .resource_mut::<UiMessages<M>>()
                .0
                .push(message);
        }
    }
}

impl<M: Clone + Send + Sync + 'static> WindowHandler for Host<M> {
    /// Creates the renderer for the new window.
    fn created(&mut self, window: &Window) {
        self.size = window.size();
        self.scale_factor = window.scale_factor();
        self.window = Some(window.clone());
        match Renderer::new(
            window.clone(),
            self.size.0,
            self.size.1,
            self.scale_factor as f32,
        ) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => self.error = Some(EngineError::Render(error)),
        }
    }

    /// Resizes the swapchain.
    fn resized(&mut self, width: u32, height: u32) {
        self.size = (width, height);
        self.resize_renderer();
    }

    /// Rescales the swapchain's logical size.
    fn scale_factor_changed(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor;
        self.resize_renderer();
    }

    /// Updates the game, builds its tree, then draws the frame.
    fn redraw(&mut self) {
        if self.error.is_some() || self.renderer.is_none() {
            return;
        }
        self.clock.tick();
        let frame = Frame {
            delta_seconds: self.clock.delta_seconds(),
            index: self.clock.frame(),
            width: self.size.0,
            height: self.size.1,
            scale_factor: self.scale_factor as f32,
            interpolation: 0.0,
        };
        if let Err(error) = self.app.update(frame) {
            self.error = Some(error);
            return;
        }
        self.apply_cursor_mode();
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        if let Err(error) = self
            .uploads
            .sync(self.app.world().resource::<Assets>(), renderer)
        {
            self.error = Some(EngineError::Render(error));
            return;
        }
        self.scene = self
            .uploads
            .resolve(&self.app.world().resource::<RenderScene>().0);

        let theme = *self.ui.theme();
        let viewport = renderer.size();
        self.list.reset(viewport);
        let view = (self.app.view)(self.app.world(), &theme);
        self.ui.draw(
            renderer.text(),
            &mut self.list,
            viewport,
            Point::default(),
            view,
        );
        let vsync = self.app.world().resource::<WindowSettings>().vsync;
        if renderer.vsync() != vsync {
            renderer.set_vsync(vsync);
        }
        renderer.render(
            &self.list,
            Some(&self.scene),
            (self.app.clear_color)(self.app.world(), &theme),
        );
    }

    /// Moves the pointer in the UI.
    fn pointer_moved(&mut self, x: f32, y: f32) {
        let message = self.ui.pointer_moved(Point::new(x, y));
        self.deliver(message);
        self.enqueue(Input::PointerMoved { x, y });
    }

    /// Reports relative pointer motion to the app.
    fn pointer_motion(&mut self, dx: f32, dy: f32) {
        self.enqueue(Input::PointerMotion { dx, dy });
    }

    /// Takes the pointer out of the UI.
    fn pointer_left(&mut self) {
        self.ui.pointer_left();
        self.enqueue(Input::PointerLeft);
    }

    /// Presses or releases a button in the UI, and tells the app what the UI did not take.
    fn pointer_button(&mut self, button: PointerButton, state: ButtonState) {
        let over_ui = self.ui.pointer_over_region();
        let message = self.ui.pointer_button(button, state);
        self.deliver(message);
        if state == ButtonState::Released || !over_ui {
            self.enqueue(Input::PointerButton { button, state });
        }
    }

    /// Tells the app about scrolling that is not over the UI.
    fn scrolled(&mut self, delta: ScrollDelta) {
        if !self.ui.pointer_over_region() {
            self.enqueue(Input::Scrolled(delta));
        }
    }

    /// Moves focus on tab, activates it on enter or space, drops it on escape.
    fn key(&mut self, event: &KeyEvent) {
        self.enqueue(Input::Key(event.clone()));
        let message = self.ui.key(event);
        self.deliver(message);
    }

    /// Exits promptly when initialization or an application system fails.
    fn should_exit(&self) -> bool {
        self.error.is_some()
    }

    /// Ignores text, which no UI element takes yet.
    fn text_input(&mut self, _text: &str) {}
}

/// Opens a window titled `title` and runs `app` until the window is closed.
///
/// # Errors
///
/// Returns [`EngineError`] when the window or the graphics device cannot be set up.
pub fn run<M: Clone + Send + Sync + 'static>(
    title: &str,
    mut app: App<M>,
) -> Result<(), EngineError> {
    app.start()?;
    let ui = Ui::new(Theme::default());
    let mut host = Host {
        app,
        window: None,
        cursor_mode: CursorMode::Normal,
        renderer: None,
        ui,
        list: DrawList::new(Size::zero()),
        scene: Scene::default(),
        clock: FrameClock::new(),
        uploads: Uploads::default(),
        size: (0, 0),
        scale_factor: 1.0,
        error: None,
    };
    let config = WindowConfig {
        title: title.to_owned(),
        ..WindowConfig::default()
    };
    fr_window::run(config, &mut host).map_err(EngineError::Window)?;
    host.error.map_or(Ok(()), Err)
}
