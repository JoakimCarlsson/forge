//! Glue between the app, the window and the renderer.

use std::fmt;

use fr_input::{ButtonState, KeyEvent, PointerButton, ScrollDelta};
use fr_math::{Point, Size};
use fr_render::{DrawList, RenderError, Renderer, Scene};
use fr_time::{FixedStepper, FrameClock};
use fr_ui::{Theme, Ui};
use fr_window::{Window, WindowConfig, WindowError, WindowHandler};

use crate::{App, Assets, FixedStep, Frame, Input};

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

/// The most fixed steps one frame may run before the remaining time is dropped.
const MAX_FIXED_STEPS_PER_FRAME: u32 = 8;

/// Connects an [`App`] to the window, the clock, the renderer and the UI.
struct Host<A: App> {
    /// The game being driven.
    app: A,
    /// The renderer, once the window exists.
    renderer: Option<Renderer>,
    /// The UI state that outlives each frame's tree.
    ui: Ui<A::Message>,
    /// The primitives of the frame being built, reused from frame to frame.
    list: DrawList,
    /// The 3D scene the app describes each frame, reused from frame to frame.
    scene: Scene,
    /// Times each frame.
    clock: FrameClock,
    /// Turns frame times into fixed steps.
    stepper: FixedStepper,
    /// The number of fixed steps run so far.
    fixed_steps: u64,
    /// The current drawable size in physical pixels.
    size: (u32, u32),
    /// The physical pixels per logical pixel.
    scale_factor: f64,
    /// The first failure, reported when the loop ends.
    error: Option<RenderError>,
}

impl<A: App> Host<A> {
    /// Passes the renderer the latest size and scale factor.
    fn resize_renderer(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(self.size.0, self.size.1, self.scale_factor as f32);
        }
    }

    /// Applies the message a control sent, if it sent one.
    fn deliver(&mut self, message: Option<A::Message>) {
        if let Some(message) = message {
            self.app.message(message);
        }
    }
}

impl<A: App> WindowHandler for Host<A> {
    /// Creates the renderer for the new window.
    fn created(&mut self, window: &Window) {
        self.size = window.size();
        self.scale_factor = window.scale_factor();
        match Renderer::new(
            window.clone(),
            self.size.0,
            self.size.1,
            self.scale_factor as f32,
        ) {
            Ok(mut renderer) => {
                self.app.init(&mut Assets::new(&mut renderer));
                self.renderer = Some(renderer);
            }
            Err(error) => self.error = Some(error),
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
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        self.clock.tick();
        let fixed_timestep = self.app.fixed_timestep();
        if self.stepper.step() != fixed_timestep {
            self.stepper.set_step(fixed_timestep);
        }
        for _ in 0..self.stepper.advance(self.clock.delta_seconds()) {
            self.app.fixed_update(&FixedStep {
                index: self.fixed_steps,
                delta_seconds: fixed_timestep,
            });
            self.fixed_steps += 1;
        }
        self.app.update(&Frame {
            delta_seconds: self.clock.delta_seconds(),
            index: self.clock.frame(),
            width: self.size.0,
            height: self.size.1,
            interpolation: self.stepper.alpha(),
        });

        self.scene.clear();
        self.app.scene(&mut self.scene);

        let theme = *self.ui.theme();
        let viewport = renderer.size();
        self.list.reset(viewport);
        let view = self.app.view(&theme);
        self.ui.draw(
            renderer.text(),
            &mut self.list,
            viewport,
            Point::default(),
            view,
        );
        let vsync = self.app.vsync();
        if renderer.vsync() != vsync {
            renderer.set_vsync(vsync);
        }
        renderer.render(&self.list, Some(&self.scene), self.app.clear_color(&theme));
    }

    /// Moves the pointer in the UI.
    fn pointer_moved(&mut self, x: f32, y: f32) {
        self.ui.pointer_moved(Point::new(x, y));
        self.app.input(&Input::PointerMoved { x, y });
    }

    /// Takes the pointer out of the UI.
    fn pointer_left(&mut self) {
        self.ui.pointer_left();
        self.app.input(&Input::PointerLeft);
    }

    /// Presses or releases a button in the UI, and tells the app what the UI did not take.
    fn pointer_button(&mut self, button: PointerButton, state: ButtonState) {
        let over_ui = self.ui.pointer_over_region();
        let message = self.ui.pointer_button(button, state);
        self.deliver(message);
        if state == ButtonState::Released || !over_ui {
            self.app.input(&Input::PointerButton { button, state });
        }
    }

    /// Tells the app about scrolling that is not over the UI.
    fn scrolled(&mut self, delta: ScrollDelta) {
        if !self.ui.pointer_over_region() {
            self.app.input(&Input::Scrolled(delta));
        }
    }

    /// Moves focus on tab, activates it on enter or space, drops it on escape.
    fn key(&mut self, event: &KeyEvent) {
        self.app.input(&Input::Key(event.clone()));
        let message = self.ui.key(event);
        self.deliver(message);
    }

    /// Ignores text, which no UI element takes yet.
    fn text_input(&mut self, _text: &str) {}
}

/// Opens a window titled `title` and runs `app` until the window is closed.
///
/// # Errors
///
/// Returns [`EngineError`] when the window or the graphics device cannot be set up.
pub fn run<A: App>(title: &str, app: A) -> Result<(), EngineError> {
    let ui = Ui::new(Theme::default());
    let mut host = Host {
        app,
        renderer: None,
        ui,
        list: DrawList::new(Size::zero()),
        scene: Scene::default(),
        clock: FrameClock::new(),
        stepper: FixedStepper::new(1.0 / 60.0, MAX_FIXED_STEPS_PER_FRAME),
        fixed_steps: 0,
        size: (0, 0),
        scale_factor: 1.0,
        error: None,
    };
    let config = WindowConfig {
        title: title.to_owned(),
        ..WindowConfig::default()
    };
    fr_window::run(config, &mut host).map_err(EngineError::Window)?;
    host.error
        .map_or(Ok(()), |error| Err(EngineError::Render(error)))
}
